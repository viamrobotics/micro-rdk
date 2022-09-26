#![allow(dead_code)]
use crate::motor::Motor;
use crate::proto::common::v1::Vector3;
use crate::status::Status;
use log::*;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub trait Base: Status {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}

pub struct FakeBase {}

impl FakeBase {
    pub fn new() -> Self {
        FakeBase {}
    }
}

impl<L> Base for Mutex<L>
where
    L: Base,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        self.lock().unwrap().set_power(lin, ang)
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.lock().unwrap().stop()
    }
}

impl Base for FakeBase {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        info!(
            "Setting power following lin vec {:?} and ang {:?}",
            lin, ang
        );
        Ok(())
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping base");
        Ok(())
    }
}

impl Status for FakeBase {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        bt.insert(
            "is_moving".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::BoolValue(false)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

pub struct Esp32WheelBase<ML, MR> {
    motor_right: MR,
    motor_left: ML,
}

impl<ML, MR> Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    pub fn new(motor_left: ML, motor_right: MR) -> Self {
        Esp32WheelBase {
            motor_right,
            motor_left,
        }
    }
    fn differential_drive(&self, forward: f64, left: f64) -> (f64, f64) {
        if forward < 0.0 {
            let (r, l) = self.differential_drive(-forward, left);
            return (-r, -l);
        }
        let r = forward.hypot(left);
        let mut t = left.atan2(forward);
        t += std::f64::consts::FRAC_PI_4;
        let l = (r * t.cos()) * std::f64::consts::SQRT_2;
        let r = (r * t.sin()) * std::f64::consts::SQRT_2;
        (l.max(-1.0).min(1.0), r.max(-1.0).min(1.0))
    }
}
impl<ML, MR> Status for Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        bt.insert(
            "is_moving".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::BoolValue(false)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

impl<ML, MR> Base for Esp32WheelBase<ML, MR>
where
    ML: Motor,
    MR: Motor,
{
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        let (l, r) = self.differential_drive(lin.y, ang.z);
        self.motor_left.set_power(l)?;
        self.motor_right.set_power(r)?;
        Ok(())
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.motor_left.set_power(0.0)?;
        self.motor_right.set_power(0.0)?;
        Ok(())
    }
}
