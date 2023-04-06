pub trait WebRTCExecutor<F>
where
    F: futures_lite::Future + 'static,
{
    fn execute(&self, fut: F);
}
