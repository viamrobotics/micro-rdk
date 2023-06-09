use std::{
    io::{BufReader, Read, Write},
    net::TcpStream,
    sync::Arc,
};

use rustls::{
    ClientConfig, ClientConnection, KeyLogFile, OwnedTrustAnchor, RootCertStore, ServerConfig,
    ServerConnection, StreamOwned,
};

/// structure to store tls configuration
#[derive(Clone)]
pub struct NativeTls {
    server_config: Option<NativeTlsServerConfig>,
}

enum NativeTlsStreamRole {
    Server(StreamOwned<ServerConnection, TcpStream>),
    Client(StreamOwned<ClientConnection, TcpStream>),
}

/// TCP like stream for encrypted communication over TLS
pub struct NativeTlsStream {
    socket: Option<TcpStream>, // may store the raw socket
    stream: Box<NativeTlsStreamRole>,
}

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
    pub fn open_ssl_context(&self, socket: Option<TcpStream>) -> anyhow::Result<NativeTlsStream> {
        NativeTlsStream::new(socket, &self.server_config)
    }
}

impl TlsClientConnector for NativeTls {
    type Stream = NativeStream;
    fn connect(&mut self) -> Result<Self::Stream, crate::common::conn::server::ServerError> {
        Ok(NativeStream::TLSStream(Box::new(
            self.open_ssl_context(None)
                .map_err(|e| ServerError::Other(e.into()))?,
        )))
    }
}

use rustls::KeyLog;

use crate::common::conn::server::{ServerError, TlsClientConnector};

use super::tcp::NativeStream;

struct Key {}
impl KeyLog for Key {
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        log::info!("{} {:?} {:?}", label, client_random, secret);
    }
    fn will_log(&self, _label: &str) -> bool {
        true
    }
}

/// Esp32TlsStream represents a properly established TLS connection to a server or a client. It can be use bye Esp32TCPStream since it
/// implements std::io::{Read,Write}
impl NativeTlsStream {
    /// based on a role and a configuration, attempt the setup an SSL context
    fn new(
        socket: Option<TcpStream>,
        tls_cfg: &Option<NativeTlsServerConfig>,
    ) -> anyhow::Result<Self> {
        let (stream, socket) = if let Some(tls_cfg) = tls_cfg {
            let cert_chain =
                rustls_pemfile::certs(&mut BufReader::new(tls_cfg.srv_cert.as_slice()))
                    .unwrap()
                    .iter()
                    .map(|c| rustls::Certificate(c.clone()))
                    .collect();
            let cert_key =
                match rustls_pemfile::read_one(&mut BufReader::new(tls_cfg.srv_key.as_slice()))
                    .expect("cannot parse private key pem file")
                {
                    Some(rustls_pemfile::Item::RSAKey(key)) => rustls::PrivateKey(key),
                    Some(rustls_pemfile::Item::PKCS8Key(key)) => rustls::PrivateKey(key),
                    Some(rustls_pemfile::Item::ECKey(key)) => rustls::PrivateKey(key),
                    None => return Err(anyhow::anyhow!("private key couldn't be parsed")),
                    _ => return Err(anyhow::anyhow!("unexpected private key type")),
                };
            let mut cfg = ServerConfig::builder()
                .with_safe_default_cipher_suites()
                .with_safe_default_kx_groups()
                .with_protocol_versions(&[&rustls::version::TLS12])?
                .with_no_client_auth()
                .with_single_cert(cert_chain, cert_key)?;
            cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
            let mut conn = ServerConnection::new(Arc::new(cfg))?;
            let mut socket = socket.unwrap();
            socket.set_nonblocking(false)?;
            let _r = conn.complete_io::<TcpStream>(&mut socket).unwrap();
            socket.set_nonblocking(true)?;
            let stream = StreamOwned::new(conn, socket);
            (NativeTlsStreamRole::Server(stream), None)
        } else {
            let mut root_certs = RootCertStore::empty();
            root_certs.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(
                |ta| {
                    OwnedTrustAnchor::from_subject_spki_name_constraints(
                        ta.subject,
                        ta.spki,
                        ta.name_constraints,
                    )
                },
            ));
            let log = Arc::new(KeyLogFile::new());
            let mut cfg = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_certs)
                .with_no_client_auth();
            cfg.alpn_protocols = vec!["h2".as_bytes().to_vec()];
            cfg.key_log = log;
            let mut conn = ClientConnection::new(Arc::new(cfg), "app.viam.com".try_into()?)?;
            let mut socket = TcpStream::connect("app.viam.com:443")?;
            conn.complete_io::<TcpStream>(&mut socket)?;
            socket.set_nonblocking(true)?;
            let stream = StreamOwned::new(conn, socket);
            (NativeTlsStreamRole::Client(stream), None)
        };
        Ok(Self {
            socket,
            stream: Box::new(stream),
        })
    }
}

impl Read for NativeTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut *self.stream {
            NativeTlsStreamRole::Server(r) => r.read(buf),
            NativeTlsStreamRole::Client(r) => r.read(buf),
        }
    }
}

impl Write for NativeTlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut *self.stream {
            NativeTlsStreamRole::Server(r) => r.write(buf),
            NativeTlsStreamRole::Client(r) => r.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.socket {
            Some(s) => s.flush(),
            None => Ok(()),
        }
    }
}

impl Drop for NativeTlsStream {
    fn drop(&mut self) {
        log::info!("dropping the tls stream");
    }
}
