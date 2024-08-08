//! The exec module exposes helpers to execute futures
use async_executor::{LocalExecutor, Task};
use futures_lite::{
    future::{self, block_on},
    Future,
};

use crate::common::{provisioning::server::ProvisioningExecutor, webrtc::exec::WebRtcExecutor};

#[derive(Clone, Debug, Default)]
/// This executor is local and bounded to the CPU that created it usually you would create it after spwaning a thread on a specific core
pub struct Executor {}

std::thread_local! {
    static EX: LocalExecutor<'static> = const { LocalExecutor::new() };
}

impl Executor {
    pub fn new() -> Self {
        Self {}
    }
    // Spawn a future onto the local executor
    pub fn spawn<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> Task<T> {
        EX.with(|e| e.spawn(future))
    }

    pub fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        EX.with(|e| block_on(e.run(future)))
    }
}

/// helper trait for hyper to spwan future onto a local executor
impl<F> hyper::rt::Executor<F> for Executor
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        EX.with(|e| e.spawn(fut)).detach();
    }
}

impl<F> WebRtcExecutor<F> for Executor
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        EX.with(|e| e.spawn(fut)).detach();
    }
}

impl ProvisioningExecutor for Executor {
    fn spawn<F: future::Future<Output = ()> + 'static>(&self, future: F) -> Task<()> {
        self.spawn(future)
    }
}
