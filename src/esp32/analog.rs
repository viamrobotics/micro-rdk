#![allow(dead_code)]
use crate::common::analog::AnalogReader;
use core::cell::RefCell;
use esp_idf_hal::adc::{AdcChannelDriver, AdcDriver};
use esp_idf_hal::gpio::ADCPin;
use std::rc::Rc;

pub struct Esp32AnalogReader<'a, const A: u32, T: ADCPin> {
    channel: AdcChannelDriver<'a, A, T>,
    driver: Rc<RefCell<AdcDriver<'a, T::Adc>>>,
    name: String,
}

impl<'a, const A: u32, T: ADCPin> Esp32AnalogReader<'a, A, T> {
    pub fn new(
        name: String,
        channel: AdcChannelDriver<'a, A, T>,
        driver: Rc<RefCell<AdcDriver<'a, T::Adc>>>,
    ) -> Self {
        Self {
            name,
            channel,
            driver,
        }
    }
    fn inner_read(&mut self) -> anyhow::Result<u16> {
        self.driver
            .borrow_mut()
            .read(&mut self.channel)
            .map_err(|e| anyhow::anyhow!(format!("error while reading analog reader {e}")))
    }
    fn inner_name(&self) -> String {
        self.name.clone()
    }
}

impl<'a, const A: u32, T: ADCPin> AnalogReader<u16> for Esp32AnalogReader<'a, A, T> {
    type Error = anyhow::Error;
    fn read(&mut self) -> Result<u16, Self::Error> {
        self.inner_read()
    }
    fn name(&self) -> String {
        self.inner_name()
    }
}
