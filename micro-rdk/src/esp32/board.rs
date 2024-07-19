#![allow(dead_code)]
use log::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    common::{
        analog::{AnalogReader, AnalogReaderConfig, AnalogReaderType},
        board::{Board, BoardError, BoardType},
        config::ConfigType,
        digital_interrupt::DigitalInterruptConfig,
        i2c::I2cHandleType,
        registry::ComponentRegistry,
        status::{Status, StatusError},
    },
    google,
    proto::component,
};

use super::{
    analog::Esp32AnalogReader,
    i2c::{Esp32I2C, Esp32I2cConfig},
    pin::Esp32GPIOPin,
};

use crate::esp32::esp_idf_svc::hal::{
    adc::{
        attenuation::adc_atten_t_ADC_ATTEN_DB_11 as Atten11dB, config::Config, AdcChannelDriver,
        AdcDriver, ADC1,
    },
    gpio::InterruptType,
};

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
        let (analogs, mut pins, i2c_confs) = {
            let analogs =
                if let Ok(analogs) = cfg.get_attribute::<Vec<AnalogReaderConfig>>("analogs") {
                    let analogs: Result<Vec<AnalogReaderType<u16>>, BoardError> = analogs
                        .iter()
                        .map(|v| {
                            let adc1 = Arc::new(Mutex::new(AdcDriver::new(
                                unsafe { ADC1::new() },
                                &Config::new().calibration(true),
                            )?));
                            let chan: Result<AnalogReaderType<u16>, BoardError> = match v.pin {
                                32 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio32::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                33 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio33::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                34 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio34::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                35 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio35::new()
                                            })
                                            .map_err(BoardError::EspError(v.pin as u32, "Unable to make Adc Channel Driver with Pin",))?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                36 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio36::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                37 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio37::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                38 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio38::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
                                        )));
                                    Ok(p)
                                }
                                39 => {
                                    let p: AnalogReaderType<u16> =
                                        Arc::new(Mutex::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<Atten11dB, _>::new(unsafe {
                                                crate::esp32::esp_idf_svc::hal::gpio::Gpio39::new()
                                            })
                                            .map_err(BoardError::EspError)?,
                                            adc1,
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
            let pins = if let Ok(pins) = cfg.get_attribute::<Vec<i32>>("pins") {
                pins.iter()
                    .filter_map(|pin| {
                        let p = Esp32GPIOPin::new(*pin, None);
                        if let Ok(p) = p {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                vec![]
            };

            let i2c_confs = if let Ok(i2c_confs) = cfg.get_attribute::<Vec<Esp32I2cConfig>>("i2cs")
            {
                i2c_confs
            } else {
                vec![]
            };
            (analogs, pins, i2c_confs)
        };
        let mut i2cs = HashMap::new();
        for conf in i2c_confs.iter() {
            let name = conf.name.to_string();
            let i2c = Esp32I2C::new_from_config(conf)?;
            let i2c_wrapped: I2cHandleType = Arc::new(Mutex::new(i2c));
            i2cs.insert(name.to_string(), i2c_wrapped);
        }
        if let Ok(interrupt_confs) =
            cfg.get_attribute::<Vec<DigitalInterruptConfig>>("digital_interrupts")
        {
            for conf in interrupt_confs {
                let p = pins.iter_mut().find(|p| p.pin() == conf.pin);
                if let Some(p) = p {
                    // RSDK-4763: make event type configurable
                    // https://viam.atlassian.net/browse/RSDK-4763
                    p.setup_interrupt(InterruptType::PosEdge)?
                } else {
                    let mut p = Esp32GPIOPin::new(conf.pin, None)?;
                    p.setup_interrupt(InterruptType::PosEdge)?;
                    pins.push(p);
                }
            }
        }
        Ok(Arc::new(Mutex::new(Self {
            pins,
            analogs,
            i2cs,
        })))
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
}

impl Status for EspBoard {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        let mut hm = HashMap::new();
        let mut analogs = HashMap::new();
        self.analogs.iter().for_each(|a| {
            let mut analog = a.clone();
            analogs.insert(
                analog.name(),
                google::protobuf::Value {
                    kind: Some(google::protobuf::value::Kind::NumberValue(
                        analog.read().unwrap_or(0).into(),
                    )),
                },
            );
        });
        if !analogs.is_empty() {
            hm.insert(
                "analogs".to_string(),
                google::protobuf::Value {
                    kind: Some(google::protobuf::value::Kind::StructValue(
                        google::protobuf::Struct { fields: analogs },
                    )),
                },
            );
        }
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}
