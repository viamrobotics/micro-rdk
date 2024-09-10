use std::io::Write;
use std::pin::Pin;

use std::task::Poll;
use std::time::SystemTime;
use std::{fs::OpenOptions, rc::Rc};

use futures_lite::Future;
use openssl::ec::EcKey;

use openssl::nid::Nid;
use openssl::pkey::PKey;

use openssl::ssl::{
    Ssl, SslContext, SslContextBuilder, SslMethod, SslOptions, SslRef, SslVerifyMode,
};

use crate::common::webrtc::certificate::Certificate;
use crate::common::webrtc::dtls::{
    DtlsBuilder, DtlsConnector, DtlsError, DtlsStream, IntoDtlsStream,
};
use crate::common::webrtc::udp_mux::UdpMux;

fn dtls_log_session_key(_: &SslRef, line: &str) {
    log::info!("Loggin key data");
    if let Ok(file) = std::env::var("SSLKEYLOGFILE") {
        if let Ok(mut file) = OpenOptions::new().append(true).truncate(false).open(file) {
            let _ = file.write(line.as_bytes()).unwrap();
            let _ = file.write(b"\n").unwrap();
        }
    }
}

pub struct NativeDtls<C: Certificate> {
    cert: Rc<C>,
}

impl<C: Certificate> NativeDtls<C> {
    pub fn new(cert: Rc<C>) -> Self {
        Self { cert }
    }
}

pub struct Dtls {
    pub context: SslContext,
    transport: Option<UdpMux>,
}

impl Drop for Dtls {
    fn drop(&mut self) {
        log::error!("dropped dtls");
    }
}

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl Dtls {
    pub fn new<S: Certificate>(cert: Rc<S>) -> Result<Self, DtlsError> {
        let mut ssl_ctx_builder = SslContextBuilder::new(SslMethod::dtls())
            .map_err(|e| DtlsError::DtlsError(Box::new(e)))?;
        let mut verify = SslVerifyMode::empty();
        verify.insert(SslVerifyMode::PEER);
        verify.insert(SslVerifyMode::FAIL_IF_NO_PEER_CERT);

        ssl_ctx_builder
            .set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80")
            .map_err(|e| DtlsError::DtlsError(Box::new(e)))?;

        ssl_ctx_builder.set_verify_callback(verify, |_ok, _ctx| true);
        ssl_ctx_builder.set_keylog_callback(dtls_log_session_key);

        let x509 = openssl::x509::X509::from_der(cert.get_der_certificate())
            .map_err(|e| DtlsError::DtlsError(Box::new(e)))?;

        let pkey = PKey::private_key_from_der(cert.get_der_keypair())
            .map_err(|e| DtlsError::DtlsError(Box::new(e)))?;
        ssl_ctx_builder
            .set_private_key(&pkey)
            .map_err(|e| DtlsError::DtlsError(Box::new(e)))?;

        let cert0 = x509;

        ssl_ctx_builder
            .set_certificate(&cert0)
            .map_err(|e| DtlsError::DtlsError(Box::new(e)))?;

        let mut dtls_options = SslOptions::empty();
        dtls_options.insert(SslOptions::SINGLE_ECDH_USE);
        dtls_options.insert(SslOptions::NO_DTLSV1);

        ssl_ctx_builder.set_options(dtls_options);

        let dtls_ctx = ssl_ctx_builder.build();

        Ok(Self {
            context: dtls_ctx,
            transport: None,
        })
    }
}

pub struct DtlsAcceptor(Option<async_std_openssl::SslStream<UdpMux>>);
impl IntoDtlsStream for DtlsAcceptor {}

impl Future for DtlsAcceptor {
    type Output = Result<Box<dyn DtlsStream>, DtlsError>;
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = std::pin::Pin::new(self.0.as_mut().unwrap());

        let result = futures_lite::ready!(this.poll_accept(cx));
        match result {
            Ok(()) => Poll::Ready(Ok(Box::new(self.0.take().unwrap()))),
            Err(e) => Poll::Ready(Err(DtlsError::DtlsError(Box::new(e)))),
        }
    }
}

impl DtlsConnector for Dtls {
    fn accept(&mut self) -> Result<std::pin::Pin<Box<dyn IntoDtlsStream>>, DtlsError> {
        let mut ssl = Ssl::new(&self.context).unwrap();
        ssl.set_accept_state();
        let eckey = EcKey::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        ssl.set_tmp_ecdh(&eckey).unwrap();
        log::error!("accepting");

        let transport = self.transport.take().unwrap();

        let stream = async_std_openssl::SslStream::new(ssl, transport).unwrap();
        Ok(Box::pin(DtlsAcceptor(Some(stream))))
    }
    fn set_transport(&mut self, transport: UdpMux) {
        let _ = self.transport.insert(transport);
    }
}

impl<C: Certificate> DtlsBuilder for NativeDtls<C> {
    fn make(&self) -> Result<Box<dyn DtlsConnector>, DtlsError> {
        Ok(Box::new(Dtls::new(self.cert.clone())?))
    }
}
