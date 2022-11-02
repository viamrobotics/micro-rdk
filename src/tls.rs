use std::{
    fmt::Debug,
    io::{Read, Write},
    mem::ManuallyDrop,
    net::TcpStream,
    os::{raw::c_char, unix::prelude::AsRawFd},
};

use esp_idf_sys::{
    esp_tls_cfg_server, esp_tls_conn_destroy,
    esp_tls_conn_state_ESP_TLS_CONNECTING as ESP_TLS_CONNECTING,
    esp_tls_conn_state_ESP_TLS_DONE as ESP_TLS_DONE,
    esp_tls_conn_state_ESP_TLS_FAIL as ESP_TLS_FAIL,
    esp_tls_conn_state_ESP_TLS_HANDSHAKE as ESP_TLS_HANDSHAKE,
    esp_tls_conn_state_ESP_TLS_INIT as ESP_TLS_INIT, esp_tls_server_session_create, esp_tls_t,
    EspError, ESP_TLS_ERR_SSL_WANT_READ, ESP_TLS_ERR_SSL_WANT_WRITE,
};

pub struct Esp32tls {
    #[allow(dead_code)]
    alpn_ptr: Vec<*const c_char>,
    tls_cfg: esp_tls_cfg_server,
}

pub struct ESP32TLSStream {
    tls_context: ManuallyDrop<Box<esp_tls_t>>,
    socket: TcpStream, // may store the raw socket
}

impl Debug for ESP32TLSStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ESP32TLSStream")
            .field(
                "tls",
                match self.tls_context.conn_state {
                    ESP_TLS_INIT => &"Tls initializing",
                    ESP_TLS_CONNECTING => &"Tls connecting",
                    ESP_TLS_HANDSHAKE => &"Tls handshake",
                    ESP_TLS_FAIL => &"Tls fail",
                    ESP_TLS_DONE => &"Tls closed",
                    _ => unreachable!("nope"),
                },
            )
            .finish()
    }
}

static ALPN_PROTOCOLS: &[u8] = b"h2\0";

impl Esp32tls {
    pub fn new() -> Self {
        let cert = include_bytes!(concat!(env!("OUT_DIR"), "/ca.crt"));
        let key = include_bytes!(concat!(env!("OUT_DIR"), "/key.key"));
        let mut alpn_ptr: Vec<_> = vec![ALPN_PROTOCOLS.as_ptr() as *const i8, std::ptr::null()];

        let tls_cfg = esp_tls_cfg_server {
            alpn_protos: alpn_ptr.as_mut_ptr(),
            __bindgen_anon_1: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_1 {
                cacert_buf: std::ptr::null(),
            },
            __bindgen_anon_2: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_2 {
                cacert_bytes: 0_u32,
            },
            __bindgen_anon_3: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_3 {
                servercert_buf: cert.as_ptr(),
            },
            __bindgen_anon_4: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_4 {
                servercert_bytes: cert.len() as u32,
            },
            __bindgen_anon_5: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_5 {
                serverkey_buf: key.as_ptr(),
            },
            __bindgen_anon_6: esp_idf_sys::esp_tls_cfg_server__bindgen_ty_6 {
                serverkey_bytes: key.len() as u32,
            },
            serverkey_password: std::ptr::null(),
            serverkey_password_len: 0_u32,
        };
        Self { alpn_ptr, tls_cfg }
    }
    pub fn open_ssl_context(&mut self, socket: TcpStream) -> anyhow::Result<ESP32TLSStream> {
        ESP32TLSStream::new(socket, &mut self.tls_cfg)
    }
}
impl ESP32TLSStream {
    fn new(socket: TcpStream, tls_cfg: *mut esp_tls_cfg_server) -> anyhow::Result<Self> {
        let mut tls_context = ManuallyDrop::new(Box::new(esp_tls_t {
            ..Default::default()
        }));
        let fd = socket.as_raw_fd();
        unsafe {
            if let Some(err) = EspError::from(esp_tls_server_session_create(
                tls_cfg,
                fd,
                &mut **tls_context,
            )) {
                log::error!("Can't create TLS context {}", err);
                return Err(anyhow::anyhow!(err));
            }
        };
        Ok(Self {
            tls_context,
            socket,
        })
    }
}
impl Drop for ESP32TLSStream {
    fn drop(&mut self) {
        // This is not the right way to do it, since rust has allocated the context we should be the one the final free
        // However esp_tls_conn_destroy actually free the pointer.
        // Calling esp_tls_internal_event_tracker_destroy might be enough to avoid leaks
        // Also after this call the socket is closed
        if let Some(err) = EspError::from(unsafe { esp_tls_conn_destroy(&mut **self.tls_context) })
        {
            log::error!("error while dropping the tls connection {}", err);
        }
    }
}
impl Read for ESP32TLSStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_fn = match self.tls_context.read {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "tls no read function available",
                ))
            }
        };
        match unsafe {
            read_fn(
                &mut **self.tls_context,
                buf.as_mut_ptr() as *mut i8,
                buf.len() as u32,
            )
        } {
            n if n > 0 => Ok(n as usize),
            0 => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "tls read connection closed",
            )),
            n if n < 0 => match n {
                e @ (ESP_TLS_ERR_SSL_WANT_READ | ESP_TLS_ERR_SSL_WANT_WRITE) => {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        format!("would block cause {}", e),
                    ))
                }
                e => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("tls write failed reason {}", e),
                )),
            },
            _ => {
                unreachable!("panic")
            }
        }
    }
}

impl Write for ESP32TLSStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let write_fn = match self.tls_context.write {
            Some(f) => f,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "tls no write function available",
                ))
            }
        };
        match unsafe {
            write_fn(
                &mut **self.tls_context,
                buf.as_ptr() as *mut i8,
                buf.len() as u32,
            )
        } {
            n if n < 0 => match n {
                e @ (ESP_TLS_ERR_SSL_WANT_READ | ESP_TLS_ERR_SSL_WANT_WRITE) => {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        format!("would block cause {}", e),
                    ))
                }
                e => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("tls write failed reason {}", e),
                )),
            },
            n => Ok(n as usize),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }
}
