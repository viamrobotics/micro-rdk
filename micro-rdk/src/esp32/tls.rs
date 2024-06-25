use crate::esp32::esp_idf_svc::sys::{
    esp_tls_cfg, esp_tls_cfg_server, esp_tls_conn_destroy, esp_tls_conn_new_sync,
    esp_tls_conn_state_ESP_TLS_CONNECTING as ESP_TLS_CONNECTING,
    esp_tls_conn_state_ESP_TLS_DONE as ESP_TLS_DONE,
    esp_tls_conn_state_ESP_TLS_FAIL as ESP_TLS_FAIL,
    esp_tls_conn_state_ESP_TLS_HANDSHAKE as ESP_TLS_HANDSHAKE,
    esp_tls_conn_state_ESP_TLS_INIT as ESP_TLS_INIT, esp_tls_init, esp_tls_server_session_create,
    esp_tls_t, EspError, ESP_TLS_ERR_SSL_WANT_READ, ESP_TLS_ERR_SSL_WANT_WRITE,
};
use async_io::Async;
use either::Either;
use esp_idf_svc::sys::{
    esp_tls_get_conn_sockfd, lwip_setsockopt, socklen_t, IPPROTO_TCP, SOL_SOCKET, SO_KEEPALIVE,
    TCP_KEEPCNT, TCP_KEEPIDLE, TCP_KEEPINTVL,
};
use futures_lite::{ready, AsyncRead, AsyncWrite};

use std::{
    fmt::Debug,
    io::{Read, Write},
    net::TcpStream,
    ops::Deref,
    os::{fd::FromRawFd, raw::c_char, unix::prelude::AsRawFd},
    task::Poll,
};

use crate::common::conn::errors::ServerError;
use crate::common::conn::server::TlsClientConnector;

use super::tcp::Esp32Stream;

unsafe impl Sync for Esp32TLS {}
unsafe impl Send for Esp32TLS {}

const TCP_KEEPINTVL_S: i32 = 60; // seconds
const TCP_KEEPCNT_N: i32 = 4;
const TCP_KEEPIDLE_S: i32 = 120; // seconds

/// structure to store tls configuration
#[derive(Clone)]
pub struct Esp32TLS {
    #[allow(dead_code)]
    alpn_ptr: Vec<*const c_char>,
    tls_cfg: Either<Box<esp_tls_cfg_server>, Box<esp_tls_cfg>>,
}

impl TlsClientConnector for Esp32TLS {
    type Stream = Esp32Stream;
    async fn connect(&mut self) -> Result<Self::Stream, ServerError> {
        Ok(Esp32Stream::TLSStream(Box::new(
            self.open_ssl_context(None)
                .map_err(|e| ServerError::Other(e.into()))?,
        )))
    }
}

struct Esp32TLSContext(*mut esp_tls_t);

impl Esp32TLSContext {
    fn new() -> Result<Self, std::io::Error> {
        let p = unsafe { esp_tls_init() };
        if p.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "couldn't allocate tls context",
            ));
        }
        Ok(Self(p))
    }
}

impl Deref for Esp32TLSContext {
    type Target = *mut esp_tls_t;
    fn deref(&self) -> &Self::Target {
        &(self.0)
    }
}

impl Drop for Esp32TLSContext {
    fn drop(&mut self) {
        log::error!("dropping the tls stream");
        if let Some(err) = EspError::from(unsafe { esp_tls_conn_destroy(self.0) }) {
            log::error!("error while dropping the tls connection '{}'", err);
        }
    }
}

/// TCP like stream for encrypted communication over TLS
pub struct Esp32TLSStream {
    tls_context: Esp32TLSContext,
    socket: Async<TcpStream>, // may store the raw socket
}

pub struct Esp32TLSServerConfig {
    srv_cert: Vec<u8>,
    srv_key: *const u8,
    srv_key_len: u32,
}

impl Esp32TLSServerConfig {
    // An Esp32TlsServerConfig takes a certificate and key bytearray (in the form of a pointer and length)
    // The PEM certificate has two parts: the first is the certificate chain and the second is the
    // certificate authority.
    pub fn new(srv_cert: Vec<u8>, srv_key: *const u8, srv_key_len: u32) -> Self {
        Esp32TLSServerConfig {
            srv_cert,
            srv_key,
            srv_key_len,
        }
    }
}

impl Debug for Esp32TLSStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Esp32TlsStream")
            .field(
                "tls",
                match unsafe { (*(*(self.tls_context))).conn_state } {
                    ESP_TLS_INIT => &"Tls initializing",
                    ESP_TLS_CONNECTING => &"Tls connecting",
                    ESP_TLS_HANDSHAKE => &"Tls handshake",
                    ESP_TLS_FAIL => &"Tls fail",
                    ESP_TLS_DONE => &"Tls closed",
                    _ => &"unexpected tls error",
                },
            )
            .finish()
    }
}

static ALPN_PROTOCOLS: &[u8] = b"h2\0";
static APP_VIAM_HOSTNAME: &[u8] = b"app.viam.com\0";

impl Esp32TLS {
    pub fn new_client() -> Self {
        let mut alpn_ptr: Vec<_> = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];
        // this is a root certificate to validate the server's certificate
        let cert = include_bytes!("../../certs/google_gts_root_r1.crt");

        let tls_cfg_client = Box::new(esp_tls_cfg {
            alpn_protos: alpn_ptr.as_mut_ptr(),
            __bindgen_anon_1: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_1 {
                cacert_buf: cert.as_ptr(),
            },
            __bindgen_anon_2: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_2 {
                cacert_bytes: cert.len() as u32,
            },
            __bindgen_anon_3: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_3 {
                clientcert_buf: std::ptr::null(),
            },
            __bindgen_anon_4: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_4 {
                clientcert_bytes: 0_u32,
            },
            __bindgen_anon_5: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_5 {
                clientkey_buf: std::ptr::null(),
            },
            __bindgen_anon_6: crate::esp32::esp_idf_svc::sys::esp_tls_cfg__bindgen_ty_6 {
                clientkey_bytes: 0_u32,
            },
            clientkey_password: std::ptr::null(),
            clientkey_password_len: 0_u32,
            non_block: true,
            use_secure_element: false,
            use_global_ca_store: false,
            skip_common_name: false,
            keep_alive_cfg: std::ptr::null_mut(),
            psk_hint_key: std::ptr::null(),
            crt_bundle_attach: None,
            ds_data: std::ptr::null_mut(),
            if_name: std::ptr::null_mut(),
            is_plain_tcp: false,
            timeout_ms: 50000,
            common_name: std::ptr::null(),
        });

        Self {
            alpn_ptr,
            tls_cfg: Either::Right(tls_cfg_client),
        }
    }
    /// Creates a TLS object ready to accept connection or connect to a server
    pub fn new_server(cfg: &Esp32TLSServerConfig) -> Self {
        let mut alpn_ptr: Vec<_> = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];
        let tls_cfg_srv = Box::new(esp_tls_cfg_server {
            alpn_protos: alpn_ptr.as_mut_ptr(),
            __bindgen_anon_1: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_1 {
                // The CA root is not need when a client is connecting as it's available
                cacert_buf: std::ptr::null(),
            },
            __bindgen_anon_2: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_2 {
                cacert_bytes: 0,
            },
            __bindgen_anon_3: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_3 {
                // This is the server certificates in the PEM format
                servercert_buf: cfg.srv_cert.as_ptr(),
            },
            __bindgen_anon_4: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_4 {
                servercert_bytes: cfg.srv_cert.len() as u32,
            },
            __bindgen_anon_5: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_5 {
                serverkey_buf: cfg.srv_key,
            },
            __bindgen_anon_6: crate::esp32::esp_idf_svc::sys::esp_tls_cfg_server__bindgen_ty_6 {
                serverkey_bytes: cfg.srv_key_len,
            },
            serverkey_password: std::ptr::null(),
            serverkey_password_len: 0_u32,
        });

        Self {
            alpn_ptr,
            tls_cfg: Either::Left(tls_cfg_srv),
        }
    }

    /// open the a TLS (SSL) context either in client or in server mode
    pub fn open_ssl_context(
        &mut self,
        socket: Option<Async<TcpStream>>,
    ) -> Result<Esp32TLSStream, std::io::Error> {
        Esp32TLSStream::new(socket, &mut self.tls_cfg)
    }
}

/// Esp32TlsStream represents a properly established TLS connection to a server or a client. It can be use bye Esp32TCPStream since it
/// implements std::io::{Read,Write}
impl Esp32TLSStream {
    /// based on a role and a configuration, attempt the setup an SSL context
    fn new(
        socket: Option<Async<TcpStream>>,
        tls_cfg: &mut Either<Box<esp_tls_cfg_server>, Box<esp_tls_cfg>>,
    ) -> Result<Self, std::io::Error> {
        let tls_context = Esp32TLSContext::new()?;
        match tls_cfg {
            Either::Left(tls_cfg) => {
                let fd = socket.as_ref().unwrap().as_raw_fd();
                unsafe {
                    if let Some(err) = EspError::from(esp_tls_server_session_create(
                        &mut **tls_cfg,
                        fd,
                        *tls_context,
                    )) {
                        Err(std::io::Error::new(std::io::ErrorKind::Other, err))
                    } else {
                        Ok(Self {
                            tls_context,
                            socket: socket.unwrap(),
                        })
                    }
                }
            }
            Either::Right(tls_cfg) => {
                match unsafe {
                    esp_tls_conn_new_sync(
                        APP_VIAM_HOSTNAME.as_ptr() as *const i8,
                        APP_VIAM_HOSTNAME.len() as i32,
                        443, // HTTPS port
                        &**tls_cfg,
                        *tls_context,
                    )
                } {
                    -1 => Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "app.viam.com",
                    )),
                    1 => {
                        let socket: Async<TcpStream> = unsafe {
                            let mut fd: i32 = 0;
                            esp_idf_svc::sys::esp!(esp_tls_get_conn_sockfd(*tls_context, &mut fd))
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                            // set socket keep alive properties as such
                            // KEEPIDLE is set to 120 second (time before a first keepalive probe is sent)
                            // KEEPINTVL, KEEPCNT are set to 60 and 4 respectively
                            // Total time before an IDLE and DEAD connection is closed =  360s
                            let enabled: i32 = 1;
                            if lwip_setsockopt(
                                fd,
                                SOL_SOCKET as i32,
                                SO_KEEPALIVE as i32,
                                &enabled as *const i32 as *const _,
                                std::mem::size_of::<i32>() as socklen_t,
                            ) < 0
                            {
                                return Err(std::io::Error::last_os_error());
                            }
                            let var: i32 = TCP_KEEPINTVL_S;
                            if lwip_setsockopt(
                                fd,
                                IPPROTO_TCP as i32,
                                TCP_KEEPINTVL as i32,
                                &var as *const i32 as *const _,
                                std::mem::size_of::<i32>() as socklen_t,
                            ) < 0
                            {
                                return Err(std::io::Error::last_os_error());
                            }
                            let var: i32 = TCP_KEEPCNT_N;
                            if lwip_setsockopt(
                                fd,
                                IPPROTO_TCP as i32,
                                TCP_KEEPCNT as i32,
                                &var as *const i32 as *const _,
                                std::mem::size_of::<i32>() as socklen_t,
                            ) < 0
                            {
                                return Err(std::io::Error::last_os_error());
                            }
                            let var: i32 = TCP_KEEPIDLE_S;
                            if lwip_setsockopt(
                                fd,
                                IPPROTO_TCP as i32,
                                TCP_KEEPIDLE as i32,
                                &var as *const i32 as *const _,
                                std::mem::size_of::<i32>() as socklen_t,
                            ) < 0
                            {
                                return Err(std::io::Error::last_os_error());
                            }

                            TcpStream::from_raw_fd(fd).try_into().unwrap()
                        };
                        Ok(Self {
                            tls_context,
                            socket,
                        })
                    }
                    0 => Err(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "app.viam.com",
                    )),
                    _ => Err(std::io::Error::new(std::io::ErrorKind::Other, "unexpected")),
                }
            }
        }
    }
    fn inner_read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_fn = match unsafe { self.tls_context.read_unaligned().read } {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "tls no read function available",
                ))
            }
        };
        match unsafe { read_fn(*self.tls_context, buf.as_mut_ptr() as *mut i8, buf.len()) as i32 } {
            n @ 1_i32..=i32::MAX => Ok(n as usize),
            0 => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "tls read connection closed",
            )),
            n @ i32::MIN..=-1 => match n {
                _e @ (ESP_TLS_ERR_SSL_WANT_READ | ESP_TLS_ERR_SSL_WANT_WRITE) => {
                    Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
                }
                e => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("tls write failed reason '{e}'"),
                )),
            },
        }
    }
    fn inner_write(&self, buf: &[u8]) -> std::io::Result<usize> {
        let write_fn = match unsafe { self.tls_context.read_unaligned().write } {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "tls no write function available",
                ))
            }
        };
        match unsafe { write_fn(*self.tls_context, buf.as_ptr() as *mut i8, buf.len()) as i32 } {
            n @ i32::MIN..=-1 => match n {
                e @ (ESP_TLS_ERR_SSL_WANT_READ | ESP_TLS_ERR_SSL_WANT_WRITE) => {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        format!("would block cause '{e}'"),
                    ))
                }
                e => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("tls write failed reason '{e}'"),
                )),
            },
            n => Ok(n as usize),
        }
    }
}

impl AsyncWrite for Esp32TLSStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        loop {
            match self.inner_write(buf) {
                Ok(s) => return Poll::Ready(Ok(s)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
            }
            let _ = ready!(self.socket.poll_writable(cx));
        }
    }
    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.socket).poll_close(cx)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.socket).poll_flush(cx)
    }
}

impl AsyncRead for Esp32TLSStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        loop {
            match self.inner_read(buf) {
                Ok(s) => return Poll::Ready(Ok(s)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
            }
            let _ = ready!(self.socket.poll_readable(cx));
        }
    }
}

impl Read for Esp32TLSStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner_read(buf)
    }
}

impl Write for Esp32TLSStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner_write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.as_ref().flush()
    }
}
