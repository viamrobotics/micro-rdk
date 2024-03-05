use std::sync::{Arc, Mutex};

use crate::google;

pub trait Status {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>>;
}

impl<L> Status for Mutex<L>
where
    L: ?Sized + Status,
{
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        self.lock().unwrap().get_status()
    }
}

impl<A> Status for Arc<A>
where
    A: ?Sized + Status,
{
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        (**self).get_status()
    }
}
