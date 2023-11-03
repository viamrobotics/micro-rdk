//! Abstraction of a general-purpose compute board

#![allow(dead_code)]
use crate::{
    common::{
        analog::AnalogReader,
        status::Status,
    },
    google,
    proto::{
        common, 
        component::board::v1::PowerMode
    },
};

use log::*;
use core::cell::RefCell;
use std::{
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    sync::Mutex,
    time::Duration,
};

use super::{
    analog::FakeAnalogReader,
    config::ConfigType,
    i2c::{
        FakeI2CHandle, 
        FakeI2cConfig, 
        I2CHandle, 
        I2cHandleType},
    registry::ComponentRegistry,
};

pub static COMPONENT_NAME: &str = "board";

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_board("fake", &FakeBoard::from_config)
        .is_err()
    {
        log::error!("model fake is already registered")
    }
}

/// Represents the functionality of a general purpose compute board that contains various components such as analog readers and digital interrupts.
pub trait Board: Status {
    /// Set a pin to high or low
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()>;

    /// Return the current [BoardStatus] of the board
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus>;

    /// Get the state of a pin, high(`true`) or low(`false`)
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool>;

    /// Get an [AnalogReader] by name
    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> anyhow::Result<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>;

    /// Set the board to the indicated [PowerMode]
    fn set_power_mode(
        &self,
        mode: PowerMode,
        duration: Option<Duration>,
    ) -> anyhow::Result<()>;

    fn get_i2c_by_name(&self, name: String) -> anyhow::Result<I2cHandleType>;

    /// Return the amount of detected interrupt events on a pin. Should error if the
    /// pin has not been configured as an interrupt
    fn get_digital_interrupt_value(&self, _pin: i32) -> anyhow::Result<u32> {
        anyhow::bail!("this board does not support digital interrupts")
    }

    /// Get the pin's given duty cycle, returns percentage as float between 0.0 and 1.0
    fn get_pwm_duty(&self, pin: i32) -> f64;

    /// Set the pin to the given duty cycle , `duty_cycle_pct` is a float between 0.0 and 1.0
    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> anyhow::Result<()>;

    /// Get the PWM frequency of the pin
    fn get_pwm_frequency(&self, pin: i32) -> anyhow::Result<u64>;

    /// Set the pin to the given PWM frequency (in Hz). 
    /// When frequency is 0, the board will unregister the pin and PWM signal
    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> anyhow::Result<()>;
}

/// An alias for a thread-safe handle to a struct that implements the [Board] trait
pub type BoardType = Arc<Mutex<dyn Board>>;

/// A test implementation of a generic compute board
pub struct FakeBoard {
    analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
    i2cs: HashMap<String, Arc<Mutex<FakeI2CHandle>>>,
    pin_pwms: HashMap<i32, f64>,
    pin_pwm_freq: HashMap<i32, u64>,
}

impl FakeBoard {
    pub fn new(analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>) -> Self {
        let mut i2cs: HashMap<String, Arc<Mutex<FakeI2CHandle>>> = HashMap::new();
        let i2c0 = Arc::new(Mutex::new(FakeI2CHandle::new("i2c0".to_string())));
        i2cs.insert(i2c0.name(), i2c0);
        let i2c1 = Arc::new(Mutex::new(FakeI2CHandle::new("i2c1".to_string())));
        i2cs.insert(i2c1.name(), i2c1);
        FakeBoard {
            analogs,
            i2cs,
            pin_pwms: HashMap::new(),
            pin_pwm_freq: HashMap::new(),
        }
    }

    pub(crate) fn from_config(cfg: ConfigType) -> anyhow::Result<BoardType> {
        let analogs = if let Ok(analog_confs) = cfg.get_attribute::<HashMap<&str, f64>>("analogs") {
            analog_confs
                .iter()
                .map(|(k, v)| {
                    let a: Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>> = Rc::new(
                        RefCell::new(FakeAnalogReader::new(k.to_string(), *v as u16)),
                    );
                    a
                })
                .collect()
        } else {
            vec![]
        };

        let i2cs = if let Ok(i2c_confs) = cfg.get_attribute::<Vec<FakeI2cConfig>>("i2cs") {
            let name_to_i2c = i2c_confs.iter().map(|v| {
                let name = v.name.to_string();
                let value: [u8; 3] = [v.value_1, v.value_2, v.value_3];
                (
                    name.to_string(),
                    Arc::new(Mutex::new(FakeI2CHandle::new_with_value(name, value))),
                )
            });
            HashMap::from_iter(name_to_i2c)
        } else {
            HashMap::new()
        };

        Ok(Arc::new(Mutex::new(FakeBoard {
            analogs,
            i2cs,
            pin_pwms: HashMap::new(),
            pin_pwm_freq: HashMap::new(),
        })))
    }
}

impl Board for FakeBoard {
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()> {
        info!("set pin {} to {}", pin, is_high);
        Ok(())
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

    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool> {
        info!("get pin {}", pin);
        Ok(true)
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
        mode: PowerMode,
        duration: Option<Duration>,
    ) -> anyhow::Result<()> {
        info!(
            "set power mode to {} for {} milliseconds",
            mode.as_str_name(),
            match duration {
                Some(dur) => dur.as_millis().to_string(),
                None => "<forever>".to_string(),
            }
        );
        Ok(())
    }

    fn get_i2c_by_name(&self, name: String) -> anyhow::Result<I2cHandleType> {
        if let Some(i2c_handle) = self.i2cs.get(&name) {
            Ok((*i2c_handle).clone())
        } else {
            anyhow::bail!("could not find I2C with name {}", name)
        }
    }

    fn get_pwm_duty(&self, pin: i32) -> f64 {
        *self.pin_pwms.get(&pin).unwrap_or(&0.0)
    }

    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> anyhow::Result<()> {
        self.pin_pwms.insert(pin, duty_cycle_pct);
        Ok(())
    }

    fn get_pwm_frequency(&self, pin: i32) -> anyhow::Result<u64> {
        Ok(*self.pin_pwm_freq.get(&pin).unwrap_or(&0))
    }

    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> anyhow::Result<()> {
        self.pin_pwm_freq.insert(pin, frequency_hz);
        Ok(())
    }
}

impl Status for FakeBoard {
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

impl<A> Board for Arc<Mutex<A>>
where
    A: ?Sized + Board,
{
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus> {
        self.lock().unwrap().get_board_status()
    }

    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool> {
        self.lock().unwrap().get_gpio_level(pin)
    }

    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()> {
        self.lock().unwrap().set_gpio_pin_level(pin, is_high)
    }

    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> anyhow::Result<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>> {
        self.lock().unwrap().get_analog_reader_by_name(name)
    }

    fn set_power_mode(
        &self,
        mode: PowerMode,
        duration: Option<Duration>,
    ) -> anyhow::Result<()> {
        self.lock().unwrap().set_power_mode(mode, duration)
    }

    fn get_i2c_by_name(&self, name: String) -> anyhow::Result<I2cHandleType> {
        self.lock().unwrap().get_i2c_by_name(name)
    }

    fn get_digital_interrupt_value(&self, pin: i32) -> anyhow::Result<u32> {
        self.lock().unwrap().get_digital_interrupt_value(pin)
    }

    fn get_pwm_duty(&self, pin: i32) -> f64 {
        self.lock().unwrap().get_pwm_duty(pin)
    }

    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> anyhow::Result<()> {
        self.lock().unwrap().set_pwm_duty(pin, duty_cycle_pct)
    }

    fn get_pwm_frequency(&self, pin: i32) -> anyhow::Result<u64> {
        self.lock().unwrap().get_pwm_frequency(pin)
    }

    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> anyhow::Result<()> {
        self.lock().unwrap().set_pwm_frequency(pin, frequency_hz)
    }
}
