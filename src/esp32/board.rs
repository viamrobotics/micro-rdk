#![allow(dead_code)]
use super::pin::PinExt;
use crate::common::analog::AnalogReader;
use crate::common::board::Board;
use crate::common::status::Status;
use crate::proto::common;
use crate::proto::component;
use core::cell::RefCell;
use embedded_hal::digital::v2::StatefulOutputPin;
use log::*;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::time::Duration;

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

        // The esp_deep_sleep_start function above is documented to
        // not return. If we have somehow proceeded past it, then the
        // request has failed.

        anyhow::bail!(
            "call to esp_deep_sleep_start returned - board failed to honor power mode request"
        );
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
