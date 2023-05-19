#![allow(dead_code)]
use crate::common::i2c::I2cHandleType;
use crate::common::math_utils::Vector3;
use crate::common::movement_sensor::{MovementSensor, MovementSensorSupportedMethods};

use super::board::{Board, BoardType};
use super::config::Kind::BoolValue;
use super::config::{Component, ConfigType};
use super::i2c::I2CHandle;
use super::movement_sensor::MovementSensorType;
use super::registry::ComponentRegistry;
use super::status::Status;

use std::collections::BTreeMap;
use std::mem::size_of;
use std::sync::{Arc, Mutex};

// This module represents an implementation of the MPU-6050 gyroscope/accelerometer
// as a Movement Sensor component

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_movement_sensor("accel-adxl345", &ADXL345::from_config)
        .is_err()
    {
        log::error!("accel-adxl345 type is already registered");
    }
}

const READING_START_REGISTER: u8 = 50;
const STANDBY_MODE_REGISTER: u8 = 45;

pub struct ADXL345 {
    i2c_handle: I2cHandleType,
    i2c_address: u8,
}

impl ADXL345 {
    pub fn new(mut i2c_handle: I2cHandleType, i2c_address: u8) -> anyhow::Result<Self> {
        let bytes: [u8; 2] = [STANDBY_MODE_REGISTER, 8];
        i2c_handle.write_i2c(i2c_address, &bytes)?;
        println!("created adxl");
        Ok(Self {
            i2c_handle,
            i2c_address,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn from_config(
        cfg: ConfigType,
        board: Option<BoardType>,
    ) -> anyhow::Result<MovementSensorType> {
        if board.is_none() {
            return Err(anyhow::anyhow!(
                "actual board is required to be passed to configure ADXL-345"
            ));
        }
        let board_unwrapped = board.unwrap();
        match cfg {
            ConfigType::Static(cfg) => {
                let i2c_handle: I2cHandleType;
                if let Ok(i2c_name) = cfg.get_attribute::<&'static str>("i2c_bus") {
                    i2c_handle = board_unwrapped.get_i2c_by_name(i2c_name.to_string())?;
                } else {
                    return Err(anyhow::anyhow!(
                        "i2c_bus is a required config attribute for ADXL-345"
                    ));
                };
                if match &cfg.attributes {
                    None => false,
                    Some(attrs) => match attrs.get("use_alt_i2c_address") {
                        Some(BoolValue(value)) => *value,
                        _ => false,
                    },
                } {
                    return Ok(Arc::new(Mutex::new(ADXL345::new(i2c_handle, 29)?)));
                }
                Ok(Arc::new(Mutex::new(ADXL345::new(i2c_handle, 83)?)))
            }
        }
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        // put the MPU in the sleep state
        let off_data: [u8; 2] = [STANDBY_MODE_REGISTER, 0];
        if let Err(err) = self.i2c_handle.write_i2c(self.i2c_address, &off_data) {
            return Err(anyhow::anyhow!("adxl-345 sleep command failed: {:?}", err));
        };
        Ok(())
    }
}

impl Drop for ADXL345 {
    fn drop(&mut self) {
        if let Err(err) = self.close() {
            log::error!("adxl-345 close failure: {:?}", err)
        };
    }
}

fn get_linear_acceleration_from_reading(reading: &[u8; 6]) -> Vector3 {
    let (x_bytes, y_z_bytes) = reading.split_at(size_of::<i16>());
    let unscaled_x = i16::from_le_bytes(x_bytes.try_into().unwrap());
    let (y_bytes, z_bytes) = y_z_bytes.split_at(size_of::<i16>());
    let unscaled_y = i16::from_le_bytes(y_bytes.try_into().unwrap());
    let unscaled_z = i16::from_le_bytes(z_bytes.try_into().unwrap());

    let max_acceleration: f64 = 2.0 * 9.81 * 1000.0;
    let max_i6: f64 = 512.0;

    let x = f64::from(unscaled_x) * max_acceleration / max_i6;
    let y = f64::from(unscaled_y) * max_acceleration / max_i6;
    let z = f64::from(unscaled_z) * max_acceleration / max_i6;
    Vector3 { x, y, z }
}

impl MovementSensor for ADXL345 {
    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: false,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            linear_acceleration_supported: true,
            compass_heading_supported: false,
        }
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        let register_write: [u8; 1] = [READING_START_REGISTER];
        let mut result: [u8; 6] = [0; 6];
        self.i2c_handle
            .write_read_i2c(self.i2c_address, &register_write, &mut result)?;
        Ok(get_linear_acceleration_from_reading(&result))
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_angular_velocity")
    }

    fn get_position(&mut self) -> anyhow::Result<super::movement_sensor::GeoPosition> {
        anyhow::bail!("unimplemented: movement_sensor_get_position")
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_linear_velocity")
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        anyhow::bail!("unimplemented: movement_sensor_get_compass_heading")
    }
}

impl Status for ADXL345 {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::get_linear_acceleration_from_reading;

    #[test_log::test]
    fn test_read_linear_acceleration() -> anyhow::Result<()> {
        let reading: [u8; 6] = [12, 0, 208, 255, 239, 0];
        let lin_acc = get_linear_acceleration_from_reading(&reading);
        assert_eq!(lin_acc.x, 459.84375);
        assert_eq!(lin_acc.y, -1839.375);
        assert_eq!(lin_acc.z, 9158.5546875);
        Ok(())
    }
}
