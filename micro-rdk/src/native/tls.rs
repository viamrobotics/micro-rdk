use std::{io::BufReader, net::TcpStream, sync::Arc};

use async_io::Async;
use futures_lite::AsyncRead;
use futures_lite::AsyncWrite;
use futures_rustls::{TlsAcceptor, TlsConnector};
use rustls::{ClientConfig, KeyLogFile, OwnedTrustAnchor, RootCertStore, ServerConfig};

/// structure to store tls configuration
#[derive(Clone)]
pub struct NativeTls {
    server_config: Option<NativeTlsServerConfig>,
}

/// TCP like stream for encrypted communication over TLS
pub struct NativeTlsStream(futures_rustls::TlsStream<Async<TcpStream>>);

#[derive(Clone, Debug, Default)]
pub struct NativeTlsServerConfig {
    srv_cert: Vec<u8>,
    srv_key: Vec<u8>,
}

impl NativeTlsServerConfig {
    pub fn new(srv_cert: Vec<u8>, srv_key: Vec<u8>) -> Self {
        NativeTlsServerConfig { srv_cert, srv_key }
    }
}

impl NativeTls {
    pub fn new_client() -> Self {
        Self {
            server_config: None,
        }
    }
    /// Creates a TLS object ready to accept connection or connect to a server
    pub fn new_server(cfg: NativeTlsServerConfig) -> Self {
        Self {
            server_config: Some(cfg),
        }
    }

    /// open the a TLS (SSL) context either in client or in server mode
    pub async fn open_ssl_context(
        &self,
        socket: Option<TcpStream>,
    ) -> Result<NativeTlsStream, std::io::Error> {
        NativeTlsStream::accept_or_connect(socket, &self.server_config).await
    }
}

impl TlsClientConnector for NativeTls {
    type Stream = NativeStream;
    async fn connect(&mut self) -> Result<Self::Stream, ServerError> {
        Ok(NativeStream::TLSStream(Box::new(
            self.open_ssl_context(None)
                .await
                .map_err(|e| ServerError::Other(e.into()))?,
        )))
    }
}

use crate::common::conn::errors::ServerError;
use crate::common::conn::server::TlsClientConnector;

use super::tcp::NativeStream;

impl NativeTlsStream {
    /// based on a role and a configuration, attempt the setup an SSL context
    async fn accept_or_connect(
        socket: Option<TcpStream>,
        tls_cfg: &Option<NativeTlsServerConfig>,
    ) -> Result<Self, std::io::Error> {
        let stream = if let Some(tls_cfg) = tls_cfg {
            let cert_chain =
                rustls_pemfile::certs(&mut BufReader::new(tls_cfg.srv_cert.as_slice()))
                    .unwrap()
                    .iter()
                    .map(|c| rustls::Certificate(c.clone()))
                    .collect();

            let mut cfg = ServerConfig::builder()
                .with_safe_default_cipher_suites()
                .with_safe_default_kx_groups()
                .with_protocol_versions(&[&rustls::version::TLS12])
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?
                .with_no_client_auth()
                .with_single_cert(cert_chain, rustls::PrivateKey(tls_cfg.srv_key.clone()))
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
            cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
            let stream = async_io::Async::new(socket.unwrap())?;
            let conn = TlsAcceptor::from(Arc::new(cfg));
            let stream = conn.accept(stream).await?;

            futures_rustls::TlsStream::Server(stream)
        } else {
            let mut root_certs = RootCertStore::empty();
            root_certs.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
                OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }));
            let log = Arc::new(KeyLogFile::new());
            let mut cfg = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_certs)
                .with_no_client_auth();
            cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
            cfg.key_log = log;
            let stream = async_io::Async::new(TcpStream::connect("app.viam.com:443")?)?;
            let conn = TlsConnector::from(Arc::new(cfg));
            let stream = conn
                .connect(
                    "app.viam.com"
                        .try_into()
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?,
                    stream,
                )
                .await
                .unwrap();

            futures_rustls::TlsStream::Client(stream)
        };
        Ok(NativeTlsStream(stream))
    }
}

impl AsyncRead for NativeTlsStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let s = &mut std::pin::Pin::into_inner(self).0;
        futures_lite::pin!(s);
        s.poll_read(cx, buf)
    }
}

impl AsyncWrite for NativeTlsStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let s = &mut std::pin::Pin::into_inner(self).0;
        futures_lite::pin!(s);
        s.poll_write(cx, buf)
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let s = &mut std::pin::Pin::into_inner(self).0;
        futures_lite::pin!(s);
        s.poll_flush(cx)
    }
    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let s = &mut std::pin::Pin::into_inner(self).0;
        futures_lite::pin!(s);
        s.poll_close(cx)
    }
}

impl Drop for NativeTlsStream {
    fn drop(&mut self) {
        log::info!("dropping the tls stream");
    }
}
