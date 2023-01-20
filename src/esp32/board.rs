#![allow(dead_code)]
use super::pin::PinExt;
use crate::common::analog::AnalogReader;
use crate::common::board::Board;
use crate::common::status::Status;
use crate::proto::common;
use core::cell::RefCell;
use embedded_hal::digital::v2::StatefulOutputPin;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
pub struct EspBoard<Pins> {
    pins: Vec<Pins>,
    analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
}

impl<Pins> EspBoard<Pins>
where
    Pins: StatefulOutputPin + PinExt,
{
    pub fn new(
        pins: Vec<Pins>,
        analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
    ) -> Self {
        EspBoard { pins, analogs }
    }
}

impl<Pins> Board for EspBoard<Pins>
where
    Pins: StatefulOutputPin + PinExt,
{
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()> {
        if let Some(pin) = self.pins.iter_mut().find(|x| x.pin() == pin) {
            if is_high {
                pin.set_high()
                    .map_err(|_| anyhow::anyhow!("error setting pin {} high", pin.pin()))?;
            } else {
                pin.set_low()
                    .map_err(|_| anyhow::anyhow!("error setting pin {} low", pin.pin()))?;
            }
        }
        Ok(())
    }
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool> {
        if let Some(pin) = self.pins.iter().find(|x| x.pin() == pin) {
            return pin
                .is_set_high()
                .map_err(|_| anyhow::anyhow!("error getting pin {}", pin.pin()));
        }
        anyhow::bail!("pin {} not found", pin)
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
}
impl<Pins> Status for EspBoard<Pins>
where
    Pins: StatefulOutputPin + PinExt,
{
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
