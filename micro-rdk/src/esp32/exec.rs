//! The exec module exposes helpers to execute futures on an ESP32
use crate::common::webrtc::exec::WebRtcExecutor;
use async_executor::{LocalExecutor, Task};
use futures_lite::{
    future::{self, block_on},
    Future,
};
#[derive(Clone, Debug, Default)]
/// This executor is local and bounded to the CPU that created it usually you would create it after spwaning a thread on a specific core
pub struct Esp32Executor {}

std::thread_local! {
    static EX: LocalExecutor<'static> = LocalExecutor::new();
}

impl Esp32Executor {
    /// Return a new executor bounded to the current core.
    pub fn new() -> Self {
        Self {}
    }
    // Spawn a future onto the local executor
    pub fn spawn<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> Task<T> {
        EX.with(|e| e.spawn(future))
    }

    pub fn run_forever<T>(&self, future: impl Future<Output = T>) -> T {
        EX.with(|e| block_on(e.run(future)))
    }
}

/// helper trait for hyper to spwan future onto a local executor
impl<F> hyper::rt::Executor<F> for Esp32Executor
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        EX.with(|e| e.spawn(fut)).detach();
    }
}

impl<F> WebRtcExecutor<F> for Esp32Executor
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        EX.with(|e| e.spawn(fut)).detach();
    }
}
