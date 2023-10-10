#![allow(dead_code)]
use crate::common::analog::AnalogReader;
use crate::common::analog::AnalogReaderConfig;
use crate::common::board::Board;
use crate::common::board::BoardType;
use crate::common::config::ConfigType;
use crate::common::digital_interrupt::DigitalInterruptConfig;
use crate::common::i2c::I2cHandleType;
use crate::common::registry::ComponentRegistry;
use crate::common::status::Status;
use crate::google;
use crate::proto::common;
use crate::proto::component;

use anyhow::Context;
use core::cell::RefCell;
use esp_idf_hal::adc::config::Config;
use esp_idf_hal::adc::AdcChannelDriver;
use esp_idf_hal::adc::AdcDriver;
use esp_idf_hal::adc::Atten11dB;
use esp_idf_hal::adc::ADC1;
use esp_idf_hal::gpio::InterruptType;

use log::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::analog::Esp32AnalogReader;
use super::i2c::{Esp32I2C, Esp32I2cConfig};
use super::pin::Esp32GPIOPin;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_board("esp32", &EspBoard::from_config)
        .is_err()
    {
        log::error!("esp32 board type already registered");
    }
}

pub struct EspBoard {
    pins: Vec<Esp32GPIOPin>,
    analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
    i2cs: HashMap<String, I2cHandleType>,
}

impl EspBoard {
    pub fn new(
        pins: Vec<Esp32GPIOPin>,
        analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
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
    pub(crate) fn from_config(cfg: ConfigType) -> anyhow::Result<BoardType> {
        let (analogs, mut pins, i2c_confs) = {
            let analogs = if let Ok(analogs) =
                cfg.get_attribute::<Vec<AnalogReaderConfig>>("analogs")
            {
                let analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>> =
                    analogs
                        .iter()
                        .filter_map(|v| {
                            let adc1 = Rc::new(RefCell::new(
                                AdcDriver::new(
                                    unsafe { ADC1::new() },
                                    &Config::new().calibration(true),
                                )
                                .ok()?,
                            ));
                            let chan: Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>> =
                                match v.pin {
                                    32 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio32::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    33 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio33::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    34 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio34::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    35 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio35::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    36 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio36::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    37 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio37::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    38 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio38::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    39 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio39::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    _ => {
                                        log::error!("pin {} is not an ADC1 pin", v.pin);
                                        None
                                    }
                                }?;

                            Some(chan)
                        })
                        .collect();
                analogs
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
            let i2c = Esp32I2C::new_from_config(*conf)?;
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
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()> {
        let p = self.pins.iter_mut().find(|p| p.pin() == pin);
        if let Some(p) = p {
            if p.is_interrupt() {
                anyhow::bail!(
                    "cannot set level for pin {:?}, it is registered as an interrupt",
                    pin
                )
            }
            if is_high {
                return p.set_high();
            } else {
                return p.set_low();
            }
        }
        Err(anyhow::anyhow!("pin {} is not set as an output pin", pin))
    }
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool> {
        let pin = self
            .pins
            .iter()
            .find(|p| p.pin() == pin)
            .context(format!("pin {pin} not registered on board"))?;
        Ok(pin.is_high())
    }
    fn get_pwm_duty(&self, pin: i32) -> f64 {
        match self.pins.iter().find(|p| p.pin() == pin) {
            None => 0.0,
            Some(pin) => pin.get_pwm_duty(),
        }
    }
    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> anyhow::Result<()> {
        let pin = self
            .pins
            .iter_mut()
            .find(|p| p.pin() == pin)
            .context(format!("pin {pin} not registered on board"))?;
        pin.set_pwm_duty(duty_cycle_pct)
    }
    fn get_pwm_frequency(&self, pin: i32) -> anyhow::Result<u64> {
        let pin = self
            .pins
            .iter()
            .find(|p| p.pin() == pin)
            .context(format!("pin {pin} not registered on board"))?;
        Ok(pin.get_pwm_frequency())
    }
    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> anyhow::Result<()> {
        let pin = self
            .pins
            .iter_mut()
            .find(|p| p.pin() == pin)
            .context(format!("pin {pin} not registered on board"))?;
        pin.set_pwm_frequency(frequency_hz)
    }
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus> {
        let mut b = common::v1::BoardStatus {
            analogs: HashMap::new(),
            digital_interrupts: HashMap::new(),
        };
        self.analogs.iter().for_each(|a| {
            let mut borrowed = a.borrow_mut();
            b.analogs.insert(
                borrowed.name(),
                common::v1::AnalogStatus {
                    value: borrowed.read().unwrap_or(0).into(),
                },
            );
        });
        Ok(b)
    }
    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> anyhow::Result<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>> {
        match self.analogs.iter().find(|a| a.borrow().name() == name) {
            Some(reader) => Ok(reader.clone()),
            None => Err(anyhow::anyhow!("couldn't find analog reader {}", name)),
        }
    }
    fn set_power_mode(
        &self,
        mode: component::board::v1::PowerMode,
        duration: Option<Duration>,
    ) -> anyhow::Result<()> {
        info!(
            "Esp32 received request to set power mode to {} for {} milliseconds",
            mode.as_str_name(),
            match duration {
                Some(dur) => dur.as_millis().to_string(),
                None => "<forever>".to_string(),
            }
        );

        anyhow::ensure!(
            mode == component::board::v1::PowerMode::OfflineDeep,
            "unimplemented: EspBoard::set_power_mode: modes other than 'OfflineDeep' are not currently supported"
        );

        if let Some(dur) = duration {
            let dur_micros = dur.as_micros() as u64;
            let result: esp_idf_sys::esp_err_t;
            unsafe {
                result = esp_idf_sys::esp_sleep_enable_timer_wakeup(dur_micros);
            }
            anyhow::ensure!(
                result == esp_idf_sys::ESP_OK,
                "unimplemented: EspBoard::set_power_mode: sleep duration {:?} rejected as unsupportedly long", dur
            );
            warn!("Esp32 entering deep sleep for {} microseconds!", dur_micros);
        } else {
            warn!("Esp32 entering deep sleep without scheduled wakeup!");
        }

        unsafe {
            esp_idf_sys::esp_deep_sleep_start();
        }
    }
    fn get_i2c_by_name(&self, name: String) -> anyhow::Result<I2cHandleType> {
        match self.i2cs.get(&name) {
            Some(i2c_handle) => Ok(Arc::clone(i2c_handle)),
            None => Err(anyhow::anyhow!("no i2c found with name {}", name)),
        }
    }
    fn get_digital_interrupt_value(&self, pin: i32) -> anyhow::Result<u32> {
        let p = self.pins.iter().find(|p| p.pin() == pin);
        if let Some(p) = p {
            if !p.is_interrupt() {
                return Err(anyhow::anyhow!(
                    "pin {} is not configured as an interrupt",
                    pin
                ));
            }
            return Ok(p.get_event_count());
        }
        Err(anyhow::anyhow!(
            "pin {} is not configured on the board instance",
            pin
        ))
    }
}

impl Status for EspBoard {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        let mut analogs = HashMap::new();
        self.analogs.iter().for_each(|a| {
            let mut borrowed = a.borrow_mut();
            analogs.insert(
                borrowed.name(),
                google::protobuf::Value {
                    kind: Some(google::protobuf::value::Kind::StructValue(
                        google::protobuf::Struct {
                            fields: HashMap::from([(
                                "value".to_string(),
                                google::protobuf::Value {
                                    kind: Some(google::protobuf::value::Kind::NumberValue(
                                        borrowed.read().unwrap_or(0).into(),
                                    )),
                                },
                            )]),
                        },
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
