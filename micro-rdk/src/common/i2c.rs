#![allow(dead_code)]

use super::config::{AttributeError, Kind};
use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum I2CErrors {
    #[error("invalid argument: {0}")]
    I2CInvalidArgument(&'static str),
    #[error("ic2 bus {0} read error {1}")]
    I2CReadError(String, i32),
    #[error("ic2 bus {0} write error {1}")]
    I2CWriteError(String, i32),
    #[error("ic2 bus {0} read_write error {1}")]
    I2CReadWriteError(String, i32),
    #[error("{0} unimplemented")]
    I2CUnimplemented(&'static str),
    #[error(transparent)]
    I2COtherError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// A trait representing blocking I2C communication for a board. TODO: replace with the
// embedded_hal I2C trait when supporting boards beyond ESP32.
pub trait I2CHandle {
    fn name(&self) -> String;

    fn read_i2c(&mut self, _address: u8, _buffer: &mut [u8]) -> Result<(), I2CErrors> {
        Err(I2CErrors::I2CUnimplemented("read_i2c"))
    }

    fn write_i2c(&mut self, _address: u8, _bytes: &[u8]) -> Result<(), I2CErrors> {
        Err(I2CErrors::I2CUnimplemented("write_i2c"))
    }

    // a transactional write and subsequent read action
    fn write_read_i2c(
        &mut self,
        _address: u8,
        _bytes: &[u8],
        _buffer: &mut [u8],
    ) -> Result<(), I2CErrors> {
        Err(I2CErrors::I2CUnimplemented("write_read_i2c"))
    }
}

pub type I2cHandleType = Arc<Mutex<dyn I2CHandle + Send>>;

#[derive(Debug)]
pub(crate) struct FakeI2cConfig<'a> {
    pub(crate) name: &'a str,
    pub(crate) value_1: u8,
    pub(crate) value_2: u8,
    pub(crate) value_3: u8,
}

impl<'a> TryFrom<&'a Kind> for FakeI2cConfig<'a> {
    type Error = AttributeError;
    fn try_from(value: &'a Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("name")? {
            return Err(AttributeError::KeyNotFound("name".to_string()));
        }
        let name = value.get("name")?.unwrap().try_into()?;
        let value_1 = match value.get("value_1")? {
            Some(val) => val.try_into()?,
            None => 0,
        };
        let value_2 = match value.get("value_2")? {
            Some(val) => val.try_into()?,
            None => 0,
        };
        let value_3 = match value.get("value_3")? {
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

impl I2CHandle for FakeI2CHandle {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn read_i2c(&mut self, _address: u8, buffer: &mut [u8]) -> Result<(), I2CErrors> {
        for (i, x) in self.value.iter().enumerate() {
            if i < buffer.len() {
                buffer[i] = *x;
            }
        }
        Ok(())
    }

    fn write_i2c(&mut self, _address: u8, bytes: &[u8]) -> Result<(), I2CErrors> {
        for (i, x) in bytes.iter().enumerate() {
            self.value[i] = *x;
        }
        Ok(())
    }
}

impl<A> I2CHandle for Arc<Mutex<A>>
where
    A: ?Sized + I2CHandle,
{
    fn name(&self) -> String {
        self.lock().unwrap().name()
    }

    fn read_i2c(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), I2CErrors> {
        self.lock().unwrap().read_i2c(address, buffer)
    }

    fn write_i2c(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2CErrors> {
        self.lock().unwrap().write_i2c(address, bytes)
    }

    fn write_read_i2c(
        &mut self,
        address: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), I2CErrors> {
        self.lock().unwrap().write_read_i2c(address, bytes, buffer)
    }
}
