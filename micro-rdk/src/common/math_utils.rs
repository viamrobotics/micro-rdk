#![allow(dead_code)]
use crate::{
    google::protobuf::{value::Kind, Struct, Value},
    proto::common,
};
use std::{collections::HashMap, time::Duration};
use thiserror::Error;

#[cfg(feature = "data")]
use crate::{
    google::protobuf::Timestamp,
    proto::app::data_sync::v1::{sensor_data, SensorData, SensorMetadata},
};

#[derive(Error, Debug)]
#[error("invalid argument")]
pub struct UtilsInvalidArg;

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
    #[cfg(feature = "data")]
    pub fn to_sensor_data(self, key: &str) -> SensorData {
        let data_struct = Struct {
            fields: HashMap::from([
                (
                    "x".to_string(),
                    Value {
                        kind: Some(Kind::NumberValue(self.x)),
                    },
                ),
                (
                    "y".to_string(),
                    Value {
                        kind: Some(Kind::NumberValue(self.y)),
                    },
                ),
                (
                    "z".to_string(),
                    Value {
                        kind: Some(Kind::NumberValue(self.z)),
                    },
                ),
            ]),
        };

        let current_date = chrono::offset::Local::now().fixed_offset();
        SensorData {
            metadata: Some(SensorMetadata {
                time_received: Some(Timestamp {
                    seconds: current_date.timestamp(),
                    nanos: current_date.timestamp_subsec_nanos() as i32,
                }),
                time_requested: Some(Timestamp {
                    seconds: current_date.timestamp(),
                    nanos: current_date.timestamp_subsec_nanos() as i32,
                }),
            }),
            data: Some(sensor_data::Data::Struct(Struct {
                fields: HashMap::from([(
                    key.to_string(),
                    Value {
                        kind: Some(Kind::StructValue(data_struct)),
                    },
                )]),
            })),
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

impl From<Vector3> for Value {
    fn from(value: Vector3) -> Self {
        let fields = HashMap::from([
            (
                "x".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(value.x)),
                },
            ),
            (
                "y".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(value.y)),
                },
            ),
            (
                "z".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(value.z)),
                },
            ),
        ]);
        Self {
            kind: Some(Kind::StructValue(Struct { fields })),
        }
    }
}

// If revolutions is 0, the returned wait duration will be 0 representing that
// the motor should run indefinitely.
pub(crate) fn go_for_math(
    max_rpm: f64,
    rpm: f64,
    revolutions: f64,
) -> Result<(f64, Option<Duration>), UtilsInvalidArg> {
    /*
    dir := rpm * revolutions / math.Abs(revolutions*rpm)
    powerPct := math.Abs(rpm) / maxRPM * dir
    waitDur := time.Duration(math.Abs(revolutions/rpm)*60*1000) * time.Millisecond
    return powerPct, waitDur
        */
    if max_rpm.is_nan() || rpm.is_nan() || revolutions.is_nan() {
        return Err(UtilsInvalidArg);
    }

    let rpm = rpm.clamp(-1.0 * max_rpm, max_rpm);

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

    #[test_log::test]
    fn test_go_for_math_none_duration() -> anyhow::Result<()> {
        // taken from rdk/components/motor/gpio/basic_test.go
        let (pwr, dur) = go_for_math(200.0, 50.0, 0.0)?;
        assert_eq!(pwr, 0.25);
        assert_eq!(dur, None);

        let (pwr, dur) = go_for_math(200.0, 50.0, 0.0)?;
        assert_eq!(pwr, 0.25);
        assert_eq!(dur, None);

        let (pwr, dur) = go_for_math(200.0, -50.0, 0.0)?;
        assert_eq!(pwr, -0.25);
        assert_eq!(dur, None);

        Ok(())
    }

    #[test_log::test]
    fn test_go_for_math_some_duration() -> anyhow::Result<()> {
        // taken from rdk/components/motor/gpio/basic_test.go

        let (pwr, dur) = go_for_math(100.0, 100.0, 100.0)?;
        assert_eq!(pwr, 1.0);
        assert_eq!(dur, Some(Duration::from_secs(60)));

        let (pwr, dur) = go_for_math(100.0, -100.0, 100.0)?;
        assert_eq!(pwr, -1.0);
        assert_eq!(dur, Some(Duration::from_secs(60)));

        let (pwr, dur) = go_for_math(100.0, -1000.0, 100.0)?;
        assert_eq!(pwr, -1.0);
        assert_eq!(dur, Some(Duration::from_secs(60)));

        let (pwr, dur) = go_for_math(100.0, 1000.0, 200.0)?;
        assert_eq!(pwr, 1.0);
        assert_eq!(dur, Some(Duration::from_secs(120)));

        let (pwr, dur) = go_for_math(100.0, 1000.0, 50.0)?;
        assert_eq!(pwr, 1.0);
        assert_eq!(dur, Some(Duration::from_secs(30)));

        let (pwr, dur) = go_for_math(200.0, 100.0, 50.0)?;
        assert_eq!(pwr, 0.5);
        assert_eq!(dur, Some(Duration::from_secs(30)));

        let (pwr, dur) = go_for_math(200.0, 100.0, -50.0)?;
        assert_eq!(pwr, -0.5);
        assert_eq!(dur, Some(Duration::from_secs(30)));
        Ok(())
    }
}
