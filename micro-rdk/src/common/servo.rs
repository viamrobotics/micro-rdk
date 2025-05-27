use super::{actuator::Actuator, board::BoardError, config::AttributeError, generic::DoCommand};

#[cfg(feature = "builtin-components")]
use super::{
    config::ConfigType,
    registry::{ComponentRegistry, Dependency},
};

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

#[cfg(feature = "builtin-components")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_servo("fake", &FakeServo::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
}

#[cfg(feature = "builtin-components")]
#[derive(DoCommand)]
pub struct FakeServo {
    pos: u32,
    pow: u32,
}

#[cfg(feature = "builtin-components")]
impl FakeServo {
    pub fn new() -> Self {
        Self { pos: 10, pow: 1 }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<ServoType, ServoError> {
        let mut servo = FakeServo::default();
        if let Ok(pos) = cfg.get_attribute::<u32>("fake_position") {
            servo.pos = pos
        }
        Ok(Arc::new(Mutex::new(servo)))
    }
}
#[cfg(feature = "builtin-components")]
impl Default for FakeServo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "builtin-components")]
impl Servo for FakeServo {
    fn get_position(&mut self) -> Result<u32, ServoError> {
        Ok(self.pos)
    }

    fn move_to(&mut self, angle_deg: u32) -> Result<(), ServoError> {
        self.pos = angle_deg;
        Ok(())
    }
}

#[cfg(feature = "builtin-components")]
impl Actuator for FakeServo {
    fn is_moving(&mut self) -> Result<bool, super::actuator::ActuatorError> {
        Ok(self.pow > 0)
    }
    fn stop(&mut self) -> Result<(), super::actuator::ActuatorError> {
        self.pos = 0;
        Ok(())
    }
}
