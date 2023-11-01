use std::sync::{Arc, Mutex};

pub trait Actuator {
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        anyhow::bail!("is_moving is unsupported")
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("stop is unsupported")
    }
}

impl<L> Actuator for Mutex<L>
where
    L: ?Sized + Actuator,
{
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        self.get_mut().unwrap().is_moving()
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.get_mut().unwrap().stop()
    }
}

impl<A> Actuator for Arc<Mutex<A>>
where
    A: ?Sized + Actuator,
{
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        self.lock().unwrap().is_moving()
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        self.lock().unwrap().stop()
    }
}
