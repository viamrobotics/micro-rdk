use std::sync::{Arc, Mutex};

use crate::proto::component;

use super::{generic::DoCommand, status::Status};

pub static COMPONENT_NAME: &str = "power_sensor";

#[derive(Debug, Copy, Clone)]
pub enum PowerSupplyType {
    AC,
    DC,
}

#[derive(Debug, Copy, Clone)]
pub struct Voltage {
    volts: f64,
    power_supply_type: PowerSupplyType,
}

#[derive(Debug, Copy, Clone)]
pub struct Current {
    amperes: f64,
    power_supply_type: PowerSupplyType,
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

pub trait PowerSensor: Status + DoCommand {
    fn get_voltage(&self) -> anyhow::Result<Voltage>;

    fn get_current(&self) -> anyhow::Result<Current>;

    /// returns the power reading in watts
    fn get_power(&self) -> anyhow::Result<f64>;
}

pub type PowerSensorType = Arc<Mutex<dyn PowerSensor>>;

impl<P> PowerSensor for Mutex<P>
where
    P: ?Sized + PowerSensor,
{
    fn get_current(&self) -> anyhow::Result<Current> {
        self.lock().unwrap().get_current()
    }

    fn get_voltage(&self) -> anyhow::Result<Voltage> {
        self.lock().unwrap().get_voltage()
    }

    fn get_power(&self) -> anyhow::Result<f64> {
        self.lock().unwrap().get_power()
    }
}

impl<A> PowerSensor for Arc<Mutex<A>>
where
    A: ?Sized + PowerSensor,
{
    fn get_current(&self) -> anyhow::Result<Current> {
        self.lock().unwrap().get_current()
    }

    fn get_voltage(&self) -> anyhow::Result<Voltage> {
        self.lock().unwrap().get_voltage()
    }

    fn get_power(&self) -> anyhow::Result<f64> {
        self.lock().unwrap().get_power()
    }
}
