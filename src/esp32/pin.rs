use super::pwm::PwmDriver;
use crate::esp_idf_svc::hal::gpio::{AnyIOPin, InputOutput, InterruptType, Pin, PinDriver, Pull};
use crate::esp_idf_svc::sys::{
    esp, gpio_install_isr_service, gpio_isr_handler_add, ESP_INTR_FLAG_IRAM,
};
use once_cell::sync::{Lazy, OnceCell};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub trait PinExt {
    fn pin(&self) -> i32;
}

impl<'d, T: Pin, MODE> PinExt for PinDriver<'d, T, MODE> {
    fn pin(&self) -> i32 {
        self.pin()
    }
}

fn install_gpio_isr_service() -> anyhow::Result<()> {
    static GPIO_ISR_SERVICE_INSTALLED: Lazy<Arc<OnceCell<()>>> =
        Lazy::new(|| Arc::new(OnceCell::new()));
    GPIO_ISR_SERVICE_INSTALLED.get_or_try_init(|| {
        unsafe {
            esp!(gpio_install_isr_service(ESP_INTR_FLAG_IRAM as i32))?;
        };
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
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
    pub fn new(pin: i32, pull: Option<Pull>) -> anyhow::Result<Self> {
        let mut driver = PinDriver::input_output(unsafe { AnyIOPin::new(pin) })?;
        if let Some(pull) = pull {
            driver.set_pull(pull)?;
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

    pub fn set_high(&mut self) -> anyhow::Result<()> {
        if self.pwm_driver.is_some() {
            anyhow::bail!(
                "pin {:?} currently has a PWM signal active, cannot set level",
                self.pin
            )
        }
        self.driver
            .set_high()
            .map_err(|e| anyhow::anyhow!("couldn't set pin {} high {}", self.pin, e))
    }

    pub fn set_low(&mut self) -> anyhow::Result<()> {
        self.driver
            .set_low()
            .map_err(|e| anyhow::anyhow!("couldn't set pin {} low {}", self.pin, e))
    }

    pub fn get_pwm_duty(&self) -> f64 {
        match &self.pwm_driver {
            Some(pwm_driver) => pwm_driver.get_ledc_duty_pct(),
            None => 0.0,
        }
    }

    pub fn set_pwm_duty(&mut self, pct: f64) -> anyhow::Result<()> {
        if self.interrupt_type.is_some() {
            anyhow::bail!(
                "pin {:?} set as digital interrupt, PWM functionality unavailable",
                self.pin
            )
        }
        match self.pwm_driver.as_mut() {
            Some(pwm_driver) => {
                pwm_driver.set_ledc_duty_pct(pct)?;
            }
            None => {
                let mut pwm_driver = PwmDriver::new(unsafe { AnyIOPin::new(self.pin) }, 10000)?;
                pwm_driver.set_ledc_duty_pct(pct)?;
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

    pub fn set_pwm_frequency(&mut self, freq: u64) -> anyhow::Result<()> {
        if self.interrupt_type.is_some() {
            anyhow::bail!(
                "pin {:?} set as digital interrupt, PWM functionality unavailable",
                self.pin
            )
        }
        if freq == 0 {
            self.pwm_driver = None
        } else {
            match self.pwm_driver.as_mut() {
                Some(pwm_driver) => {
                    pwm_driver.set_timer_frequency(freq as u32)?;
                }
                None => {
                    let pwm_driver =
                        PwmDriver::new(unsafe { AnyIOPin::new(self.pin) }, freq as u32)?;
                    self.pwm_driver = Some(pwm_driver);
                }
            }
        }
        Ok(())
    }

    pub fn is_interrupt(&self) -> bool {
        self.interrupt_type.is_some()
    }

    pub fn setup_interrupt(&mut self, intr_type: InterruptType) -> anyhow::Result<()> {
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
        install_gpio_isr_service()?;
        self.driver.set_interrupt_type(intr_type)?;
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
            ))?;
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
