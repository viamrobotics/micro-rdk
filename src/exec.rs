use futures_lite::{future, Future};
use smol::{LocalExecutor, Task};
use std::rc::Rc;
#[derive(Clone, Debug)]
pub struct Esp32Executor<'a> {
    executor: Rc<LocalExecutor<'a>>,
}

impl<'a> Esp32Executor<'a> {
    pub fn new() -> Self {
        Esp32Executor {
            executor: Rc::new(LocalExecutor::new()),
        }
    }
    pub fn spawn<T: 'a>(&self, future: impl Future<Output = T> + 'a) -> Task<T> {
        self.executor.spawn(future)
    }
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        self.executor.run(future).await
    }
}

impl<F> hyper::rt::Executor<F> for Esp32Executor<'_>
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        self.executor.spawn(fut).detach();
    }
}
