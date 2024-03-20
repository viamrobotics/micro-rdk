#![allow(dead_code)]
use crate::common::analog::{AnalogError, AnalogReader};
use crate::esp32::esp_idf_svc::hal::adc::{AdcChannelDriver, AdcDriver};
use crate::esp32::esp_idf_svc::hal::gpio::ADCPin;
use std::sync::{Arc, Mutex};

pub struct Esp32AnalogReader<'a, const A: u32, T: ADCPin> {
    channel: AdcChannelDriver<'a, A, T>,
    driver: Arc<Mutex<AdcDriver<'a, T::Adc>>>,
    name: String,
}

impl<'a, const A: u32, T: ADCPin> Esp32AnalogReader<'a, A, T> {
    pub fn new(
        name: String,
        channel: AdcChannelDriver<'a, A, T>,
        driver: Arc<Mutex<AdcDriver<'a, T::Adc>>>,
    ) -> Self {
        Self {
            name,
            channel,
            driver,
        }
    }
    fn inner_read(&mut self) -> Result<u16, AnalogError> {
        self.driver
            .lock()
            .unwrap()
            .read_raw(&mut self.channel)
            .map_err(|e| AnalogError::AnalogReadError(e.code()))
    }
    fn inner_name(&self) -> String {
        self.name.clone()
    }
}

impl<'a, const A: u32, T: ADCPin> AnalogReader<u16> for Esp32AnalogReader<'a, A, T> {
    type Error = AnalogError;
    fn read(&mut self) -> Result<u16, Self::Error> {
        self.inner_read()
    }
    fn name(&self) -> String {
        self.inner_name()
    }
}
