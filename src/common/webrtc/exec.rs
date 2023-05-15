use futures_lite::Future;

pub trait WebRtcExecutor<F>
where
    F: futures_lite::Future + 'static,
{
    fn execute(&self, fut: F);
    fn block_on<T>(&self, fut: impl Future<Output = T>) -> T;
}
