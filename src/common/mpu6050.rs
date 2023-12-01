//! Package mpu6050 implements the movementsensor interface for an MPU-6050 6-axis accelerometer. A
//! datasheet for this chip is at
//! https://components101.com/sites/default/files/component_datasheet/MPU6050-DataSheet.pdf and a
//! description of the I2C registers is at
//! https://download.datasheets.com/pdfs/2015/3/19/8/3/59/59/invse_/manual/5rm-mpu-6000a-00v4.2.pdf
//!
//! We support reading the accelerometer, gyroscope, and thermometer data off of the chip. We do not
//! yet support using the digital interrupt pin to notify on events (freefall, collision, etc.),
//! nor do we yet support using the secondary I2C connection to add an external clock or
//! magnetometer.
//!
//! The chip has two possible I2C addresses, which can be selected by wiring the AD0 pin to either
//! hot or ground:
//!   - if AD0 is wired to ground, it uses the default I2C address of 0x68
//!   - if AD0 is wired to hot, it uses the alternate I2C address of 0x69
//!

use crate::common::i2c::I2cHandleType;
use crate::common::math_utils::Vector3;
use crate::common::movement_sensor::{MovementSensor, MovementSensorSupportedMethods};
use crate::google;

use super::board::Board;
use super::config::ConfigType;
use super::generic::DoCommand;
use super::i2c::I2CHandle;
use super::movement_sensor::MovementSensorType;
use super::registry::{get_board_from_dependencies, ComponentRegistry, Dependency};
use super::status::Status;

use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Arc, Mutex};

// This module represents an implementation of the MPU-6050 gyroscope/accelerometer
// as a Movement Sensor component

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_movement_sensor("gyro-mpu6050", &MPU6050::from_config)
        .is_err()
    {
        log::error!("gyro-mpu6050 type is already registered");
    }
}

const READING_START_REGISTER: u8 = 59;
const STANDBY_MODE_REGISTER: u8 = 107;
const MAX_U16: f64 = 32768.0;

#[derive(DoCommand)]
pub struct MPU6050 {
    i2c_handle: I2cHandleType,
    i2c_address: u8,
}

impl MPU6050 {
    pub fn new(mut i2c_handle: I2cHandleType, i2c_address: u8) -> anyhow::Result<Self> {
        let bytes: [u8; 2] = [STANDBY_MODE_REGISTER, 0];
        i2c_handle.write_i2c(i2c_address, &bytes)?;
        Ok(MPU6050 {
            i2c_handle,
            i2c_address,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn from_config(
        cfg: ConfigType,
        dependencies: Vec<Dependency>,
    ) -> anyhow::Result<MovementSensorType> {
        let board = get_board_from_dependencies(dependencies);
        if board.is_none() {
            return Err(anyhow::anyhow!(
                "actual board is required to be passed to configure MPU6050"
            ));
        }
        let board_unwrapped = board.unwrap();
        let i2c_handle: I2cHandleType;
        if let Ok(i2c_name) = cfg.get_attribute::<String>("i2c_bus") {
            i2c_handle = board_unwrapped.get_i2c_by_name(i2c_name)?;
        } else {
            return Err(anyhow::anyhow!(
                "i2c_bus is a required config attribute for MPU6050"
            ));
        };
        if let Ok(use_alt_address) = cfg.get_attribute::<bool>("use_alt_i2c_address") {
            if use_alt_address {
                return Ok(Arc::new(Mutex::new(MPU6050::new(i2c_handle, 105)?)));
            }
            Ok(Arc::new(Mutex::new(MPU6050::new(i2c_handle, 104)?)))
        } else {
            Ok(Arc::new(Mutex::new(MPU6050::new(i2c_handle, 104)?)))
        }
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        // put the MPU in the sleep state
        let off_data: [u8; 2] = [STANDBY_MODE_REGISTER, 64];
        if let Err(err) = self.i2c_handle.write_i2c(self.i2c_address, &off_data) {
            return Err(anyhow::anyhow!("mpu6050 sleep command failed: {:?}", err));
        };
        Ok(())
    }
}

// we want to close the MPU (put the sensor to sleep)
// when the component memory gets dropped
impl Drop for MPU6050 {
    fn drop(&mut self) {
        if let Err(err) = self.close() {
            log::error!("mpu6050 close failure: {:?}", err)
        };
    }
}

fn get_angular_velocity_from_reading(reading: &[u8; 14]) -> Vector3 {
    let (x_bytes, y_z_bytes) = reading[8..14].split_at(size_of::<i16>());
    let unscaled_x = i16::from_be_bytes(x_bytes.try_into().unwrap());
    let (y_bytes, z_bytes) = y_z_bytes.split_at(size_of::<i16>());
    let unscaled_y = i16::from_be_bytes(y_bytes.try_into().unwrap());
    let unscaled_z = i16::from_be_bytes(z_bytes.try_into().unwrap());

    let max_velocity: f64 = 250.0;

    let x = f64::from(unscaled_x) * max_velocity / MAX_U16;
    let y = f64::from(unscaled_y) * max_velocity / MAX_U16;
    let z = f64::from(unscaled_z) * max_velocity / MAX_U16;
    Vector3 { x, y, z }
}

fn get_linear_acceleration_from_reading(reading: &[u8; 14]) -> Vector3 {
    let (x_bytes, y_z_bytes) = reading[0..6].split_at(size_of::<i16>());
    let unscaled_x = i16::from_be_bytes(x_bytes.try_into().unwrap());
    let (y_bytes, z_bytes) = y_z_bytes.split_at(size_of::<i16>());
    let unscaled_y = i16::from_be_bytes(y_bytes.try_into().unwrap());
    let unscaled_z = i16::from_be_bytes(z_bytes.try_into().unwrap());

    let max_acceleration: f64 = 2.0 * 9.81;

    let x = f64::from(unscaled_x) * max_acceleration / MAX_U16;
    let y = f64::from(unscaled_y) * max_acceleration / MAX_U16;
    let z = f64::from(unscaled_z) * max_acceleration / MAX_U16;
    Vector3 { x, y, z }
}

impl MovementSensor for MPU6050 {
    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: false,
            linear_velocity_supported: false,
            angular_velocity_supported: true,
            linear_acceleration_supported: true,
            compass_heading_supported: false,
        }
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        let register_write: [u8; 1] = [READING_START_REGISTER];
        let mut result: [u8; 14] = [0; 14];
        self.i2c_handle
            .write_read_i2c(self.i2c_address, &register_write, &mut result)?;
        Ok(get_angular_velocity_from_reading(&result))
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        let register_write: [u8; 1] = [READING_START_REGISTER];
        let mut result: [u8; 14] = [0; 14];
        self.i2c_handle
            .write_read_i2c(self.i2c_address, &register_write, &mut result)?;
        Ok(get_linear_acceleration_from_reading(&result))
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

impl Status for MPU6050 {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{get_angular_velocity_from_reading, get_linear_acceleration_from_reading};

    #[test_log::test]
    fn test_read_linear_acceleration() -> anyhow::Result<()> {
        let reading: [u8; 14] = [64, 0, 32, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let lin_acc = get_linear_acceleration_from_reading(&reading);
        assert_eq!(lin_acc.x, 9.81);
        assert_eq!(lin_acc.y, 4.905);
        assert_eq!(lin_acc.z, 2.4525);

        let reading: [u8; 14] = [64, 0, 130, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let lin_acc = get_linear_acceleration_from_reading(&reading);

        assert_eq!(lin_acc.x, 9.81);
        assert!((lin_acc.y - -19.3134375).abs() < 0.000001);
        assert_eq!(lin_acc.z, 2.4525);
        Ok(())
    }

    #[test_log::test]
    fn test_read_angular_velocity() -> anyhow::Result<()> {
        let reading: [u8; 14] = [0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 32, 0, 16, 0];
        let ang_vel = get_angular_velocity_from_reading(&reading);
        assert_eq!(ang_vel.x, 125.0);
        assert_eq!(ang_vel.y, 62.5);
        assert_eq!(ang_vel.z, 31.25);
        Ok(())
    }
}
