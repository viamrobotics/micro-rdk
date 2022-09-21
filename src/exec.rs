use futures_lite::future;
use smol::LocalExecutor;
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
}

impl<F> hyper::rt::Executor<F> for Esp32Executor<'_>
where
    F: future::Future + 'static,
{
    fn execute(&self, fut: F) {
        let t = self.executor.spawn(fut);
        future::block_on(self.executor.run(async { t.await }));
    }
}
