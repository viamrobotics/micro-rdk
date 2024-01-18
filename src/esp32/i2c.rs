#![allow(dead_code)]

use crate::common::config::{AttributeError, Kind};
use crate::common::i2c::I2CHandle;
use crate::esp_idf_svc::hal::delay::BLOCK;
use crate::esp_idf_svc::hal::gpio::AnyIOPin;
use crate::esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver, I2C0, I2C1};
use crate::esp_idf_svc::hal::units::Hertz;

#[derive(Clone, Debug)]
pub struct Esp32I2cConfig {
    pub name: String,
    pub bus: String,
    pub baudrate_hz: u32,
    pub timeout_ns: u32,
    pub data_pin: i32,
    pub clock_pin: i32,
}

impl From<&Esp32I2cConfig> for I2cConfig {
    fn from(value: &Esp32I2cConfig) -> I2cConfig {
        // TODO: when next version of esp_idf_hal is released, use below instead
        // of storing timeout on Esp32I2C struct
        // let config = I2cConfig::new().baudrate(Hertz(value.baudrate_hz));
        // if value.timeout_ns != 0 {
        //     config = config.timeout(value.timeout_ns.into());
        // }
        // config
        I2cConfig::new().baudrate(Hertz(value.baudrate_hz))
    }
}

impl TryFrom<&Kind> for Esp32I2cConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("name")? {
            return Err(AttributeError::KeyNotFound("name".to_string()));
        }
        let name = value.get("name")?.unwrap().try_into()?;
        if !value.contains_key("bus")? {
            return Err(AttributeError::KeyNotFound("bus".to_string()));
        }
        let bus = value.get("bus")?.unwrap().try_into()?;
        let mut data_pin = 11;
        if value.contains_key("data_pin")? {
            data_pin = value.get("data_pin")?.unwrap().try_into()?;
        }
        let mut clock_pin = 6;
        if value.contains_key("clock_pin")? {
            clock_pin = value.get("clock_pin")?.unwrap().try_into()?;
        }
        let mut baudrate_hz: u32 = 1000000;
        if value.contains_key("baudrate_hz")? {
            baudrate_hz = value.get("baudrate")?.unwrap().try_into()?;
        }
        let mut timeout_ns: u32 = 0;
        if value.contains_key("timeout_ns")? {
            timeout_ns = value.get("timeout_ns")?.unwrap().try_into()?;
        }
        Ok(Self {
            name,
            bus,
            baudrate_hz,
            timeout_ns,
            data_pin,
            clock_pin,
        })
    }
}

pub struct Esp32I2C<'a> {
    name: String,
    driver: I2cDriver<'a>,
    timeout_ns: u32,
}

impl<'a> Esp32I2C<'a> {
    pub fn new_from_config(conf: &Esp32I2cConfig) -> anyhow::Result<Self> {
        let name = conf.name.to_string();
        let timeout_ns = conf.timeout_ns;
        let sda = unsafe { AnyIOPin::new(conf.data_pin) };
        let scl = unsafe { AnyIOPin::new(conf.clock_pin) };
        let driver_conf = I2cConfig::from(conf);

        match conf.bus.as_str() {
            "i2c0" => {
                let i2c0 = unsafe { I2C0::new() };
                let driver = I2cDriver::new(i2c0, sda, scl, &driver_conf)?;
                Ok(Esp32I2C {
                    name,
                    driver,
                    timeout_ns,
                })
            }
            "i2c1" => {
                let i2c1 = unsafe { I2C1::new() };
                let driver = I2cDriver::new(i2c1, sda, scl, &driver_conf)?;
                Ok(Esp32I2C {
                    name,
                    driver,
                    timeout_ns,
                })
            }
            _ => anyhow::bail!("only i2c0 or i2c1 supported, i2c bus must match either value"),
        }
    }
}

impl<'a> I2CHandle for Esp32I2C<'a> {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn read_i2c(&mut self, address: u8, buffer: &mut [u8]) -> anyhow::Result<()> {
        match self.driver.read(address, buffer, BLOCK) {
            Ok(()) => Ok(()),
            Err(err) => anyhow::bail!("ESP32 read_i2c failed for i2c {}: {}", self.name, err),
        }
    }

    fn write_i2c(&mut self, address: u8, bytes: &[u8]) -> anyhow::Result<()> {
        match self.driver.write(address, bytes, BLOCK) {
            Ok(()) => Ok(()),
            Err(err) => anyhow::bail!("ESP32 write_i2c failed for i2c {}: {}", self.name, err),
        }
    }

    fn write_read_i2c(
        &mut self,
        address: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> anyhow::Result<()> {
        match self.driver.write_read(address, bytes, buffer, BLOCK) {
            Ok(()) => Ok(()),
            Err(err) => anyhow::bail!("ESP32 write_read_i2c failed for i2c {}: {}", self.name, err),
        }
    }
}
