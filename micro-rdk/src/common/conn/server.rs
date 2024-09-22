use super::errors::ServerError;
use crate::{
    common::{
        grpc::GrpcServer,
        robot::LocalRobot,
        webrtc::{
            api::{WebRtcApi, WebRtcError, WebRtcSdp},
            certificate::Certificate,
            dtls::DtlsBuilder,
            exec::WebRtcExecutor,
            grpc::{WebRtcGrpcBody, WebRtcGrpcServer},
        },
    },
    proto::{self},
};

use async_io::Timer;

use futures_lite::prelude::*;
use hyper::rt;

use async_executor::Task;
use std::{
    fmt::Debug,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

pub trait TlsClientConnector {
    type Stream: rt::Read + rt::Write + Unpin + 'static;

    fn connect(&mut self) -> impl std::future::Future<Output = Result<Self::Stream, ServerError>>;
}

pub trait Http2Connector: std::fmt::Debug {
    type Stream;
    fn accept(&mut self) -> impl std::future::Future<Output = std::io::Result<Self::Stream>>;
}

#[derive(Debug)]
pub enum IncomingConnection<L, U> {
    Http2Connection(L),
    WebRtcConnection(U),
}

pub struct WebRtcConfiguration2 {
    pub(crate) dtls: Box<dyn DtlsBuilder>,
    pub(crate) cert: Rc<Box<dyn Certificate>>,
}

impl WebRtcConfiguration2 {
    pub fn new(cert: Rc<Box<dyn Certificate>>, dtls: Box<dyn DtlsBuilder>) -> Self {
        Self { cert, dtls }
    }
}

pub(crate) struct WebRTCConnection<C, E> {
    pub(crate) webrtc_api: WebRtcApi<C, E>,
    pub(crate) sdp: Box<WebRtcSdp>,
    pub(crate) server: Option<WebRtcGrpcServer<GrpcServer<WebRtcGrpcBody>>>,
    pub(crate) robot: Arc<Mutex<LocalRobot>>,
    pub(crate) prio: u32,
}

impl<C, E> WebRTCConnection<C, E>
where
    C: Certificate,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()>>>> + Clone,
{
    pub(crate) async fn open_data_channel(&mut self) -> Result<(), ServerError> {
        self.webrtc_api
            .run_ice_until_connected(&self.sdp)
            .or(async {
                Timer::after(Duration::from_secs(10)).await;
                Err(WebRtcError::OperationTiemout)
            })
            .await
            .map_err(|e| match e {
                WebRtcError::OperationTiemout => ServerError::ServerConnectionTimeout,
                _ => ServerError::Other(e.into()),
            })?;

        let c = self
            .webrtc_api
            .open_data_channel()
            .or(async {
                Timer::after(Duration::from_secs(10)).await;
                Err(WebRtcError::OperationTiemout)
            })
            .await
            .map_err(|e| match e {
                WebRtcError::OperationTiemout => ServerError::ServerConnectionTimeout,
                _ => ServerError::Other(e.into()),
            })?;
        let srv = WebRtcGrpcServer::new(
            c,
            GrpcServer::new(self.robot.clone(), WebRtcGrpcBody::default()),
        );
        let _ = self.server.insert(srv);
        Ok(())
    }
    pub(crate) async fn run(&mut self) -> Result<(), ServerError> {
        if self.server.is_none() {
            return Err(ServerError::ServerConnectionNotConfigured);
        }
        let srv = self.server.as_mut().unwrap();
        loop {
            let req = srv
                .next_request()
                .or(async {
                    Timer::after(Duration::from_secs(30)).await;
                    Err(WebRtcError::OperationTiemout)
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
