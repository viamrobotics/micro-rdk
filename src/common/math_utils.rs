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
    if max_rpm.is_nan() || rpm.is_nan() || revolutions.is_nan() {
        bail!("NaN in supplied input");
    }

    if revolutions == 0.0 {
        return Ok((rpm / max_rpm, None));
    }

    let dir = rpm * revolutions / (revolutions * rpm).abs();
    let pct = rpm.abs() / max_rpm * dir;
    let dur = Duration::from_secs_f64((revolutions / rpm).abs() * 60.0);

    Ok((pct, Some(dur)))
}

#[cfg(test)]
mod tests {
    use crate::common::math_utils::*;
    use std::time::Duration;

    #[test_log::test]
    fn test_go_for_math_nans() {
        let max_rpm = 0.0;
        let rpm = 0.0;
        let revolutions = 0.0;

        let max_nan = go_for_math(f64::NAN, rpm, revolutions);
        assert!(max_nan.is_err());
        let rpm_nan = go_for_math(max_rpm, f64::NAN, revolutions);
        assert!(rpm_nan.is_err());
        let rev_nan = go_for_math(max_rpm, rpm, f64::NAN);
        assert!(rev_nan.is_err());
    }

    // TODO: put real inputs and expected outcomes

    #[test_log::test]
    fn test_go_for_math_none_duration() -> anyhow::Result<()> {
        let max_rpm = 200.0;
        let rpm = 500.0;
        let revolutions = 0.0;

        let (pwr, dur) = go_for_math(max_rpm, rpm, revolutions)?;
        assert!(dur.is_none());
        assert_eq!(pwr, 0.005);
        Ok(())
    }

    #[test_log::test]
    fn test_go_for_math_some_duration() -> anyhow::Result<()> {
        let max_rpm = 50.0;
        let rpm = 10.0;
        let revolutions = 100.0;

        let (pwr, dur) = go_for_math(max_rpm, rpm, revolutions)?;
        assert_eq!(pwr, 0.02);
        assert_eq!(dur, Some(Duration::from_secs(6000)));
        assert!(dur.is_some());
        Ok(())
    }
}
