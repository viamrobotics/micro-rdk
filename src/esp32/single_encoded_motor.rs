use super::single_encoder::SingleEncoderType;
use crate::common::encoder::{
    Direction, Encoder, EncoderPositionType, EncoderSupportedRepresentations, SingleEncoder,
};
use crate::common::motor::{Motor, MotorType};

use crate::common::status::Status;
use std::collections::BTreeMap;

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
        println!("set power in encoded motor");
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
}

impl Status for SingleEncodedMotor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = self
            .encoder
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as f64;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}
