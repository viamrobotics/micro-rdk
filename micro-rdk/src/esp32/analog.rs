#![allow(dead_code)]
use crate::common::analog::{AnalogError, AnalogReader, AnalogResolution};
use crate::esp32::esp_idf_svc::hal::adc::oneshot::{AdcChannelDriver, AdcDriver};

// TODO(RSDK-10188): Update to ESP-IDF 5 ADC API
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
            .read(&mut self.channel)
            .map_err(|e| AnalogError::AnalogReadError(e.code()))
    }
    fn inner_name(&self) -> String {
        self.name.clone()
    }
}

impl<const A: u32, T: ADCPin> AnalogReader<u16> for Esp32AnalogReader<'_, A, T> {
    type Error = AnalogError;
    fn read(&mut self) -> Result<u16, Self::Error> {
        self.inner_read()
    }
    fn name(&self) -> String {
        self.inner_name()
    }
    fn resolution(&self) -> crate::common::analog::AnalogResolution {
        // TODO: In esp32/board.rs we have selected the ADC_ATTEN_DB_11 setting
        // for our ADC drivers, resulting in the max_range value below (see docs on ESP-IDF for more info).
        // If and when we make this configurable, this function should adjust accordingly
        // (NOTE: `AdcDriver::get_max_mv` in esp-idf-hal is private, so we will have to implement this ourselves
        // using esp-idf-sys directly)

        // ESP32 has a natively available function that actually converts the raw value into
        // millivolts and does not operate under the assumption of linear scaling. However,
        // the board API's resolution values are linearly parametrized, so we provide a min of 0
        // and step size of 1 so that any client code that uses the parameters will end up returning
        // the same value in mV returned by `read`
        AnalogResolution {
            min_range: 0.0,
            max_range: 2450.0,
            step_size: 1.0,
        }
    }
}
