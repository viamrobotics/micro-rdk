pub trait WebRtcExecutor<F>
where
    F: futures_lite::Future + 'static,
{
    fn execute(&self, fut: F);
}
