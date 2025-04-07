use super::pwm::PwmDriver;
use crate::common::board::BoardError;
use crate::esp32::esp_idf_svc::hal::gpio::{
    AnyIOPin, InputOutput, InterruptType, Pin, PinDriver, Pull,
};
use crate::esp32::esp_idf_svc::sys::{
    esp, gpio_install_isr_service, gpio_isr_handler_add, ESP_INTR_FLAG_IRAM,
    SOC_GPIO_VALID_OUTPUT_GPIO_MASK,
};
use once_cell::sync::{Lazy, OnceCell};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub trait PinExt {
    fn pin(&self) -> i32;
}

impl<T: Pin, MODE> PinExt for PinDriver<'_, T, MODE> {
    fn pin(&self) -> i32 {
        self.pin()
    }
}

fn install_gpio_isr_service() -> Result<(), BoardError> {
    static GPIO_ISR_SERVICE_INSTALLED: Lazy<Arc<OnceCell<()>>> =
        Lazy::new(|| Arc::new(OnceCell::new()));
    GPIO_ISR_SERVICE_INSTALLED.get_or_try_init(|| {
        unsafe {
            esp!(gpio_install_isr_service(ESP_INTR_FLAG_IRAM as i32))
                .map_err(|e| BoardError::OtherBoardError(Box::new(e)))?;
        };
        Ok::<(), BoardError>(())
    })?;
    Ok(())
}

// Since C macros are not usable in Rust via FFI, we cannot GPIO_IS_VALID_OUTPUT_GPIO from
// ESP-IDF, so we must replicate the logic from that function here. If we do not validate,
// PinDriver::input_output will panic because esp-idf-hal does not perform the same check
fn is_valid_gpio_pin(pin: i32) -> Result<(), BoardError> {
    // Do this masking in 64-bit space because it works for both esp32
    // where the mask is 32 bits and esp32s3 where it is 64.
    if !(0..64).contains(&pin) {
        return Err(BoardError::InvalidGpioNumber(pin as u32));
    }
    #[allow(clippy::unnecessary_cast)]
    match (1_u64 << (pin as u64)) & (SOC_GPIO_VALID_OUTPUT_GPIO_MASK as u64) {
        0 => Err(BoardError::InvalidGpioNumber(pin as u32)),
        _ => Ok(()),
    }
}

/// Esp32GPIOPin is a wrapper for a pin on ESP32 as represented in esp-idf-hal
/// and esp-idf-sys. This exists so that all micro-RDK drivers can interact
/// with pins through the board instance and avoid conflicting uses of pins
/// by multiple processes
pub struct Esp32GPIOPin {
    pin: i32,
    driver: PinDriver<'static, AnyIOPin, InputOutput>,
    interrupt_type: Option<InterruptType>,
    event_count: Arc<AtomicU32>,
    pwm_driver: Option<PwmDriver<'static>>,
}

impl Esp32GPIOPin {
    pub fn new(pin: i32, pull: Option<Pull>) -> Result<Self, BoardError> {
        is_valid_gpio_pin(pin)?;
        let mut driver = PinDriver::input_output(unsafe { AnyIOPin::new(pin) })
            .map_err(|e| BoardError::GpioPinOtherError(pin as u32, Box::new(e)))?;
        if let Some(pull) = pull {
            driver
                .set_pull(pull)
                .map_err(|e| BoardError::GpioPinOtherError(pin as u32, Box::new(e)))?;
        }
        Ok(Self {
            pin,
            driver,
            interrupt_type: None,
            event_count: Arc::new(AtomicU32::new(0)),
            pwm_driver: None,
        })
    }

    pub fn pin(&self) -> i32 {
        self.pin
    }

    pub fn is_high(&self) -> bool {
        self.driver.is_high()
    }

    pub fn set_high(&mut self) -> Result<(), BoardError> {
        if self.pwm_driver.is_some() {
            return Err(BoardError::GpioPinError(
                self.pin as u32,
                "is pwm cannot set level",
            ));
        }
        // TODO losing esperr info -> make sure it can be logged?
        self.driver
            .set_high()
            .map_err(|_| BoardError::GpioPinError(self.pin as u32, "cannot set high"))
    }

    pub fn set_low(&mut self) -> Result<(), BoardError> {
        if self.pwm_driver.is_some() {
            return Err(BoardError::GpioPinError(
                self.pin as u32,
                "is pwm cannot set level",
            ));
        }
        self.driver
            .set_low()
            .map_err(|_| BoardError::GpioPinError(self.pin as u32, "cannot set high"))
    }

    pub fn get_pwm_duty(&self) -> f64 {
        match &self.pwm_driver {
            Some(pwm_driver) => pwm_driver.get_ledc_duty_pct(),
            None => 0.0,
        }
    }

    pub fn set_pwm_duty(&mut self, pct: f64) -> Result<(), BoardError> {
        if self.interrupt_type.is_some() {
            return Err(BoardError::GpioPinError(
                self.pin as u32,
                "is not a pwm pin",
            ));
        }
        match self.pwm_driver.as_mut() {
            Some(pwm_driver) => {
                pwm_driver
                    .set_ledc_duty_pct(pct)
                    .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
            }
            None => {
                let mut pwm_driver = PwmDriver::new(unsafe { AnyIOPin::new(self.pin) }, 10000)
                    .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
                pwm_driver
                    .set_ledc_duty_pct(pct)
                    .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
                self.pwm_driver = Some(pwm_driver);
            }
        };
        Ok(())
    }

    pub fn get_pwm_frequency(&self) -> u64 {
        match &self.pwm_driver {
            Some(pwm_driver) => pwm_driver.get_timer_frequency() as u64,
            None => 0,
        }
    }

    pub fn set_pwm_frequency(&mut self, freq: u64) -> Result<(), BoardError> {
        if self.interrupt_type.is_some() {
            return Err(BoardError::GpioPinError(
                self.pin as u32,
                "is not a pwm pin",
            ));
        }
        if freq == 0 {
            self.pwm_driver = None
        } else {
            match self.pwm_driver.as_mut() {
                Some(pwm_driver) => {
                    pwm_driver
                        .set_timer_frequency(freq as u32)
                        .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
                }
                None => {
                    let pwm_driver =
                        PwmDriver::new(unsafe { AnyIOPin::new(self.pin) }, freq as u32).map_err(
                            |e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)),
                        )?;
                    self.pwm_driver = Some(pwm_driver);
                }
            }
        }
        Ok(())
    }

    pub fn is_interrupt(&self) -> bool {
        self.interrupt_type.is_some()
    }

    pub fn setup_interrupt(&mut self, intr_type: InterruptType) -> Result<(), BoardError> {
        match &self.interrupt_type {
            Some(existing_type) => {
                if *existing_type == intr_type {
                    return Ok(());
                }
            }
            None => {
                self.interrupt_type = Some(intr_type);
            }
        };
        install_gpio_isr_service()
            .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
        self.driver
            .set_interrupt_type(intr_type)
            .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
        self.event_count.store(0, Ordering::Relaxed);
        unsafe {
            // we can't use the subscribe method on PinDriver to add the handler
            // because it requires an FnMut with a static lifetime. A possible follow-up
            // would be to lazily initialize a Esp32GPIOPin for every possible pin (delineated by feature)
            // in a global state which an EspBoard instance would be able to access
            esp!(gpio_isr_handler_add(
                self.pin,
                Some(Self::interrupt),
                &mut self.event_count as *mut Arc<AtomicU32> as *mut _
            ))
            .map_err(|e| BoardError::GpioPinOtherError(self.pin as u32, Box::new(e)))?;
        }
        Ok(())
    }

    pub fn get_event_count(&self) -> u32 {
        self.event_count.load(Ordering::Relaxed)
    }

    #[inline(always)]
    #[link_section = ".iram1.intr_srv"]
    unsafe extern "C" fn interrupt(arg: *mut core::ffi::c_void) {
        let arg: &mut Arc<AtomicU32> = &mut *(arg as *mut _);
        arg.fetch_add(1, Ordering::Relaxed);
    }
}
