use super::errors::ServerError;
use crate::common::{
    grpc::GrpcServer,
    system::shutdown_requested_nonblocking,
    webrtc::{
        api::{AtomicSync, WebRtcError},
        certificate::Certificate,
        dtls::DtlsBuilder,
        grpc::{WebRtcGrpcBody, WebRtcGrpcServer},
        io::WebRtcTransport,
        sctp::SctpHandle,
    },
};

use async_io::Timer;

use futures_lite::prelude::*;

use async_executor::Task;
use std::{rc::Rc, time::Duration};

pub struct WebRtcConfiguration {
    pub(crate) dtls: Box<dyn DtlsBuilder>,
    pub(crate) cert: Rc<Box<dyn Certificate>>,
}

impl WebRtcConfiguration {
    pub fn new(cert: Rc<Box<dyn Certificate>>, dtls: Box<dyn DtlsBuilder>) -> Self {
        Self { cert, dtls }
    }
}

pub(crate) struct WebRTCConnection {
    server: WebRtcGrpcServer<GrpcServer<WebRtcGrpcBody>>,
    _transport: WebRtcTransport,
    ice_agent: AtomicSync,
    sctp_handle: SctpHandle,
}

impl Drop for WebRTCConnection {
    fn drop(&mut self) {
        let _ = self.sctp_handle.close();
        self.ice_agent.done();
    }
}

impl WebRTCConnection {
    pub(crate) fn new(
        server: WebRtcGrpcServer<GrpcServer<WebRtcGrpcBody>>,
        transport: WebRtcTransport,
        ice_agent: AtomicSync,
        sctp_handle: SctpHandle,
    ) -> Self {
        Self {
            server,
            _transport: transport,
            ice_agent,
            sctp_handle,
        }
    }
    pub(crate) async fn run(&mut self) -> Result<(), ServerError> {
        loop {
            if shutdown_requested_nonblocking().await {
                log::info!("recieved shutdown signal, exiting WebRTCConnection");
                return Ok(());
            }
            let req = self
                .server
                .next_request()
                .or(async {
                    Timer::after(Duration::from_secs(30)).await;
                    Err(WebRtcError::OperationTimeout)
                })
                .await;

            if let Err(e) = req {
                return Err(ServerError::Other(Box::new(e)));
            }
        }
    }
}

#[derive(Default)]
struct IncomingConnectionTask {
    task: Option<Task<Result<(), ServerError>>>,
    prio: Option<u32>,
}

impl IncomingConnectionTask {
    fn replace(&mut self, task: Task<Result<(), ServerError>>, prio: u32) {
        let _ = self.task.replace(task);
        let _ = self.prio.replace(prio);
    }
    fn is_finished(&self) -> bool {
        if let Some(task) = self.task.as_ref() {
            return task.is_finished();
        }
        true
    }
    async fn cancel(&mut self) -> Option<ServerError> {
        if let Some(task) = self.task.take() {
            return task.cancel().await?.err();
        }
        None
    }
    fn get_prio(&self) -> u32 {
        if !self.is_finished() {
            return *self.prio.as_ref().unwrap_or(&0);
        }
        0
    }
}

pub(crate) struct IncomingConnectionManager {
    connections: Vec<IncomingConnectionTask>,
}

impl IncomingConnectionManager {
    pub(crate) fn new(size: usize) -> Self {
        let mut connections = Vec::with_capacity(size);
        connections.resize_with(size, Default::default);
        Self { connections }
    }

    #[allow(dead_code)]
    pub(crate) fn max_connections(&self) -> usize {
        self.connections.len()
    }

    // return the lowest priority of active webrtc tasks or 0
    pub(crate) fn get_lowest_prio(&self) -> u32 {
        self.connections
            .iter()
            .min_by(|a, b| a.get_prio().cmp(&b.get_prio()))
            .map_or(0, |c| c.get_prio())
    }
    // function will never fail and the lowest priority will always be replaced
    pub(crate) async fn insert_new_conn(&mut self, task: Task<Result<(), ServerError>>, prio: u32) {
        if let Some(slot) = self
            .connections
            .iter_mut()
            .min_by(|a, b| a.get_prio().cmp(&b.get_prio()))
        {
            if let Some(last_error) = slot.cancel().await {
                log::info!("last_error {:?}", last_error);
            }
            slot.replace(task, prio);
        }
    }
}
