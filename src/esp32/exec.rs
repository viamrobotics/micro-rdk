use crate::common::webrtc::exec::WebRTCExecutor;
///! The exec module exposes helpers to execute futures on an ESP32
use futures_lite::{future, Future};
use smol::{LocalExecutor, Task};
use std::rc::Rc;
#[derive(Clone, Debug)]
/// This executor is local and bounded to the CPU that created it usually you would create it after spwaning a thread on a specific core
pub struct Esp32Executor<'a> {
    /// A local executor
    executor: Rc<LocalExecutor<'a>>,
}

impl<'a> Esp32Executor<'a> {
    /// Return a new executor bounded to the current core.
    pub fn new() -> Self {
        Esp32Executor {
            executor: Rc::new(LocalExecutor::new()),
        }
    }
    /// Spawn a future onto the local executor
    pub fn spawn<T: 'a>(&self, future: impl Future<Output = T> + 'a) -> Task<T> {
        self.executor.spawn(future)
    }
    /// Run a future until it's completion
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        self.executor.run(future).await
    }
}

impl<'a> Default for Esp32Executor<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// helper trait for hyper to spwan future onto a local executor
impl<F> hyper::rt::Executor<F> for Esp32Executor<'_>
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        self.executor.spawn(fut).detach();
    }
}

impl<F> WebRTCExecutor<F> for Esp32Executor<'_>
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        self.executor.spawn(fut).detach();
    }
}
