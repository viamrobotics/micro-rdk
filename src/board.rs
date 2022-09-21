#![allow(dead_code)]
use crate::proto::common;
use crate::status::Status;
use embedded_hal::digital::v2::OutputPin;
use esp_idf_hal::gpio::*;
use log::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::Mutex;

pub struct FakeBoard {
    adc_val: i32,
}
pub trait Board: Status {
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()>;
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus>;
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool>;
}

impl FakeBoard {
    pub fn new() -> Self {
        FakeBoard { adc_val: 10 }
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
        b.analogs
            .insert("A1".to_string(), common::v1::AnalogStatus { value: 0x0 });
        Ok(b) //component::board::v1::StatusResponse { status: Some(b) }
    }
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool> {
        info!("get pin {}", pin);
        Ok(true)
    }
}

impl Status for FakeBoard {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let anaval = prost_types::Value {
            kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                fields: BTreeMap::from([(
                    "value".to_string(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::NumberValue(10.0)),
                    },
                )]),
            })),
        };
        bt.insert(
            "analogs".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                    fields: BTreeMap::from([("A1".to_string(), anaval)]),
                })),
            },
        );

        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

pub struct EspBoard<Pins> {
    pins: Vec<Pins>,
}

impl<Pins> EspBoard<Pins>
where
    Pins: OutputPin + Pin,
{
    pub fn new(pins: Vec<Pins>) -> Self {
        EspBoard { pins }
    }
}

impl<Pins> Board for EspBoard<Pins>
where
    Pins: OutputPin + Pin,
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
    fn get_gpio_level(&self, _: i32) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus> {
        let mut b = common::v1::BoardStatus {
            analogs: HashMap::new(),
            digital_interrupts: HashMap::new(),
        };
        b.analogs
            .insert("A1".to_string(), common::v1::AnalogStatus { value: 0x0 });
        Ok(b) //component::board::v1::StatusResponse { status: Some(b) }
    }
}
impl<Pins> Status for EspBoard<Pins>
where
    Pins: OutputPin + Pin,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let anaval = prost_types::Value {
            kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                fields: BTreeMap::from([(
                    "value".to_string(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::NumberValue(10.0)),
                    },
                )]),
            })),
        };
        bt.insert(
            "analogs".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                    fields: BTreeMap::from([("A1".to_string(), anaval)]),
                })),
            },
        );
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
}
