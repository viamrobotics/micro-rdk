#![allow(dead_code)]
use std::{
    ffi::{c_char, c_int, c_uchar, c_uint, c_void},
    io::{self, Read, Write},
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use crate::common::webrtc::{certificate::Certificate, dtls::DtlsConnector, io::IoPktChannel};

use core::ffi::CStr;
use esp_idf_sys::{
    mbedtls_ctr_drbg_context, mbedtls_ctr_drbg_init, mbedtls_ctr_drbg_random,
    mbedtls_ctr_drbg_seed, mbedtls_entropy_context, mbedtls_entropy_func, mbedtls_entropy_init,
    mbedtls_pk_context, mbedtls_pk_init, mbedtls_pk_parse_key, mbedtls_ssl_conf_ca_chain,
    mbedtls_ssl_conf_dbg, mbedtls_ssl_conf_dtls_cookies,
    mbedtls_ssl_conf_dtls_srtp_protection_profiles, mbedtls_ssl_conf_own_cert,
    mbedtls_ssl_conf_rng, mbedtls_ssl_config, mbedtls_ssl_config_defaults, mbedtls_ssl_config_init,
    mbedtls_ssl_context, mbedtls_ssl_handshake, mbedtls_ssl_init, mbedtls_ssl_read,
    mbedtls_ssl_set_bio, mbedtls_ssl_set_timer_cb, mbedtls_ssl_setup, mbedtls_ssl_write,
    mbedtls_x509_crt, mbedtls_x509_crt_init, mbedtls_x509_crt_parse_der,
    MBEDTLS_ERR_NET_RECV_FAILED, MBEDTLS_ERR_NET_SEND_FAILED, MBEDTLS_ERR_SSL_WANT_READ,
    MBEDTLS_ERR_SSL_WANT_WRITE, MBEDTLS_SSL_IS_SERVER, MBEDTLS_SSL_PRESET_DEFAULT,
    MBEDTLS_SSL_TRANSPORT_DATAGRAM,
};
use futures_lite::{AsyncRead, AsyncWrite, Future};
use log::{log, Level};
use thiserror::Error;

extern "C" {
    fn mbedtls_debug_set_threshold(level: c_int);
}

pub struct SslStreamState<S> {
    pub stream: S,
    pub error: Option<std::io::Error>,
}

impl<S> SslStreamState<S>
where
    S: Read + Write,
{
    fn new(stream: S) -> Self {
        Self {
            stream,
            error: None,
        }
    }
}

unsafe fn state<'a, S: 'a>(ctx: *mut c_void) -> &'a mut SslStreamState<S> {
    &mut *(ctx as *mut _)
}

unsafe extern "C" fn mbedtls_net_write<S: Write>(
    ctx: *mut c_void,
    buf: *const c_uchar,
    len: usize,
) -> c_int {
    let state = state::<S>(ctx);

    let buf = std::slice::from_raw_parts(buf as *const _, len as usize);

    match state.stream.write(buf) {
        Ok(len) => len as c_int,
        Err(e) => {
            let _ = state.error.insert(e);
            if state.error.as_ref().unwrap().kind() == std::io::ErrorKind::WouldBlock {
                return MBEDTLS_ERR_SSL_WANT_WRITE;
            }
            MBEDTLS_ERR_NET_SEND_FAILED
        }
    }
}

unsafe extern "C" fn mbedtls_net_read<S: Read>(
    ctx: *mut c_void,
    buf: *mut c_uchar,
    len: usize,
) -> c_int {
    let state = state::<S>(ctx);

    let buf = std::slice::from_raw_parts_mut(buf as *mut _, len as usize);

    match state.stream.read(buf) {
        Ok(len) => len as c_int,
        Err(e) => {
            let _ = state.error.insert(e);
            if state.error.as_ref().unwrap().kind() == std::io::ErrorKind::WouldBlock {
                return MBEDTLS_ERR_SSL_WANT_READ;
            }
            MBEDTLS_ERR_NET_RECV_FAILED
        }
    }
}

unsafe extern "C" fn mbedtls_net_read_with_timeout<S: Read>(
    ctx: *mut c_void,
    buf: *mut c_uchar,
    len: usize,
    _: c_uint,
) -> c_int {
    // forward to read, we can't handle tiemout for now
    mbedtls_net_read::<S>(ctx, buf, len)
}

extern "C" fn ssl_debug(
    _: *mut c_void,
    level: c_int,
    file: *const c_char,
    line: c_int,
    msg: *const c_char,
) {
    let level = match level {
        5 => Level::Trace,
        4 => Level::Debug,
        3 => Level::Info,
        2 => Level::Warn,
        1 => Level::Error,
        _ => Level::Trace,
    };

    let file = unsafe { CStr::from_ptr(file).to_string_lossy() };
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };

    log!(level, "[mbedtls] {}:{} - {}", file, line, msg);
}

#[derive(Debug)]
struct Esp32DtlsDelay {
    intermediate: Option<Instant>,
    fin: Option<Instant>,
}

impl Default for Esp32DtlsDelay {
    fn default() -> Self {
        Self {
            intermediate: None,
            fin: None,
        }
    }
}

extern "C" fn mbedtls_timing_dtls_set_delay(
    data: *mut c_void,
    intermediate_ms: c_uint,
    fin_ms: c_uint,
) {
    let ctx: &mut Esp32DtlsDelay = unsafe { &mut *(data as *mut _) };

    if fin_ms == 0 {
        ctx.intermediate = None;
        ctx.fin = None
    } else {
        let now = Instant::now();
        let _ = ctx
            .intermediate
            .insert(now + Duration::from_millis(intermediate_ms as u64));
        let _ = ctx.fin.insert(now + Duration::from_millis(fin_ms as u64));
    }
}

extern "C" fn mbedtls_timing_get_delay(data: *mut c_void) -> c_int {
    let ctx: &mut Esp32DtlsDelay = unsafe { &mut *(data as *mut _) };

    if ctx.fin.is_none() {
        return -1;
    }
    let now = Instant::now();
    if now > *ctx.intermediate.as_ref().unwrap() {
        log::debug!("intermetidate timer expired");
        return 2;
    }
    if now > *ctx.fin.as_ref().unwrap() {
        log::debug!("final timer expired");
        return 1;
    }
    0
}
#[repr(u16)]
enum MbedTlsStrpProfile {
    MbedtlsSrtpUnsetProfile = 0,
    MbedtlsSrtpAes128CmHmacSha180,
    MbedtlsSrtpAes128CmHmacSha132,
    MbedtlsSrtpNullHmacSha180,
    MbedtlsSrtpNullHmacSha132,
}

#[derive(Default)]
pub(crate) struct SSLContext {
    dtls_entropy: Box<mbedtls_entropy_context>,
    drbg_ctx: Box<mbedtls_ctr_drbg_context>,
    ssl_ctx: Box<mbedtls_ssl_context>,
    ssl_config: Box<mbedtls_ssl_config>,
    x509: Box<mbedtls_x509_crt>,
    pk_ctx: Box<mbedtls_pk_context>,
    timer_ctx: Box<Esp32DtlsDelay>,
    strp_profiles: Box<[MbedTlsStrpProfile]>,
}

impl SSLContext {
    fn init<S: Certificate>(&mut self, certificate: Rc<S>) -> Result<(), SSLError> {
        log::debug!("initializing DTLS context");
        unsafe {
            mbedtls_ssl_init(self.ssl_ctx.as_mut());
            mbedtls_ssl_config_init(self.ssl_config.as_mut());
            mbedtls_x509_crt_init(self.x509.as_mut());
            mbedtls_pk_init(self.pk_ctx.as_mut());
            mbedtls_entropy_init(self.dtls_entropy.as_mut());
            mbedtls_ctr_drbg_init(self.drbg_ctx.as_mut());
        }
        let ret = unsafe {
            //TODO(RSDK-3058) we can avoid an allocation if we use the nocpy version
            mbedtls_x509_crt_parse_der(
                self.x509.as_mut(),
                certificate.get_der_certificate().as_ptr(),
                certificate.get_der_certificate().len(),
            )
        };
        if ret != 0 {
            return Err(SSLError::SSLCertParseFail(ret));
        }
        let ret = unsafe {
            mbedtls_pk_parse_key(
                self.pk_ctx.as_mut(),
                certificate.get_der_keypair().as_ptr(),
                certificate.get_der_keypair().len(),
                std::ptr::null(),
                0,
            )
        };
        if ret != 0 {
            return Err(SSLError::SSLKeyParseFail(ret));
        }

        let ret = unsafe {
            mbedtls_ctr_drbg_seed(
                self.drbg_ctx.as_mut(),
                Some(mbedtls_entropy_func),
                self.dtls_entropy.as_mut() as *mut mbedtls_entropy_context as *mut _,
                std::ptr::null(),
                0,
            )
        };
        if ret != 0 {
            return Err(SSLError::SSLEntropySeedFailure(ret));
        }

        let ret = unsafe {
            mbedtls_ssl_config_defaults(
                self.ssl_config.as_mut(),
                MBEDTLS_SSL_IS_SERVER as i32,
                MBEDTLS_SSL_TRANSPORT_DATAGRAM as i32,
                MBEDTLS_SSL_PRESET_DEFAULT as i32,
            )
        };
        if ret != 0 {
            return Err(SSLError::SSLConfigFailure(ret));
        }
        unsafe {
            mbedtls_ssl_conf_rng(
                self.ssl_config.as_mut(),
                Some(mbedtls_ctr_drbg_random),
                self.drbg_ctx.as_mut() as *mut mbedtls_ctr_drbg_context as *mut c_void,
            );
            // if wee need to debug the handshake
            // mbedtls_debug_set_threshold(0);
            mbedtls_ssl_conf_dbg(
                self.ssl_config.as_mut(),
                Some(ssl_debug),
                std::ptr::null_mut(),
            );

            // Cookie are disabled, we might want to re-enable them at a later stage or only accept ClientHello originating from
            // the selected pair (or any DTLS packets)
            // see 4.2.1.  Denial-of-Service Countermeasures (RFC 6347)
            mbedtls_ssl_conf_dtls_cookies(
                self.ssl_config.as_mut(),
                None,
                None,
                std::ptr::null_mut(),
            );

            //(TODO(npm)) Attempt to weak link mbedtls_ssl_conf_dbg
            //mbedtls_ssl_conf_dbg( &conf, my_debug, stdout );
            //mbedtls_ssl_conf_read_timeout(self.ssl_config.as_mut(), 10000);

            mbedtls_ssl_conf_ca_chain(
                self.ssl_config.as_mut(),
                self.x509.next,
                std::ptr::null_mut(),
            );
            let ret = mbedtls_ssl_conf_own_cert(
                self.ssl_config.as_mut(),
                self.x509.as_mut(),
                self.pk_ctx.as_mut(),
            );
            if ret != 0 {
                return Err(SSLError::SSLConfigFailure(ret));
            }
            if !self.strp_profiles.is_empty() {
                let ret = mbedtls_ssl_conf_dtls_srtp_protection_profiles(
                    self.ssl_config.as_mut(),
                    self.strp_profiles.as_ptr() as *const u16,
                );
                if ret != 0 {
                    return Err(SSLError::SSLSrtpConfigFailure(ret));
                }
            }
        }

        let ret = unsafe { mbedtls_ssl_setup(self.ssl_ctx.as_mut(), self.ssl_config.as_mut()) };
        if ret != 0 {
            return Err(SSLError::SSLConfigFailure(ret));
        }

        unsafe {
            mbedtls_ssl_set_timer_cb(
                self.ssl_ctx.as_mut(),
                self.timer_ctx.as_mut() as *mut Esp32DtlsDelay as *mut c_void,
                Some(mbedtls_timing_dtls_set_delay),
                Some(mbedtls_timing_get_delay),
            );
        };
        Ok(())
    }
    fn set_srtp_profiles(&mut self, profiles: [MbedTlsStrpProfile; 2]) {
        self.strp_profiles = Box::new(profiles);
    }
}

pub struct Esp32Dtls<C> {
    context: Box<SSLContext>,
    transport: Option<IoPktChannel>,
    certificate: Rc<C>,
}
#[derive(Error, Debug)]
pub enum SSLError {
    #[error("couldn't parse certificate")]
    SSLCertParseFail(i32),
    #[error("couldn't parse key")]
    SSLKeyParseFail(i32),
    #[error("ssl config failed")]
    SSLConfigFailure(i32),
    #[error("srtp config failed")]
    SSLSrtpConfigFailure(i32),
    #[error("entropy seed failed")]
    SSLEntropySeedFailure(i32),
    #[error("ssl other error")]
    SSLOtherError(i32),
    #[error("ssl wants read")]
    SSLWantsRead,
    #[error("ssl wants write")]
    SSLWantsWrite,
}

impl From<i32> for SSLError {
    fn from(value: i32) -> Self {
        if value == MBEDTLS_ERR_SSL_WANT_READ {
            SSLError::SSLWantsRead
        } else if value == MBEDTLS_ERR_SSL_WANT_WRITE {
            SSLError::SSLWantsWrite
        } else {
            SSLError::SSLOtherError(value)
        }
    }
}

impl<C> Esp32Dtls<C>
where
    C: Certificate,
{
    pub fn new(certificate: Rc<C>) -> Result<Self, SSLError> {
        let context = Box::new(SSLContext::default());
        Ok(Self {
            context,
            transport: None,
            certificate,
        })
    }

    fn init(&mut self) -> Result<(), SSLError> {
        self.context.set_srtp_profiles([
            MbedTlsStrpProfile::MbedtlsSrtpAes128CmHmacSha180,
            MbedTlsStrpProfile::MbedtlsSrtpUnsetProfile,
        ]);
        self.context.init(self.certificate.clone())?;

        Ok(())
    }

    pub(crate) fn get_context(self) -> Box<SSLContext> {
        self.context
    }
}

pub struct DtlsStream<S> {
    context: Box<SSLContext>,
    bio_ptr: *mut c_void,
    _p: PhantomData<S>,
}

impl<S> DtlsStream<S>
where
    S: Read + Write,
{
    pub(crate) fn new(mut context: Box<SSLContext>, stream: S) -> Result<Self, SSLError> {
        let bio_ptr = Box::new(SslStreamState::new(stream));
        let bio_ptr = Box::into_raw(bio_ptr) as *mut c_void;
        unsafe {
            mbedtls_ssl_set_bio(
                context.ssl_ctx.as_mut(),
                bio_ptr,
                Some(mbedtls_net_write::<S>),
                Some(mbedtls_net_read::<S>),
                // we don't set a read  timeout and mbedtls_net_read_with_timeout forward to mbedtls_net_read
                Some(mbedtls_net_read_with_timeout::<S>),
            )
        }
        Ok(Self {
            context,
            bio_ptr,
            _p: PhantomData,
        })
    }

    pub(crate) fn get_inner_mut(&mut self) -> &mut S {
        let state = unsafe { state::<S>(self.bio_ptr) };
        &mut state.stream
    }

    pub(crate) fn handshake(&mut self) -> Result<(), SSLError> {
        let ret: i32 = unsafe { mbedtls_ssl_handshake(self.context.ssl_ctx.as_mut()) };
        if ret == 0 {
            Ok(())
        } else {
            if !(ret == MBEDTLS_ERR_SSL_WANT_READ || ret == MBEDTLS_ERR_SSL_WANT_WRITE) {
                log::error!("handshake error {:?}", ret);
            }
            Err(ret.into())
        }
    }

    fn ssl_read(&mut self, buf: &mut [u8]) -> Result<usize, SSLError> {
        if buf.is_empty() {
            return Ok(0);
        }

        let len = buf.len();

        // There might be leftover dtls records mbedtls_ssl_check_pending would tell us if anything is left
        // however subsequent call to ssl_read until WouldBlock is returned by the io should exhaust remaining records
        let ret: i32 =
            unsafe { mbedtls_ssl_read(self.context.ssl_ctx.as_mut(), buf.as_mut_ptr(), len) }
                as i32;

        if ret >= 0 {
            Ok(ret as usize)
        } else {
            Err(ret.into())
        }
    }

    fn ssl_write(&mut self, buf: &[u8]) -> Result<usize, SSLError> {
        // we skip sending empty records
        if buf.is_empty() {
            return Ok(0);
        }

        let len = buf.len();

        // if returning WANTS_READ/WANTS_WRITE we use the stored error to find out if it came from
        // the network layer (eg a call returned WouldBlock)
        // partial write are dealt with in an higher level call
        let ret: i32 =
            unsafe { mbedtls_ssl_write(self.context.ssl_ctx.as_mut(), buf.as_ptr(), len) } as i32;

        // if  MBEDTLS_ERR_SSL_BAD_INPUT_DATA is returned, mbedtls_ssl_get_max_out_record_payload() should be used to query
        // the active maximum fragment length
        if ret >= 0 {
            Ok(ret as usize)
        } else {
            Err(ret.into())
        }
    }
}

impl<S> Read for DtlsStream<S>
where
    S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.ssl_read(buf) {
                Ok(n) => return Ok(n),
                Err(e) => match e {
                    SSLError::SSLWantsRead | SSLError::SSLWantsWrite => {
                        if let Some(state) = unsafe { state::<S>(self.bio_ptr) }.error.take() {
                            return Err(state);
                        }
                    }
                    _ => {
                        return Err(io::Error::new(io::ErrorKind::Other, e));
                    }
                },
            }
        }
    }
}

impl<S> Write for DtlsStream<S>
where
    S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        loop {
            match self.ssl_write(&buf[written..]) {
                Ok(n) => {
                    if n == buf[written..].len() {
                        return Ok(buf.len()); // might be wrong?
                    }
                    written = n - 1;
                    log::error!(
                        "partial write wanted {} did {} remaining {}",
                        &buf[written..].len(),
                        n,
                        written
                    );
                }
                Err(e) => match e {
                    SSLError::SSLWantsRead | SSLError::SSLWantsWrite => {
                        if let Some(state) = unsafe { state::<S>(self.bio_ptr) }.error.take() {
                            return Err(state);
                        }
                    }
                    _ => {
                        return Err(io::Error::new(io::ErrorKind::Other, e));
                    }
                },
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        let state = unsafe { state::<S>(self.bio_ptr) };
        state.stream.flush()
    }
}

struct AsyncInnerStreamWrapper<S> {
    stream: S,
    context: Option<*mut c_void>,
}

impl<S> AsyncInnerStreamWrapper<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn new(stream: S) -> Self {
        Self {
            stream,
            context: None,
        }
    }
}

impl<S> AsyncInnerStreamWrapper<S> {
    unsafe fn as_parts(&mut self) -> (Pin<&mut S>, &mut Context<'_>) {
        debug_assert!(self.context.is_some());
        let c = &mut *(self.context.unwrap() as *mut Context);
        let s = Pin::new_unchecked(&mut self.stream);

        (s, c)
    }
}

impl<S> Write for AsyncInnerStreamWrapper<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let (s, c) = unsafe { self.as_parts() };
        match s.poll_write(c, buf) {
            Poll::Ready(ret) => ret,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        let (s, c) = unsafe { self.as_parts() };
        match s.poll_flush(c) {
            Poll::Ready(ret) => ret,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}

impl<S> Read for AsyncInnerStreamWrapper<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (s, c) = unsafe { self.as_parts() };
        match s.poll_read(c, buf) {
            Poll::Ready(ret) => ret,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}

pub struct AsyncDtlsStream<S>(DtlsStream<AsyncInnerStreamWrapper<S>>);

impl<S> AsyncDtlsStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    pub(crate) fn new(context: Box<SSLContext>, stream: S) -> Result<Self, SSLError> {
        DtlsStream::new(context, AsyncInnerStreamWrapper::new(stream)).map(Self)
    }

    fn save_context<F, R>(self: Pin<&mut Self>, ctx: &mut Context<'_>, f: F) -> R
    where
        F: FnOnce(&mut DtlsStream<AsyncInnerStreamWrapper<S>>) -> R,
    {
        let this = unsafe { self.get_unchecked_mut() };

        let _ = this
            .0
            .get_inner_mut()
            .context
            .insert(ctx as *mut _ as *mut c_void);
        let r = f(&mut this.0);
        let _ = this.0.get_inner_mut().context.take();
        r
    }

    pub fn poll_accept(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), SSLError>> {
        self.save_context(cx, |s| match s.handshake() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e {
                SSLError::SSLWantsRead | SSLError::SSLWantsWrite => Poll::Pending,
                _ => Poll::Ready(Err(e)),
            },
        })
    }
    pub async fn accept(mut self: Pin<&mut Self>) -> Result<(), SSLError> {
        futures_lite::future::poll_fn(|cx| self.as_mut().poll_accept(cx)).await
    }
}

impl<C> DtlsConnector for Esp32Dtls<C>
where
    C: Certificate,
{
    type Error = SSLError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Stream, Self::Error>>>>;
    type Stream = AsyncDtlsStream<IoPktChannel>;
    fn accept(mut self) -> Self::Future {
        let transport = self.transport.take().unwrap();

        //TODO(npm) consider returning and error
        self.init().unwrap();

        let mut stream = AsyncDtlsStream::new(self.get_context(), transport).unwrap();

        Box::pin(async move {
            Pin::new(&mut stream).accept().await?;
            Ok(stream)
        })
    }
    fn set_transport(&mut self, transport: IoPktChannel) {
        let _ = self.transport.insert(transport);
    }
}

unsafe impl<S> Send for AsyncDtlsStream<S> {}

impl<S> AsyncRead for AsyncDtlsStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.save_context(cx, |s| match s.read(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Poll::Pending;
                }
                Poll::Ready(Err(e))
            }
        })
    }
}

impl<S> AsyncWrite for AsyncDtlsStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        //TODO(npm) : should check if buf len fits in max frag length negotiated
        // should use mbedtls_ssl_get_max_out_record_payload()
        self.save_context(cx, |s| match s.write(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Poll::Pending;
                }
                Poll::Ready(Err(e))
            }
        })
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.save_context(cx, |s| match s.flush() {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Poll::Pending;
                }
                Poll::Ready(Err(e))
            }
        })
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        // TODO(RSDK-3059) implement
        Poll::Ready(Ok(()))
    }
}
