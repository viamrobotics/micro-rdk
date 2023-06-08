#![allow(dead_code)]
use crate::proto::common;
use anyhow::bail;
use std::time::Duration;

pub enum MathUtilError {
    No,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub fn new() -> Self {
        Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl From<Vector3> for common::v1::Vector3 {
    fn from(vector: Vector3) -> Self {
        common::v1::Vector3 {
            x: vector.x,
            y: vector.y,
            z: vector.z,
        }
    }
}
// If revolutions is 0, the returned wait duration will be 0 representing that
// the motor should run indefinitely.
pub(crate) fn go_for_math(
    max_rpm: f64,
    rpm: f64,
    revolutions: f64,
) -> anyhow::Result<(f64, Option<Duration>)> {
    let rpm = rpm.clamp(-1.0, 1.0);
    if rpm.is_nan() {
        bail!("supplied rpm is NaN");
    }
    if revolutions.is_nan() {
        bail!("supplied revolutions is NaN");
    }

    if revolutions == 0.0 {
        return Ok((rpm / max_rpm, None));
    }

    let dir = rpm * revolutions / (revolutions * rpm).abs();
    let pct = rpm.abs() / max_rpm * dir;
    let dur = Duration::from_secs_f64((revolutions / rpm).abs() * 60.0);

    Ok((pct, Some(dur)))
}
