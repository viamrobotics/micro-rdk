use std::sync::{Arc, Mutex};

use crate::{
    google::protobuf::{value::Kind, Value},
    proto::component,
};

use super::{
    generic::DoCommand,
    sensor::{GenericReadingsResult, Readings},
    status::Status,
};

pub static COMPONENT_NAME: &str = "power_sensor";

#[derive(Debug, Copy, Clone)]
pub enum PowerSupplyType {
    AC,
    DC,
}

#[derive(Debug, Copy, Clone)]
pub struct Voltage {
    pub volts: f64,
    pub power_supply_type: PowerSupplyType,
}

#[derive(Debug, Copy, Clone)]
pub struct Current {
    pub amperes: f64,
    pub power_supply_type: PowerSupplyType,
}

impl From<Voltage> for component::power_sensor::v1::GetVoltageResponse {
    fn from(value: Voltage) -> Self {
        Self {
            volts: value.volts,
            is_ac: match value.power_supply_type {
                PowerSupplyType::AC => true,
                PowerSupplyType::DC => false,
            },
        }
    }
}

impl From<Current> for component::power_sensor::v1::GetCurrentResponse {
    fn from(value: Current) -> Self {
        Self {
            amperes: value.amperes,
            is_ac: match value.power_supply_type {
                PowerSupplyType::AC => true,
                PowerSupplyType::DC => false,
            },
        }
    }
}

pub trait PowerSensor: Status + Readings + DoCommand {
    fn get_voltage(&mut self) -> anyhow::Result<Voltage>;

    fn get_current(&mut self) -> anyhow::Result<Current>;

    /// returns the power reading in watts
    fn get_power(&mut self) -> anyhow::Result<f64>;
}

pub type PowerSensorType = Arc<Mutex<dyn PowerSensor>>;

pub fn get_power_sensor_generic_readings(
    ps: &mut dyn PowerSensor,
) -> anyhow::Result<GenericReadingsResult> {
    let voltage = ps.get_voltage()?;
    let current = ps.get_current()?;
    let power = ps.get_power()?;

    let res = std::collections::HashMap::from([
        (
            "volts".to_string(),
            Value {
                kind: Some(Kind::NumberValue(voltage.volts)),
            },
        ),
        (
            "amps".to_string(),
            Value {
                kind: Some(Kind::NumberValue(current.amperes)),
            },
        ),
        (
            "is_ac".to_string(),
            Value {
                kind: Some(Kind::BoolValue(matches!(
                    voltage.power_supply_type,
                    PowerSupplyType::AC
                ))),
            },
        ),
        (
            "watts".to_string(),
            Value {
                kind: Some(Kind::NumberValue(power)),
            },
        ),
    ]);
    Ok(res)
}

impl<P> PowerSensor for Mutex<P>
where
    P: ?Sized + PowerSensor,
{
    fn get_current(&mut self) -> anyhow::Result<Current> {
        self.get_mut().unwrap().get_current()
    }

    fn get_voltage(&mut self) -> anyhow::Result<Voltage> {
        self.get_mut().unwrap().get_voltage()
    }

    fn get_power(&mut self) -> anyhow::Result<f64> {
        self.get_mut().unwrap().get_power()
    }
}

impl<A> PowerSensor for Arc<Mutex<A>>
where
    A: ?Sized + PowerSensor,
{
    fn get_current(&mut self) -> anyhow::Result<Current> {
        self.lock().unwrap().get_current()
    }

    fn get_voltage(&mut self) -> anyhow::Result<Voltage> {
        self.lock().unwrap().get_voltage()
    }

    fn get_power(&mut self) -> anyhow::Result<f64> {
        self.lock().unwrap().get_power()
    }
}
