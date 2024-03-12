use std::sync::{Arc, Mutex};
use thiserror::Error;

use super::board::BoardError;

#[derive(Debug, Error)]
pub enum ActuatorError {
    #[error("couldn't stop actuator")]
    CouldntStop,
    #[error(transparent)]
    BoardError(#[from] BoardError),
}

pub trait Actuator {
    fn is_moving(&mut self) -> Result<bool, ActuatorError>;
    fn stop(&mut self) -> Result<(), ActuatorError>;
}

impl<L> Actuator for Mutex<L>
where
    L: ?Sized + Actuator,
{
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        self.get_mut().unwrap().is_moving()
    }
    fn stop(&mut self) -> Result<(), ActuatorError> {
        self.get_mut().unwrap().stop()
    }
}

impl<A> Actuator for Arc<Mutex<A>>
where
    A: ?Sized + Actuator,
{
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        self.lock().unwrap().is_moving()
    }
    fn stop(&mut self) -> Result<(), ActuatorError> {
        self.lock().unwrap().stop()
    }
}
