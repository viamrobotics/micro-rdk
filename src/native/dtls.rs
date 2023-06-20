use std::io::Write;
use std::pin::Pin;

use std::time::SystemTime;
use std::{fs::OpenOptions, rc::Rc};

use async_std_openssl::SslStream;

use futures_lite::Future;
use openssl::ec::EcKey;

use openssl::nid::Nid;
use openssl::pkey::PKey;

use openssl::ssl::{
    Ssl, SslContext, SslContextBuilder, SslMethod, SslOptions, SslRef, SslVerifyMode,
};

use crate::common::webrtc::certificate::Certificate;
use crate::common::webrtc::dtls::{DtlsBuilder, DtlsConnector};
use crate::common::webrtc::io::IoPktChannel;

fn dtls_log_session_key(_: &SslRef, line: &str) {
    log::info!("Loggin key data");
    if let Ok(file) = std::env::var("SSLKEYLOGFILE") {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .append(true)
            .truncate(false)
            .open(file)
        {
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
    transport: Option<IoPktChannel>,
}

impl Drop for Dtls {
    fn drop(&mut self) {
        log::info!("dropped dtls");
    }
}

pub fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl Dtls {
    pub fn new<S: Certificate>(cert: Rc<S>) -> anyhow::Result<Self> {
        let mut ssl_ctx_builder = SslContextBuilder::new(SslMethod::dtls())?;
        let mut verify = SslVerifyMode::empty();
        verify.insert(SslVerifyMode::PEER);
        verify.insert(SslVerifyMode::FAIL_IF_NO_PEER_CERT);

        ssl_ctx_builder.set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80")?;

        ssl_ctx_builder.set_verify_callback(verify, |_ok, _ctx| true);
        ssl_ctx_builder.set_keylog_callback(dtls_log_session_key);

        let x509 = openssl::x509::X509::from_der(cert.get_der_certificate())?;

        let pkey = PKey::private_key_from_der(cert.get_der_keypair())?;
        ssl_ctx_builder.set_private_key(&pkey)?;

        let cert0 = x509;

        ssl_ctx_builder.set_certificate(&cert0)?;

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

impl DtlsConnector for Dtls {
    type Error = openssl::ssl::Error;
    type Stream = SslStream<IoPktChannel>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Stream, Self::Error>>>>;
    fn accept(self) -> Self::Future {
        let mut ssl = Ssl::new(&self.context).unwrap();
        ssl.set_accept_state();
        let eckey = EcKey::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        ssl.set_tmp_ecdh(&eckey).unwrap();
        log::error!("accepting");

        let transport = self.transport.as_ref().unwrap().clone();

        let mut stream = async_std_openssl::SslStream::new(ssl, transport).unwrap();

        Box::pin(async move {
            let pin = Pin::new(&mut stream);
            pin.accept().await?;
            Ok(stream)
        })
    }
    fn set_transport(&mut self, transport: IoPktChannel) {
        let _ = self.transport.insert(transport);
    }
}

impl<C: Certificate> DtlsBuilder for NativeDtls<C> {
    type Output = Dtls;
    fn make(&self) -> anyhow::Result<Self::Output> {
        Dtls::new(self.cert.clone())
    }
}
