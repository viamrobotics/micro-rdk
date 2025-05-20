#![allow(dead_code)]
use log::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    common::{
        analog::{AnalogReader, AnalogReaderType},
        board::{Board, BoardError, BoardType},
        config::ConfigType,
        digital_interrupt::DigitalInterruptConfig,
        i2c::I2cHandleType,
        registry::ComponentRegistry,
    },
    proto::component,
};

#[cfg(esp32)]
use crate::common::analog::AnalogReaderConfig;

use super::{
    i2c::{Esp32I2C, Esp32I2cConfig},
    pin::Esp32GPIOPin,
};

#[cfg(esp32)]
use super::analog::Esp32AnalogReader;

// TODO(RSDK-10188): Update to ESP-IDF ADC API
#[cfg(esp32)]
use crate::esp32::esp_idf_svc::hal::adc::{
    attenuation::DB_11,
    oneshot::{
        config::{AdcChannelConfig, Calibration},
        AdcChannelDriver, AdcDriver,
    },
    ADC1,
};

use crate::common::board::InterruptType;
use crate::esp32::esp_idf_svc::hal::gpio::InterruptType as Esp32InterruptType;

impl From<InterruptType> for Esp32InterruptType {
    fn from(value: InterruptType) -> Self {
        match value {
            InterruptType::PosEdge => Esp32InterruptType::PosEdge,
            InterruptType::NegEdge => Esp32InterruptType::NegEdge,
            InterruptType::AnyEdge => Esp32InterruptType::AnyEdge,
            InterruptType::LowLevel => Esp32InterruptType::LowLevel,
            InterruptType::HighLevel => Esp32InterruptType::HighLevel,
        }
    }
}

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_board("esp32", &EspBoard::from_config)
        .is_err()
    {
        log::error!("esp32 board type already registered");
    }
}

/// An ESP32 implementation that wraps esp-idf functionality
#[derive(DoCommand)]
pub struct EspBoard {
    pins: Vec<Esp32GPIOPin>,
    analogs: Vec<AnalogReaderType<u16>>,
    i2cs: HashMap<String, I2cHandleType>,
}

impl EspBoard {
    pub fn new(
        pins: Vec<Esp32GPIOPin>,
        analogs: Vec<AnalogReaderType<u16>>,
        i2cs: HashMap<String, I2cHandleType>,
    ) -> Self {
        EspBoard {
            pins,
            analogs,
            i2cs,
        }
    }
    /// This is a temporary approach aimed at ensuring a good POC for runtime config consumption by the ESP32,
    /// Down the road we will need to wrap the Esp32Board in a singleton instance owning the peripherals and giving them as requested.
    /// The potential approach is described in esp32/motor.rs:383
    pub(crate) fn from_config(cfg: ConfigType) -> Result<BoardType, BoardError> {
        let (analogs, pins, i2c_confs) = {
            // TODO(RSDK-8451): The logic below is hardcoded for esp32
            // and is not appropriate for esp32s3 (or other boards).
            #[cfg(not(esp32))]
            let analogs = vec![];
            #[cfg(esp32)]
            let analogs =
                if let Ok(analogs) = cfg.get_attribute::<Vec<AnalogReaderConfig>>("analogs") {
                    let adc1 = Arc::new(AdcDriver::new(unsafe { ADC1::new() })?);
                    let analogs: Result<Vec<AnalogReaderType<u16>>, BoardError> = analogs
                        .iter()
                        .map(|v| {
                            let config = AdcChannelConfig {
                                attenuation: DB_11,
                                calibration: Calibration::Line,
                                ..Default::default()
                            };
                            let chan: Result<AnalogReaderType<u16>, BoardError> = match v.pin {
                                32 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio32::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                33 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio33::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                34 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio34::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                35 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio35::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                36 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio36::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                37 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio37::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                38 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio38::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                39 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::new(adc1.clone(), unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio39::new()
                                            }, &config)
                                            .map_err(BoardError::EspError)?
                                        )));
                                    Ok(p)
                                }
                                _ => {
                                    log::error!("pin {} is not an ADC1 pin", v.pin);
                                    Err(BoardError::GpioPinError(
                                        v.pin as u32,
                                        "Pin is not an ADC1 pin",
                                    ))
                                }
                            };
                            chan
                        })
                        .collect::<Result<Vec<AnalogReaderType<u16>>, BoardError>>();
                    analogs?
                } else {
                    vec![]
                };

            let mut pins = cfg.get_attribute::<Vec<i32>>("pins").unwrap_or_default();
            if let Ok(interrupt_confs) =
                cfg.get_attribute::<Vec<DigitalInterruptConfig>>("digital_interrupts")
            {
                pins.extend(interrupt_confs.iter().map(|conf| conf.pin));
            }
            pins.sort();
            pins.dedup();

            let pins = pins
                .iter()
                .filter_map(|pin| match Esp32GPIOPin::new(*pin, None) {
                    Ok(p) => Some(p),
                    Err(err) => {
                        log::error!("Error configuring pin: {:?}", err);
                        None
                    }
                })
                .collect();

            let i2c_confs = cfg
                .get_attribute::<Vec<Esp32I2cConfig>>("i2cs")
                .unwrap_or_default();
            (analogs, pins, i2c_confs)
        };

        let mut i2cs = HashMap::new();
        for conf in i2c_confs.iter() {
            let name = conf.name.to_string();
            let i2c = Esp32I2C::new_from_config(conf)?;
            let i2c_wrapped: I2cHandleType = Arc::new(Mutex::new(i2c));
            i2cs.insert(name.to_string(), i2c_wrapped);
        }

        let mut board = Self {
            pins,
            analogs,
            i2cs,
        };
        if let Ok(interrupt_confs) =
            cfg.get_attribute::<Vec<DigitalInterruptConfig>>("digital_interrupts")
        {
            for conf in interrupt_confs {
                // RSDK-4763: make event type configurable
                board.add_digital_interrupt_callback(
                    conf.pin,
                    InterruptType::PosEdge,
                    None,
                    None,
                )?;
            }
        }
        Ok(Arc::new(Mutex::new(board)))
    }
}

impl Board for EspBoard {
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> Result<(), BoardError> {
        let p = self.pins.iter_mut().find(|p| p.pin() == pin);
        if let Some(p) = p {
            if p.is_interrupt() {
                return Err(BoardError::GpioPinError(
                    pin as u32,
                    "is registered as an interrupt",
                ));
            }
            if is_high {
                return p.set_high();
            } else {
                return p.set_low();
            }
        }
        Err(BoardError::GpioPinError(pin as u32, "not an output"))
    }
    fn get_gpio_level(&self, pin: i32) -> Result<bool, BoardError> {
        let pin = self
            .pins
            .iter()
            .find(|p| p.pin() == pin)
            .ok_or(BoardError::GpioPinError(pin as u32, "not registered"))?;
        Ok(pin.is_high())
    }
    fn get_pwm_duty(&self, pin: i32) -> f64 {
        match self.pins.iter().find(|p| p.pin() == pin) {
            None => 0.0,
            Some(pin) => pin.get_pwm_duty(),
        }
    }
    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> Result<(), BoardError> {
        let pin = self
            .pins
            .iter_mut()
            .find(|p| p.pin() == pin)
            .ok_or(BoardError::GpioPinError(pin as u32, "not registered"))?;
        pin.set_pwm_duty(duty_cycle_pct)
    }
    fn get_pwm_frequency(&self, pin: i32) -> Result<u64, BoardError> {
        let pin = self
            .pins
            .iter()
            .find(|p| p.pin() == pin)
            .ok_or(BoardError::GpioPinError(pin as u32, "not registered"))?;
        Ok(pin.get_pwm_frequency())
    }
    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> Result<(), BoardError> {
        let pin = self
            .pins
            .iter_mut()
            .find(|p| p.pin() == pin)
            .ok_or(BoardError::GpioPinError(pin as u32, "not registered"))?;
        pin.set_pwm_frequency(frequency_hz)
    }
    fn get_analog_reader_by_name(&self, name: String) -> Result<AnalogReaderType<u16>, BoardError> {
        match self.analogs.iter().find(|a| a.name() == name) {
            Some(reader) => Ok(reader.clone()),
            None => Err(BoardError::AnalogReaderNotFound(name)),
        }
    }
    fn set_power_mode(
        &self,
        mode: component::board::v1::PowerMode,
        duration: Option<Duration>,
    ) -> Result<(), BoardError> {
        info!(
            "Esp32 received request to set power mode to {} for {} milliseconds",
            mode.as_str_name(),
            match duration {
                Some(dur) => dur.as_millis().to_string(),
                None => "<forever>".to_string(),
            }
        );

        if mode != component::board::v1::PowerMode::OfflineDeep {
            return Err(BoardError::BoardUnsupportedArgument(
                "only support OfflineDeep mode",
            ));
        }

        if let Some(dur) = duration {
            let dur_micros = dur.as_micros() as u64;
            let result: crate::esp32::esp_idf_svc::sys::esp_err_t;
            unsafe {
                result = crate::esp32::esp_idf_svc::sys::esp_sleep_enable_timer_wakeup(dur_micros);
            }
            if result != crate::esp32::esp_idf_svc::sys::ESP_OK {
                return Err(BoardError::BoardUnsupportedArgument("duration too long"));
            }
            warn!("Esp32 entering deep sleep for {} microseconds!", dur_micros);
        } else {
            warn!("Esp32 entering deep sleep without scheduled wakeup!");
        }

        unsafe {
            crate::esp32::esp_idf_svc::sys::esp_deep_sleep_start();
        }
    }
    fn get_i2c_by_name(&self, name: String) -> Result<I2cHandleType, BoardError> {
        match self.i2cs.get(&name) {
            Some(i2c_handle) => Ok(Arc::clone(i2c_handle)),
            None => Err(BoardError::I2CBusNotFound(name)),
        }
    }
    fn get_digital_interrupt_value(&self, pin: i32) -> Result<u32, BoardError> {
        let p = self.pins.iter().find(|p| p.pin() == pin);
        if let Some(p) = p {
            if !p.is_interrupt() {
                return Err(BoardError::GpioPinError(pin as u32, "not an interrupt"));
            }
            return Ok(p.get_event_count());
        }
        Err(BoardError::GpioPinError(pin as u32, "not configured"))
    }

    fn add_digital_interrupt_callback(
        &mut self,
        pin: i32,
        intr_type: InterruptType,
        callback: Option<unsafe extern "C" fn(_: *mut core::ffi::c_void)>,
        arg: Option<*mut core::ffi::c_void>,
    ) -> Result<(), BoardError> {
        let p = self
            .pins
            .iter_mut()
            .find(|p| p.pin() == pin)
            .ok_or_else(|| {
                BoardError::GpioPinError(
                    pin as u32,
                    "pin not found, failed to register interrupt callback",
                )
            })?;

        if callback.is_some() {
            p.setup_interrupt(
                intr_type.into(),
                callback,
                arg.unwrap_or_else(core::ptr::null_mut),
            )?;
        } else {
            let ptr = &mut p.event_count as *mut Arc<std::sync::atomic::AtomicU32> as *mut _;
            p.setup_interrupt(
                InterruptType::PosEdge.into(),
                Some(Esp32GPIOPin::default_interrupt),
                ptr,
            )?;
        }

        Ok(())
    }
}
