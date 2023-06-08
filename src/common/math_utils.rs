#![allow(dead_code)]
use crate::proto::common;
use std::time::Duration;

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
pub(crate) fn go_for_math(max_rpm: f64, rpm: f64, revolutions: f64) -> Result<(f64, Duration), ()> {
    // need to do this so time is reasonable
    let mut rpm = rpm;
    if rpm > max_rpm {
        rpm = max_rpm
    } else if rpm < -1.0 * max_rpm {
        rpm = -1.0 * max_rpm;
    }

    if revolutions == 0.0 {
        return Ok((rpm / max_rpm, Duration::from_millis(0)));
    }

    let dir = rpm * revolutions / (revolutions * rpm).abs();
    let pct = rpm.abs() / max_rpm * dir;
    let dur = Duration::from_secs_f64((revolutions / rpm).abs() * 60.0);
    Ok((pct, dur))
}

