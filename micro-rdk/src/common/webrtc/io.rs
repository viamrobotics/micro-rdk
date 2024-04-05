#![allow(dead_code)]

use async_io::Async;

use std::{net::UdpSocket, sync::Arc};

use super::udp_mux::{UdpMux, UdpMuxer};
#[derive(Clone)]
pub struct WebRtcTransport {
    mux: UdpMuxer,
}

impl WebRtcTransport {
    pub fn new(socket: Arc<Async<UdpSocket>>) -> Self {
        Self {
            mux: UdpMuxer::new(socket),
        }
    }

    pub fn get_stun_channel(&self) -> Option<UdpMux> {
        self.mux.get_stun_mux()
    }
    pub fn get_dtls_channel(&self) -> Option<UdpMux> {
        self.mux.get_dtls_mux()
    }
}
