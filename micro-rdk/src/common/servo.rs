use super::{actuator::Actuator, config::AttributeError, generic::DoCommand};
use crate::common::board::BoardError;
use std::sync::{Arc, Mutex};
use thiserror::Error;
pub static COMPONENT_NAME: &str = "servo";
#[derive(Debug, Error)]
pub enum ServoError {
    #[error(transparent)]
    ServoBoardError(#[from] BoardError),
    #[error("config error {0}")]
    ServoConfigurationError(&'static str),
    #[error(transparent)]
    ServoConfigAttributeError(#[from] AttributeError),
}

pub trait Servo: Actuator + DoCommand {
    /// Moves the servo to an angular position of `angle_deg` away
    /// from the home position
    fn move_to(&mut self, angle_deg: u32) -> Result<(), ServoError>;

    /// Gets the current angular position of the servo in degrees
    fn get_position(&mut self) -> Result<u32, ServoError>;
}

pub type ServoType = Arc<Mutex<dyn Servo>>;

impl<L> Servo for Mutex<L>
where
    L: ?Sized + Servo,
{
    fn move_to(&mut self, angle_deg: u32) -> Result<(), ServoError> {
        self.get_mut().unwrap().move_to(angle_deg)
    }
    fn get_position(&mut self) -> Result<u32, ServoError> {
        self.get_mut().unwrap().get_position()
    }
}

impl<A> Servo for Arc<Mutex<A>>
where
    A: ?Sized + Servo,
{
    fn move_to(&mut self, angle_deg: u32) -> Result<(), ServoError> {
        self.lock().unwrap().move_to(angle_deg)
    }
    fn get_position(&mut self) -> Result<u32, ServoError> {
        self.lock().unwrap().get_position()
    }
}
