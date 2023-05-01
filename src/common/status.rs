use std::sync::{Arc, Mutex};

pub trait Status {
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>>;
}

impl<L> Status for Mutex<L>
where
    L: ?Sized + Status,
{
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>> {
        self.get_mut().unwrap().get_status()
    }
}

impl<A> Status for Arc<Mutex<A>>
where
    A: ?Sized + Status,
{
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>> {
        self.lock().unwrap().get_status()
    }
}
