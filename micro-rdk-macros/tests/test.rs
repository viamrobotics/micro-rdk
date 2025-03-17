use micro_rdk::common::math_utils::Vector3;
use micro_rdk::common::movement_sensor::{
    GeoPosition, MovementSensor, MovementSensorSupportedMethods,
};
use micro_rdk::common::power_sensor::{Current, PowerSensor, PowerSupplyType, Voltage};
use micro_rdk::common::sensor::{Readings, SensorError};
use micro_rdk::common::status::{Status, StatusError};
use micro_rdk::google::protobuf::value::Kind;
use micro_rdk_macros::{DoCommand, MovementSensorReadings, PowerSensorReadings};
use std::collections::HashMap;

#[derive(DoCommand)]
struct TestDoCommandStruct {}

#[derive(DoCommand, MovementSensorReadings)]
struct TestMovementSensor {}

impl MovementSensor for TestMovementSensor {
    fn get_position(&mut self) -> Result<GeoPosition, SensorError> {
        Ok(GeoPosition {
            lat: 1.0,
            lon: 2.0,
            alt: 3.0,
        })
    }

    fn get_linear_acceleration(&mut self) -> Result<Vector3, SensorError> {
        Ok(Vector3 {
            x: 0.0,
            y: 1.0,
            z: 2.0,
        })
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: true,
            linear_acceleration_supported: true,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            compass_heading_supported: true,
        }
    }

    fn get_linear_velocity(&mut self) -> Result<Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "get_linear_velocity",
        ))
    }

    fn get_angular_velocity(&mut self) -> Result<Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "get_angular_velocity",
        ))
    }

    fn get_compass_heading(&mut self) -> Result<f64, SensorError> {
        Ok(3.5)
    }
}

impl Status for TestMovementSensor {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[derive(DoCommand, PowerSensorReadings)]
struct TestPowerSensor {}

impl PowerSensor for TestPowerSensor {
    fn get_voltage(&mut self) -> Result<Voltage, SensorError> {
        Ok(Voltage {
            volts: 5.0,
            power_supply_type: PowerSupplyType::AC,
        })
    }

    fn get_current(&mut self) -> Result<Current, SensorError> {
        Ok(Current {
            amperes: 6.0,
            power_supply_type: PowerSupplyType::AC,
        })
    }

    fn get_power(&mut self) -> Result<f64, SensorError> {
        Ok(7.0)
    }
}

impl Status for TestPowerSensor {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[test]
fn do_command_derive() {
    use micro_rdk::common::generic::DoCommand;
    let mut a = TestDoCommandStruct {};
    assert!(a.do_command(None).is_err());
}

#[test]
fn movement_sensor_readings_derive() {
    let mut a = TestMovementSensor {};
    let res = a.get_generic_readings();
    assert!(res.is_ok());
    let res = res.unwrap();

    // test position
    let pos = res.get("position");
    assert!(pos.is_some());
    let pos = &pos.unwrap().kind;
    assert!(pos.is_some());
    let pos = pos.as_ref().unwrap();
    if let Kind::StructValue(pos_struct) = pos {
        let lat_val = pos_struct.fields.get("lat");
        assert!(lat_val.is_some());
        if let Some(Kind::NumberValue(lat)) = lat_val.unwrap().kind {
            assert_eq!(lat, 1.0);
        } else {
            panic!(
                "expected a Kind::NumberValue have {:?}",
                lat_val.unwrap().kind
            );
        }
    } else {
        panic!("expected a StructValue have {:?}", pos);
    }

    // test acceleration
    let acc = res.get("linear_acceleration");
    assert!(acc.is_some());
    let acc = &acc.unwrap().kind;
    assert!(acc.is_some());
    let acc = acc.as_ref().unwrap();
    if let Kind::StructValue(acc_struct) = acc {
        let y_val = acc_struct.fields.get("y");
        assert!(y_val.is_some());
        if let Some(Kind::NumberValue(y)) = y_val.unwrap().kind {
            assert_eq!(y, 1.0);
        } else {
            panic!(
                "expected a Kind::NumberValue have {:?}",
                y_val.unwrap().kind
            );
        }
    } else {
        panic!("expected a StructValue have {:?}", acc);
    }
}

#[test]
fn power_sensor_readings_derive() {
    let mut a = TestPowerSensor {};
    let res = a.get_generic_readings();
    assert!(res.is_ok());
    let res = res.unwrap();

    let volts = res.get("volts");
    assert!(volts.is_some());
    let volts = &volts.unwrap().kind;
    assert!(volts.is_some());
    let volts = volts.as_ref().unwrap();
    if let Kind::NumberValue(volts) = volts {
        assert_eq!(*volts, 5.0)
    }

    let is_ac = res.get("is_ac");
    assert!(is_ac.is_some());
    let is_ac = &is_ac.unwrap().kind;
    assert!(is_ac.is_some());
    let is_ac = is_ac.as_ref().unwrap();
    if let Kind::BoolValue(is_ac) = is_ac {
        assert!(is_ac)
    }
}
