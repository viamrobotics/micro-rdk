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

use byteorder::{BigEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

// This is a struct meant to hold the most recently polled readings value
// and the most recent success or error state in acquiring readings. This
// is internal to this driver and should be accessed via Mutex.
#[derive(Clone, Debug)]
struct MPU6050State {
    angular_velocity: Vector3,
    linear_acceleration: Vector3,
    // temperature: f64,
    error: Option<Arc<anyhow::Error>>,
}

impl MPU6050State {
    fn new() -> Self {
        MPU6050State {
            angular_velocity: Vector3::new(),
            linear_acceleration: Vector3::new(),
            // temperature: 0.0,
            error: None,
        }
    }

    fn get_angular_velocity(&self) -> Vector3 {
        self.angular_velocity
    }

    fn get_linear_acceleration(&self) -> Vector3 {
        self.linear_acceleration
    }

    fn set_error(&mut self, err: Option<Arc<anyhow::Error>>) {
        self.error = err;
    }

    fn set_linear_acceleration_from_reading(&mut self, reading: &[u8; 14]) {
        let mut slice_copy = vec![0; 6];
        slice_copy.clone_from_slice(&(reading[0..6]));
        let mut rdr = Cursor::new(slice_copy);
        let unscaled_x = rdr.read_i16::<BigEndian>().unwrap();
        let unscaled_y = rdr.read_i16::<BigEndian>().unwrap();
        let unscaled_z = rdr.read_i16::<BigEndian>().unwrap();

        let max_acceleration: f64 = 2.0 * 9.81 * 1000.0;

        let x = f64::from(unscaled_x) * max_acceleration / 32768.0;
        let y = f64::from(unscaled_y) * max_acceleration / 32768.0;
        let z = f64::from(unscaled_z) * max_acceleration / 32768.0;
        self.linear_acceleration = Vector3 { x, y, z };
    }

    fn set_angular_velocity_from_reading(&mut self, reading: &[u8; 14]) {
        let mut slice_copy = vec![0; 6];
        slice_copy.clone_from_slice(&(reading[8..14]));
        let mut rdr = Cursor::new(slice_copy);
        let unscaled_x = rdr.read_u16::<BigEndian>().unwrap();
        let unscaled_y = rdr.read_u16::<BigEndian>().unwrap();
        let unscaled_z = rdr.read_u16::<BigEndian>().unwrap();
        let x = f64::from(unscaled_x) * 250.0 / 32768.0;
        let y = f64::from(unscaled_y) * 250.0 / 32768.0;
        let z = f64::from(unscaled_z) * 250.0 / 32768.0;
        self.angular_velocity = Vector3 { x, y, z };
    }
}

pub struct MPU6050 {
    state: Arc<Mutex<MPU6050State>>,
    i2c_handle: I2cHandleType,
    i2c_address: u8,
    canceller: Sender<bool>,
}

impl MPU6050 {
    pub fn new(mut i2c_handle: I2cHandleType, i2c_address: u8) -> anyhow::Result<Self> {
        let bytes: [u8; 2] = [107, 0];
        i2c_handle.write_i2c(i2c_address, &bytes)?;
        let i2c_address_copy = i2c_address;
        let raw_state = MPU6050State::new();
        let (canceller, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(raw_state));
        // reference copies for sending memory into thread
        let mut i2c_handle_copy = Arc::clone(&i2c_handle);
        let state_copy = Arc::clone(&state);
        // start a polling thread that reads from the MPU every millisecond and mutates state.
        // This allows multi-read access to the state for the functions satisfying the
        // Movement Sensor API
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(1));
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    log::debug!("MPU6050: Terminating polling thread.");
                    break;
                }
                Err(TryRecvError::Empty) => {
                    let register_write: [u8; 1] = [59];
                    let mut result: [u8; 14] = [0; 14];
                    let mut internal_state = state_copy.lock().unwrap();
                    let res = i2c_handle_copy.write_read_i2c(
                        i2c_address_copy,
                        &register_write,
                        &mut result,
                    );

                    match res {
                        Ok(_) => {
                            internal_state.set_linear_acceleration_from_reading(&result);
                            internal_state.set_angular_velocity_from_reading(&result);
                            internal_state.set_error(None);
                        }
                        Err(err) => {
                            log::error!("MPU I2C error: {:?}", err);
                            internal_state.set_error(Some(Arc::new(err)));
                        }
                    };
                }
            }
        });
        Ok(MPU6050 {
            state,
            i2c_handle,
            i2c_address,
            canceller,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn from_config(
        cfg: ConfigType,
        board: Option<BoardType>,
    ) -> anyhow::Result<MovementSensorType> {
        if board.is_none() {
            return Err(anyhow::anyhow!(
                "actual board is required to be passed to configure MPU6050"
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
                        "i2c_bus is a required config attribute for MPU6050"
                    ));
                };
                if match &cfg.attributes {
                    None => false,
                    Some(attrs) => match attrs.get("use_alt_i2c_address") {
                        Some(BoolValue(value)) => *value,
                        _ => false,
                    },
                } {
                    return Ok(Arc::new(Mutex::new(MPU6050::new(i2c_handle, 105)?)));
                }
                Ok(Arc::new(Mutex::new(MPU6050::new(i2c_handle, 104)?)))
            }
        }
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        // close the polling thread
        if let Err(err) = self.canceller.send(true) {
            return Err(anyhow::anyhow!(
                "mpu6050 failed to close polling thread: {:?}",
                err
            ));
        };
        // put the MPU in the sleep state
        let off_data: [u8; 2] = [107, 64];
        if let Err(err) = self.i2c_handle.write_i2c(self.i2c_address, &off_data) {
            return Err(anyhow::anyhow!("mpu6050 sleep command failed: {:?}", err));
        };
        Ok(())
    }
}

// we want to close the MPU (terminate the polling thread and put the sensor to sleep)
// when the component memory gets dropped
impl Drop for MPU6050 {
    fn drop(&mut self) {
        if let Err(err) = self.close() {
            log::error!("mpu6050 close failure: {:?}", err)
        };
    }
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

    fn get_angular_velocity(&self) -> anyhow::Result<Vector3> {
        let state = self.state.lock().unwrap();
        match &state.error {
            None => Ok(state.get_angular_velocity()),
            Some(error_arc) => {
                let inner_err = error_arc.as_ref();
                Err(anyhow::anyhow!("{}", *inner_err))
            }
        }
    }

    fn get_linear_acceleration(&self) -> anyhow::Result<Vector3> {
        let state = self.state.lock().unwrap();
        match &state.error {
            None => Ok(state.get_linear_acceleration()),
            Some(error_arc) => {
                let inner_err = error_arc.as_ref();
                Err(anyhow::anyhow!("{}", *inner_err))
            }
        }
    }

    fn get_position(&self) -> anyhow::Result<super::movement_sensor::GeoPosition> {
        anyhow::bail!("unimplemented: movement_sensor_get_position")
    }

    fn get_linear_velocity(&self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_linear_velocity")
    }

    fn get_compass_heading(&self) -> anyhow::Result<f64> {
        anyhow::bail!("unimplemented: movement_sensor_get_compass_heading")
    }
}

impl Status for MPU6050 {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::MPU6050State;

    #[test_log::test]
    fn test_read_linear_acceleration() -> anyhow::Result<()> {
        let mut state = MPU6050State::new();
        let reading: [u8; 14] = [64, 0, 32, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let expected_reading: [u8; 14] = [64, 0, 32, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        state.set_linear_acceleration_from_reading(&reading);

        // test reading array unchanged
        assert_eq!(reading.len(), expected_reading.len());
        assert!(reading
            .iter()
            .zip(expected_reading.iter())
            .all(|(a, b)| a == b));

        let lin_acc = state.get_linear_acceleration();
        assert_eq!(lin_acc.x, 9810.0);
        assert_eq!(lin_acc.y, 4905.0);
        assert_eq!(lin_acc.z, 2452.5);
        Ok(())
    }

    #[test_log::test]
    fn test_read_angular_velocity() -> anyhow::Result<()> {
        let mut state = MPU6050State::new();
        let reading: [u8; 14] = [0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 32, 0, 16, 0];
        let expected_reading: [u8; 14] = [0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 32, 0, 16, 0];

        state.set_angular_velocity_from_reading(&reading);

        // test reading array unchanged
        assert_eq!(reading.len(), expected_reading.len());
        assert!(reading
            .iter()
            .zip(expected_reading.iter())
            .all(|(a, b)| a == b));

        let ang_vel = state.get_angular_velocity();
        assert_eq!(ang_vel.x, 125.0);
        assert_eq!(ang_vel.y, 62.5);
        assert_eq!(ang_vel.z, 31.25);
        Ok(())
    }

    #[test_log::test]
    fn test_multiple_values_from_single_reading() -> anyhow::Result<()> {
        let mut state = MPU6050State::new();
        let reading: [u8; 14] = [64, 0, 32, 0, 16, 0, 0, 0, 64, 0, 32, 0, 16, 0];
        let expected_reading: [u8; 14] = [64, 0, 32, 0, 16, 0, 0, 0, 64, 0, 32, 0, 16, 0];

        state.set_angular_velocity_from_reading(&reading);
        state.set_linear_acceleration_from_reading(&reading);

        // test reading array unchanged
        assert_eq!(reading.len(), expected_reading.len());
        assert!(reading
            .iter()
            .zip(expected_reading.iter())
            .all(|(a, b)| a == b));

        let lin_acc = state.get_linear_acceleration();
        assert_eq!(lin_acc.x, 9810.0);
        assert_eq!(lin_acc.y, 4905.0);
        assert_eq!(lin_acc.z, 2452.5);

        let ang_vel = state.get_angular_velocity();
        assert_eq!(ang_vel.x, 125.0);
        assert_eq!(ang_vel.y, 62.5);
        assert_eq!(ang_vel.z, 31.25);

        Ok(())
    }
}
