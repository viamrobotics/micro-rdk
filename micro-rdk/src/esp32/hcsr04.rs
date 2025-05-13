// Support for HC-SR04 style ultrasonic ranging modules. See
// https://cdn.sparkfun.com/datasheets/Sensors/Proximity/HCSR04.pdf.
//
// Example configuration
//
// {
//   "model": "ultrasonic",
//   "name": "ultrasonic-sensor",
//   "type": "sensor",
//   "attributes": {
//     "trigger_pin": "15",
//     "echo_interrupt_pin": "18"
//     "timeout_ms" : "20",
//   },
// }
//
// Configuration details:
//
// The following `attributes` section parameters configure the sensor:
//
//  - `trigger_pin` (required): The GPIO pin number connected to the pulse
//    trigger input on the sensor.
//
//  - `echo_interrupt_pin` (required): The GPIO pin number connected to the echo
//    interrupt pin. Please note that unlike the RDK ultrasonic
//    sensor, you must not use a named pin associated with a digital
//    interrupt configured on the board: it will not (currently) work.
//
//  - `timeout_ms` (optional): The maximum timeout the sensor will
//    wait for an echo pulse in milliseconds. If no echo is observed
//    within this timeout, an error will be returned to the caller. If
//    no `timeout_ms` is set, the sensor will default to 50ms. Values
//    are clamped between 100us and 100ms.
//
// Note that unlike the RDK ultrasonic sensor, the Micro-RDK sensor
// does not currently require a `board` attribute, though this may
// change in the future.

use std::{
    cell::RefCell,
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    common::{
        config::{AttributeError, ConfigType},
        registry::{ComponentRegistry, Dependency},
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorT,
            SensorType, TypedReadingsResult,
        },
    },
    DoCommand,
};

use crate::esp32::esp_idf_svc::hal::{
    delay::TickType,
    gpio::{AnyIOPin, Input, InterruptType, Output, PinDriver, Pull},
    task::notification::{Notification, Notifier},
};

use crate::esp32::esp_idf_svc::sys::{esp, gpio_isr_handler_remove};

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_sensor("ultrasonic", &HCSR04Sensor::from_config)
        .is_err()
    {
        log::error!("HCSR04Sensor is already registered");
    }
}

struct IsrSharedState {
    // The state machine used to track interrupts and compute the
    // length of the echo pulse. It holds one of the following values:
    //
    // 0: Starting state, ready to take a reading
    // i64 > 0: Millisecond timestamp of first edge of echo signal
    //TODO 32bit wide enough - To be fixed before merged
    timestamp: AtomicU32,

    // The channel the ISR will use to communicate results back to waiters.
    notifier: Arc<Notifier>,
}

#[derive(DoCommand)]
pub struct HCSR04Sensor {
    // The PinDriver to control the pin that triggers issuing a pulse.
    //
    // NOTE: This could be an Esp32GPIOPin, but instead uses PinDriver directly
    // for consistency with `echo_interrupt_pin` below, which cannot be.
    trigger_pin: RefCell<PinDriver<'static, AnyIOPin, Output>>,

    // The PinDriver used to listen for digital interrupts and measure
    // the length of the echo pulse.
    //
    // TODO(RSDK-6279): It would be nice to use Esp32GPIOPin here
    // instead, however, that type forces the pin into `InputOutput`
    // mode which appears not to work with digital interrupts.
    echo_interrupt_pin: RefCell<PinDriver<'static, AnyIOPin, Input>>,

    // How long we will wait for an echo pulse before concluding that there is no
    // obstacle in range. Defaults to 50ms.
    timeout: Duration,

    // The notification channel used to wait on a result being posted from the ISR.
    interrupt_notification: Notification,

    // State which we share with the ISR.
    isr_shared_state: Arc<IsrSharedState>,
}

// TODO(RSDK-9956): `Notification` contains an instance of `PhantomData<const* ()>`, but `const* ()` does not
// implement Send. We force an implementation of Send here so we can actually wrap in an Arc<Mutex<_>>
// for now, but we should investigate a better canonical solution
unsafe impl Send for HCSR04Sensor {}

impl HCSR04Sensor {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let board = crate::common::registry::get_board_from_dependencies(deps)
            .expect("failed to get board from dependencies");

        let trigger_pin = cfg
            .get_attribute::<i32>("trigger_pin")
            .map_err(|_| SensorError::ConfigError("HCSR04Sensor: missing `trigger_pin`"))?;

        let echo_interrupt_pin = cfg
            .get_attribute::<i32>("echo_interrupt_pin")
            .map_err(|_| SensorError::ConfigError("HCSR04Sensor: missing `echo_interrupt_pin`"))?;

        let timeout = cfg.get_attribute::<u32>("timeout_ms").map_or_else(
            |e| match e {
                AttributeError::KeyNotFound(_) => Ok(None),
                _ => Err(SensorError::ConfigError(
                    "HCSR04Sensor: error handling `timeout_ms`",
                )),
            },
            |v| Ok(Some(Duration::from_millis(v.into()))),
        )?;

        Ok(Arc::new(Mutex::new(HCSR04Sensor::new(
            trigger_pin,
            echo_interrupt_pin,
            timeout,
            board,
        )?)))
    }

    fn new(
        trigger_pin: i32,
        echo_interrupt_pin: i32,
        timeout: Option<Duration>,
        board: crate::common::board::BoardType,
    ) -> Result<HCSR04Sensor, SensorError> {
        // TODO(RSDK-6279): Unify with esp32/pin.rs.

        let mut board = board.lock().unwrap();
        let notification = Notification::new();
        let notifier = notification.notifier();

        let sensor = Self {
            trigger_pin: RefCell::new(
                PinDriver::output(unsafe { AnyIOPin::new(trigger_pin) })
                    .map_err(|err| SensorError::SensorCodeError(err.code()))?,
            ),
            echo_interrupt_pin: RefCell::new(
                PinDriver::input(unsafe { AnyIOPin::new(echo_interrupt_pin) })
                    .map_err(|err| SensorError::SensorCodeError(err.code()))?,
            ),
            timeout: timeout
                .unwrap_or(Duration::from_millis(50))
                .clamp(Duration::from_micros(100), Duration::from_millis(100)),
            interrupt_notification: notification,
            isr_shared_state: Arc::new(IsrSharedState {
                timestamp: 0.into(),
                notifier,
            }),
        };

        sensor
            .echo_interrupt_pin
            .borrow_mut()
            .set_pull(Pull::Down)
            .map_err(|err| SensorError::SensorCodeError(err.code()))?;

        // Start the trigger pin high: the pulse is sent on the
        // falling edge, so we can just go low immediately in
        // `get_readings` to send it. As long as we don't get another
        // `get_readings` request within 10us of the prior one
        // completing, the pin will be high long enough to trigger the
        // pulse.
        sensor
            .trigger_pin
            .borrow_mut()
            .set_high()
            .map_err(|err| SensorError::SensorCodeError(err.code()))?;

        let cb_arg = Arc::as_ptr(&sensor.isr_shared_state) as *mut _;
        board.add_digital_interrupt_callback(
            echo_interrupt_pin,
            InterruptType::AnyEdge,
            Some(Self::subscription_interrupt),
            Some(cb_arg),
        )?;
        Ok(sensor)
    }

    #[inline(always)]
    #[link_section = ".iram1.intr_srv"]
    unsafe extern "C" fn subscription_interrupt(arg: *mut core::ffi::c_void) {
        let arg: &mut IsrSharedState = &mut *(arg as *mut _);
        let when = crate::esp32::esp_idf_svc::sys::esp_timer_get_time() as u32;
        match arg
            .timestamp
            .compare_exchange(0, when, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                // Initial edge: timestamp gets stored.
            }
            Err(prior) => {
                // Terminal edge: notify the waiter if we can convert
                // the computed duration into a non-zero u32. If we
                // don't notify, the waiter will time out and return
                // an error, and the state machine will be reset on
                // the next `get_readings` call.
                // If prior > when delta will equal 0
                let delta = when.saturating_sub(prior);
                if let Some(nz) = NonZeroU32::new(delta) {
                    arg.notifier.notify_and_yield(nz);
                }
            }
        }
    }
}

impl Drop for HCSR04Sensor {
    fn drop(&mut self) {
        let pin = self.echo_interrupt_pin.borrow_mut().pin();
        if let Err(error) = unsafe { esp!(gpio_isr_handler_remove(pin)) } {
            log::warn!(
                "HCSR04Sensor: failed to remove interrupt handler for pin {}: {}",
                pin,
                error
            )
        }
    }
}

impl Sensor for HCSR04Sensor {}

impl Readings for HCSR04Sensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for HCSR04Sensor {
    fn get_readings(&self) -> Result<TypedReadingsResult<f64>, SensorError> {
        // If the echo pin is already high for some reason, the state machine
        // won't work correctly.
        if self.echo_interrupt_pin.borrow().is_high() {
            return Err(SensorError::SensorGenericError(
                "HCSR04Sensor : echo pin is high befor trigger is sent",
            ));
        }

        // Reset the state machine: store zero to unlock the first
        // compare_exchange in the ISR, and consume any pending
        // notification that we may have missed on a prior timeout.
        self.isr_shared_state.timestamp.store(0, Ordering::Release);
        let _ = self.interrupt_notification.wait(0);

        // Drive the pin low to trigger the pulse, and ensure we put
        // it back to high after our wait.
        let mut trigger_pin = self.trigger_pin.borrow_mut();
        trigger_pin
            .set_low()
            .map_err(|err| SensorError::SensorCodeError(err.code()))?;

        defer! {
            let _ = trigger_pin.set_high();
        }

        // Wait (up to timeout) for a notification from the
        // ISR. Convert any result from the notification into a
        // distance.
        //
        // TODO(RSDK-6278): This blocks the calling thread. It would
        // be better to find a way to leverage an executor to avoid
        // the blocking wait.
        match self
            .interrupt_notification
            .wait(TickType::from(self.timeout).as_millis_u32())
        {
            Some(delta) => {
                let distance = delta.get() as f64 / 58.0 / 100.0;
                Ok(HashMap::from([("distance".to_string(), distance)]))
            }
            _ => Err(SensorError::SensorGenericError(
                "HCSR04Sensor no echo heard obstacle may be out of range",
            )),
        }
    }
}
