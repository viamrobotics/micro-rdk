#![allow(dead_code)]
use crate::common::analog::AnalogReader;
use core::cell::RefCell;
use esp_idf_hal::adc::Adc;
use esp_idf_hal::adc::{AdcChannelDriver, AdcDriver, Attenuation};
use esp_idf_hal::gpio::ADCPin;
use esp_idf_sys::{adc1_get_raw, adc_channel_t, adc_unit_t_ADC_UNIT_1};
use std::rc::Rc;

pub struct Esp32RawAdcChannel(pub adc_channel_t);

pub struct Esp32AnalogReader<'a, T: ADCPin, ATTEN> {
    channel: AdcChannelDriver<'a, T, ATTEN>,
    driver: Rc<RefCell<AdcDriver<'a, T::Adc>>>,
    name: String,
    pin: Esp32RawAdcChannel,
}
impl<'a, T: ADCPin, ATTEN> Esp32AnalogReader<'a, T, ATTEN>
where
    ATTEN: Attenuation<T::Adc>,
{
    pub fn new(
        name: String,
        channel: AdcChannelDriver<'a, T, ATTEN>,
        driver: Rc<RefCell<AdcDriver<'a, T::Adc>>>,
        pin: Esp32RawAdcChannel,
    ) -> Self {
        Self {
            name,
            channel,
            driver,
            pin,
        }
    }
    fn inner_read(&mut self) -> anyhow::Result<u16> {
        self.raw_reading()
            .map_err(|e| anyhow::anyhow!(format!("error while reading analog reader {e}")))
    }
    fn inner_name(&self) -> String {
        self.name.clone()
    }
    fn raw_reading(&mut self) -> anyhow::Result<u16> {
        let unit = T::Adc::unit();
        let channel = self.pin.0;
        if unit == adc_unit_t_ADC_UNIT_1 {
            let measurement = unsafe { adc1_get_raw(channel) };
            if measurement < 0 {
                return Err(anyhow::anyhow!("failed to read channel {}", channel));
            }
            return Ok(measurement as u16);
        }
        Err(anyhow::anyhow!(
            "only ADC1 is supported with raw measurement"
        ))
    }
}

impl<'a, T: ADCPin, ATTEN> AnalogReader<u16> for Esp32AnalogReader<'a, T, ATTEN>
where
    ATTEN: Attenuation<T::Adc>,
{
    type Error = anyhow::Error;
    fn read(&mut self) -> Result<u16, Self::Error> {
        self.inner_read()
    }
    fn name(&self) -> String {
        self.inner_name()
    }
}
