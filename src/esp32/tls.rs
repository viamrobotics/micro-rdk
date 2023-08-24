use either::Either;
use esp_idf_sys::{
    esp_tls_cfg, esp_tls_cfg_server, esp_tls_conn_destroy, esp_tls_conn_new_sync,
    esp_tls_conn_state_ESP_TLS_CONNECTING as ESP_TLS_CONNECTING,
    esp_tls_conn_state_ESP_TLS_DONE as ESP_TLS_DONE,
    esp_tls_conn_state_ESP_TLS_FAIL as ESP_TLS_FAIL,
    esp_tls_conn_state_ESP_TLS_HANDSHAKE as ESP_TLS_HANDSHAKE,
    esp_tls_conn_state_ESP_TLS_INIT as ESP_TLS_INIT, esp_tls_init, esp_tls_server_session_create,
    esp_tls_t, EspError, ESP_TLS_ERR_SSL_WANT_READ, ESP_TLS_ERR_SSL_WANT_WRITE,
};
use std::{
    fmt::Debug,
    io::{Read, Write},
    mem::ManuallyDrop,
    net::TcpStream,
    os::{raw::c_char, unix::prelude::AsRawFd},
};

use crate::common::conn::server::{ServerError, TlsClientConnector};

use super::tcp::Esp32Stream;

unsafe impl Sync for Esp32Tls {}
unsafe impl Send for Esp32Tls {}

/// structure to store tls configuration
#[derive(Clone)]
pub struct Esp32Tls {
    #[allow(dead_code)]
    alpn_ptr: Vec<*const c_char>,
    tls_cfg: Either<Box<esp_tls_cfg_server>, Box<esp_tls_cfg>>,
}

impl TlsClientConnector for Esp32Tls {
    type Stream = Esp32Stream;
    fn connect(&mut self) -> Result<Self::Stream, ServerError> {
        Ok(Esp32Stream::TLSStream(Box::new(
            self.open_ssl_context(None)
                .map_err(|e| ServerError::Other(e.into()))?,
        )))
    }
}

/// TCP like stream for encrypted communication over TLS
pub struct Esp32TlsStream {
    tls_context: ManuallyDrop<*mut esp_tls_t>,
    socket: Option<TcpStream>, // may store the raw socket
}

pub struct Esp32TlsServerConfig {
    srv_cert: [Vec<u8>; 2],
    srv_key: *const u8,
    srv_key_len: u32,
}

impl Esp32TlsServerConfig {
    pub fn new(srv_cert: [Vec<u8>; 2], srv_key: *const u8, srv_key_len: u32) -> Self {
        Esp32TlsServerConfig {
            srv_cert,
            srv_key,
            srv_key_len,
        }
    }
}

impl Debug for Esp32TlsStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Esp32TlsStream")
            .field(
                "tls",
                match unsafe { (*(*self.tls_context)).conn_state } {
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

impl Esp32Tls {
    pub fn new_client() -> Self {
        let mut alpn_ptr: Vec<_> = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];
        // this is a root certificate to validate the server's certificate
        let cert = include_bytes!("../../certs/google_gts_root_r1.crt");

        let tls_cfg_client = Box::new(esp_tls_cfg {
            alpn_protos: alpn_ptr.as_mut_ptr(),
            __bindgen_anon_1: esp_idf_sys::esp_tls_cfg__bindgen_ty_1 {
                cacert_buf: cert.as_ptr(),
            },
            __bindgen_anon_2: esp_idf_sys::esp_tls_cfg__bindgen_ty_2 {
                cacert_bytes: cert.len() as u32,
            },
            __bindgen_anon_3: esp_idf_sys::esp_tls_cfg__bindgen_ty_3 {
                clientcert_buf: std::ptr::null(),
            },
            __bindgen_anon_4: esp_idf_sys::esp_tls_cfg__bindgen_ty_4 {
                clientcert_bytes: 0_u32,
            },
            __bindgen_anon_5: esp_idf_sys::esp_tls_cfg__bindgen_ty_5 {
                clientkey_buf: std::ptr::null(),
            },
            __bindgen_anon_6: esp_idf_sys::esp_tls_cfg__bindgen_ty_6 {
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
    pub fn new_server(cfg: &Esp32TlsServerConfig) -> Self {
        let mut alpn_ptr: Vec<_> = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];
        let tls_cfg_srv = Box::new(esp_tls_cfg_server {
            alpn_protos: alpn_ptr.as_mut_ptr(),
            __bindgen_anon_1: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_1 {
                // This is the root LE certificate in the DER format
                cacert_buf: cfg.srv_cert[1].as_ptr(),
            },
            __bindgen_anon_2: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_2 {
                cacert_bytes: cfg.srv_cert[1].len() as u32,
            },
            __bindgen_anon_3: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_3 {
                // This is the server certificates in the PEM format
                servercert_buf: cfg.srv_cert[0].as_ptr(),
            },
            __bindgen_anon_4: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_4 {
                servercert_bytes: cfg.srv_cert[0].len() as u32,
            },
            __bindgen_anon_5: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_5 {
                serverkey_buf: cfg.srv_key,
            },
            __bindgen_anon_6: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_6 {
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
        socket: Option<TcpStream>,
    ) -> anyhow::Result<Esp32TlsStream> {
        Esp32TlsStream::new(socket, &mut self.tls_cfg)
    }
}

/// Esp32TlsStream represents a properly established TLS connection to a server or a client. It can be use bye Esp32TCPStream since it
/// implements std::io::{Read,Write}
impl Esp32TlsStream {
    /// based on a role and a configuration, attempt the setup an SSL context
    fn new(
        socket: Option<TcpStream>,
        tls_cfg: &mut Either<Box<esp_tls_cfg_server>, Box<esp_tls_cfg>>,
    ) -> anyhow::Result<Self> {
        let p = unsafe { esp_tls_init() };
        if p.is_null() {
            return Err(anyhow::anyhow!("failed to allocate TLS struct"));
        }
        let tls_context = ManuallyDrop::new(p);
        match tls_cfg {
            Either::Left(tls_cfg) => {
                let fd = socket.as_ref().unwrap().as_raw_fd();
                unsafe {
                    if let Some(err) = EspError::from(esp_tls_server_session_create(
                        &mut **tls_cfg,
                        fd,
                        *tls_context,
                    )) {
                        log::error!("can't create TLS context ''{}''", err);
                        esp_tls_conn_destroy(*tls_context);
                        Err(anyhow::anyhow!(err))
                    } else {
                        Ok(Self {
                            tls_context,
                            socket,
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
                    -1 => Err(anyhow::anyhow!(
                        "Failed to established connection to app.viam.com"
                    )),
                    1 => {
                        log::info!("Connected to app.viam.com");
                        Ok(Self {
                            tls_context,
                            socket,
                        })
                    }
                    0 => Err(anyhow::anyhow!("connection to app.viam.com in progress")),
                    n => Err(anyhow::anyhow!("Unexpected error '{}'", n)),
                }
            }
        }
    }
}

impl Drop for Esp32TlsStream {
    fn drop(&mut self) {
        log::error!("dropping the tls stream");
        if let Some(err) = EspError::from(unsafe { esp_tls_conn_destroy(*self.tls_context) }) {
            log::error!("error while dropping the tls connection '{}'", err);
        }
    }
}

impl Read for Esp32TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
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
}

impl Write for Esp32TlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
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

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.socket {
            Some(s) => s.flush(),
            None => Ok(()),
        }
    }
}
