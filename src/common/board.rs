#![allow(dead_code)]
use crate::common::analog::AnalogReader;
use crate::common::status::Status;
use crate::proto::common;
use core::cell::RefCell;
use log::*;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use super::analog::FakeAnalogReader;
use super::config::{Component, ConfigType};
use super::registry::ComponentRegistry;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_board("fake", &FakeBoard::from_config)
        .is_err()
    {
        log::error!("model fake is already registered")
    }
}

pub struct FakeBoard {
    analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
}
pub trait Board: Status {
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()>;
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus>;
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool>;
    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> anyhow::Result<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>;
}

pub(crate) type BoardType = Arc<Mutex<dyn Board>>;

impl FakeBoard {
    pub fn new(analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>) -> Self {
        FakeBoard { analogs }
    }
    pub(crate) fn from_config(cfg: ConfigType) -> anyhow::Result<BoardType> {
        match cfg {
            ConfigType::Static(cfg) => {
                if let Ok(analogs) = cfg.get_attribute::<BTreeMap<&'static str, f64>>("analogs") {
                    let analogs = analogs
                        .iter()
                        .map(|(k, v)| {
                            let a: Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>> =
                                Rc::new(RefCell::new(FakeAnalogReader::new(
                                    k.to_string(),
                                    *v as u16,
                                )));
                            a
                        })
                        .collect();
                    return Ok(Arc::new(Mutex::new(FakeBoard { analogs })));
                }
            }
        };
        Ok(Arc::new(Mutex::new(FakeBoard::new(Vec::new()))))
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
        Ok(b) //component::board::v1::StatusResponse { status: Some(b) }
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
}

impl Status for FakeBoard {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let mut analogs = BTreeMap::new();
        self.analogs.iter().for_each(|a| {
            let mut borrowed = a.borrow_mut();
            analogs.insert(
                borrowed.name(),
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                        fields: BTreeMap::from([(
                            "value".to_string(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::NumberValue(
                                    borrowed.read().unwrap_or(0).into(),
                                )),
                            },
                        )]),
                    })),
                },
            );
        });
        if !analogs.is_empty() {
            bt.insert(
                "analogs".to_string(),
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                        fields: analogs,
                    })),
                },
            );
        }
        Ok(Some(prost_types::Struct { fields: bt }))
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
}
