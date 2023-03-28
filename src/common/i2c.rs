#![allow(dead_code)]

use super::config::{AttributeError, Kind};
use std::sync::{Arc, Mutex};

// A trait representing blocking I2C communication for a board. TODO: replace with the
// embedded_hal I2C trait when supporting boards beyond ESP32. AddressType is
// either u8 (indicating support for 7-bit addresses) or u16 (for supporting 10-bit addresses)
pub trait I2CHandle<AddressType> {
    fn name(&self) -> String;

    fn read_i2c(&mut self, _address: AddressType, _buffer: &mut [u8]) -> anyhow::Result<()> {
        anyhow::bail!("read_i2c unimplemented")
    }

    fn write_i2c(&mut self, _address: AddressType, _bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("write_i2c unimplemented")
    }
}

pub(crate) type I2cHandleType = Arc<Mutex<dyn I2CHandle<u8>>>;

#[derive(Debug)]
pub(crate) struct FakeI2cConfig {
    pub(crate) name: &'static str,
    pub(crate) value_1: u8,
    pub(crate) value_2: u8,
    pub(crate) value_3: u8,
}

impl TryFrom<Kind> for FakeI2cConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StructValueStatic(v) => {
                if !v.contains_key("name") {
                    return Err(AttributeError::KeyNotFound);
                }
                let name = v.get("name").unwrap().try_into()?;
                let value_1 = match v.get("value_1") {
                    Some(val) => val.try_into()?,
                    None => 0,
                };
                let value_2 = match v.get("value_2") {
                    Some(val) => val.try_into()?,
                    None => 0,
                };
                let value_3 = match v.get("value_3") {
                    Some(val) => val.try_into()?,
                    None => 0,
                };
                Ok(FakeI2cConfig {
                    name,
                    value_1,
                    value_2,
                    value_3,
                })
            }
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

impl TryFrom<&Kind> for FakeI2cConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        match value {
            Kind::StructValueStatic(v) => {
                if !v.contains_key("name") {
                    return Err(AttributeError::KeyNotFound);
                }
                let name = v.get("name").unwrap().try_into()?;
                let value_1 = match v.get("value_1") {
                    Some(val) => val.try_into()?,
                    None => 0,
                };
                let value_2 = match v.get("value_2") {
                    Some(val) => val.try_into()?,
                    None => 0,
                };
                let value_3 = match v.get("value_3") {
                    Some(val) => val.try_into()?,
                    None => 0,
                };
                Ok(FakeI2cConfig {
                    name,
                    value_1,
                    value_2,
                    value_3,
                })
            }
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FakeI2CHandle {
    name: String,
    value: [u8; 3],
}

impl FakeI2CHandle {
    pub fn new(name: String) -> Self {
        FakeI2CHandle {
            name,
            value: [0, 0, 0],
        }
    }

    pub fn new_with_value(name: String, value: [u8; 3]) -> Self {
        FakeI2CHandle { name, value }
    }
}

impl I2CHandle<u8> for FakeI2CHandle {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn read_i2c(&mut self, _address: u8, buffer: &mut [u8]) -> anyhow::Result<()> {
        for (i, x) in self.value.iter().enumerate() {
            if i < buffer.len() {
                buffer[i] = *x;
            }
        }
        anyhow::Ok(())
    }

    fn write_i2c(&mut self, _address: u8, bytes: &[u8]) -> anyhow::Result<()> {
        for (i, x) in bytes.iter().enumerate() {
            self.value[i] = *x;
        }
        anyhow::Ok(())
    }
}

impl<A> I2CHandle<u8> for Arc<Mutex<A>>
where
    A: ?Sized + I2CHandle<u8>,
{
    fn name(&self) -> String {
        self.lock().unwrap().name()
    }

    fn read_i2c(&mut self, address: u8, buffer: &mut [u8]) -> anyhow::Result<()> {
        self.lock().unwrap().read_i2c(address, buffer)
    }

    fn write_i2c(&mut self, address: u8, bytes: &[u8]) -> anyhow::Result<()> {
        self.lock().unwrap().write_i2c(address, bytes)
    }
}

impl<A> I2CHandle<u16> for Arc<Mutex<A>>
where
    A: ?Sized + I2CHandle<u16>,
{
    fn name(&self) -> String {
        self.lock().unwrap().name()
    }

    fn read_i2c(&mut self, address: u16, buffer: &mut [u8]) -> anyhow::Result<()> {
        self.lock().unwrap().read_i2c(address, buffer)
    }

    fn write_i2c(&mut self, address: u16, bytes: &[u8]) -> anyhow::Result<()> {
        self.lock().unwrap().write_i2c(address, bytes)
    }
}
