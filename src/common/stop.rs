use std::sync::{Arc, Mutex};

pub trait Stoppable {
    fn stop(&mut self) -> anyhow::Result<()>;
}

impl<L> Stoppable for Mutex<L>
where
    L: ?Sized + Stoppable,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.lock().unwrap().stop()
    }
}

impl<A> Stoppable for Arc<Mutex<A>>
where
    A: ?Sized + Stoppable,
{
    fn stop(&mut self) -> anyhow::Result<()> {
        self.stop()
    }
}
