use super::single_encoder::SingleEncoderType;

use crate::common::actuator::Actuator;
use crate::common::encoder::{
    Direction, Encoder, EncoderPositionType, EncoderSupportedRepresentations, SingleEncoder,
};
use crate::common::motor::{Motor, MotorSupportedProperties, MotorType};
use crate::common::status::Status;
use crate::google;

use std::collections::HashMap;
use std::time::Duration;

#[derive(DoCommand)]
pub struct SingleEncodedMotor {
    encoder: SingleEncoderType,
    motor: MotorType,
}

impl SingleEncodedMotor {
    pub fn new(motor: MotorType, encoder: SingleEncoderType) -> Self {
        Self { encoder, motor }
    }
}

impl Motor for SingleEncodedMotor {
    fn set_power(&mut self, power_pct: f64) -> anyhow::Result<()> {
        let dir = match power_pct {
            x if x > 0.0 => Direction::Forwards,
            x if x < 0.0 => Direction::Backwards,
            x if x == 0.0 => {
                let prev_dir = self.encoder.get_direction()?;
                match prev_dir {
                    Direction::Backwards | Direction::StoppedBackwards => {
                        Direction::StoppedBackwards
                    }
                    Direction::Forwards | Direction::StoppedForwards => Direction::StoppedForwards,
                }
            }
            _ => unreachable!(),
        };
        self.motor.set_power(power_pct)?;
        log::debug!("set power in single encoded motor to {:?}", power_pct);
        self.encoder.set_direction(dir)
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        let props = self.encoder.get_properties();
        let pos_type = match props {
            EncoderSupportedRepresentations {
                ticks_count_supported: true,
                ..
            } => EncoderPositionType::TICKS,
            EncoderSupportedRepresentations {
                angle_degrees_supported: true,
                ..
            } => EncoderPositionType::DEGREES,
            _ => {
                return Err(anyhow::anyhow!(
                    "encoder for this motor does not support any known position representations"
                ));
            }
        };
        let pos = self.encoder.get_position(pos_type)?;
        Ok(pos.value as i32)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        self.motor.go_for(rpm, revolutions)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: true,
        }
    }
}

impl Actuator for SingleEncodedMotor {
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        self.motor.is_moving()
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.motor.stop()
    }
}

impl Status for SingleEncodedMotor {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        let pos = self
            .encoder
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as f64;
        hm.insert(
            "position".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}
