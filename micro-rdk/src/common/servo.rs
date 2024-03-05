use std::sync::{Arc, Mutex};

use super::{actuator::Actuator, generic::DoCommand, status::Status};

pub static COMPONENT_NAME: &str = "servo";

pub trait Servo: Status + Actuator + DoCommand {
    /// Moves the servo to an angular position of `angle_deg` away
    /// from the home position
    fn move_to(&mut self, angle_deg: u32) -> anyhow::Result<()>;

    /// Gets the current angular position of the servo in degrees
    fn get_position(&mut self) -> anyhow::Result<u32>;
}

pub type ServoType = Arc<Mutex<dyn Servo>>;

impl<L> Servo for Mutex<L>
where
    L: ?Sized + Servo,
{
    fn move_to(&mut self, angle_deg: u32) -> anyhow::Result<()> {
        self.get_mut().unwrap().move_to(angle_deg)
    }
    fn get_position(&mut self) -> anyhow::Result<u32> {
        self.get_mut().unwrap().get_position()
    }
}

impl<A> Servo for Arc<Mutex<A>>
where
    A: ?Sized + Servo,
{
    fn move_to(&mut self, angle_deg: u32) -> anyhow::Result<()> {
        self.lock().unwrap().move_to(angle_deg)
    }
    fn get_position(&mut self) -> anyhow::Result<u32> {
        self.lock().unwrap().get_position()
    }
}
