#![allow(dead_code)]
use std::{
    fmt::Debug,
    io::{self, Cursor},
    net::{Ipv4Addr, UdpSocket},
    pin::Pin,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc, Mutex},
    task::Poll,
    time::Duration,
};

use crate::{
    common::{
        conn::{errors::ServerError, server::WebRTCConnection},
        grpc::GrpcServer,
        grpc_client::{GrpcClientError, GrpcMessageSender, GrpcMessageStream},
        robot::LocalRobot,
    },
    google::rpc::{Code, Status},
    proto::rpc::webrtc::v1::{
        answer_request, answer_response, AnswerRequest, AnswerResponse, AnswerResponseDoneStage,
        AnswerResponseErrorStage, AnswerResponseInitStage, AnswerResponseUpdateStage, IceCandidate,
    },
};

use async_io::Timer;
use atomic_waker::AtomicWaker;
use base64::{engine::general_purpose, Engine};
use futures_lite::{Future, FutureExt, StreamExt};
use prost::{DecodeError, EncodeError};
use sdp::{
    description::{
        common::{Address, ConnectionInformation},
        media::{MediaName, RangedPort},
    },
    MediaDescription, SessionDescription,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    candidates::Candidate,
    certificate::Certificate,
    dtls::DtlsConnector,
    exec::WebRtcExecutor,
    grpc::{WebRtcGrpcBody, WebRtcGrpcServer},
    ice::{ICEAgent, ICECredentials},
    io::WebRtcTransport,
    sctp::{Channel, SctpConnector, SctpHandle},
};

#[derive(Error, Debug)]
pub enum WebRtcError {
    #[error("signaling server disconnected")]
    SignalingDisconnected(),
    #[error("invalid SDP offer")]
    InvalidSDPOffer(String),
    #[error("invalid signaling request")]
    InvalidSignalingRequest,
    #[error("can't marshal answer")]
    AnswerMarshalError(#[from] serde_json::Error),
    #[error("signaling error")]
    SignalingError(String),
    #[error("data channel error")]
    DataChannelOpenError(),
    #[error("webrtc io error")]
    IoError(#[from] io::Error),
    #[error("webrtc grpc message decode error")]
    GrpcDecodeError(#[from] DecodeError),
    #[error("webrtc grpc message encode error")]
    GprcEncodeError(#[from] EncodeError),
    #[error(transparent)]
    DtlsError(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("no connection slots available")]
    NoConnectionAvailable(),
    #[error("cannot parse candidate")]
    CannotParseCandidate,
    #[error("Operation timeout")]
    OperationTiemout,
    #[error(transparent)]
    GrpcClientError(#[from] GrpcClientError),
}

pub(crate) struct WebRtcSignalingChannel {
    signaling_tx: GrpcMessageSender<AnswerResponse>,
    signaling_rx: GrpcMessageStream<AnswerRequest>,
    engine: Box<general_purpose::GeneralPurpose>,
    _guard: async_lock::SemaphoreGuardArc,
}

impl WebRtcSignalingChannel {
    pub(crate) fn new(
        signaling_tx: GrpcMessageSender<AnswerResponse>,
        signaling_rx: GrpcMessageStream<AnswerRequest>,
        guard: async_lock::SemaphoreGuardArc,
    ) -> Self {
        Self {
            signaling_tx,
            signaling_rx,
            engine: general_purpose::STANDARD.into(),
            _guard: guard,
        }
    }
}

impl Drop for WebRtcSignalingChannel {
    fn drop(&mut self) {
        log::debug!("dropping signaling");
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SdpOffer {
    #[serde(rename = "type")]
    pub sdp_type: String,
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub struct WebRtcSdp {
    sdp: SessionDescription,
    uuid: String,
}

impl WebRtcSdp {
    pub fn new(sdp: SessionDescription, uuid: String) -> Self {
        WebRtcSdp { sdp, uuid }
    }
}

struct AtomicSyncInner {
    waker: AtomicWaker,
    done: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct AtomicSync(Arc<AtomicSyncInner>);

impl Default for AtomicSync {
    fn default() -> Self {
        Self(Arc::new(AtomicSyncInner {
            waker: AtomicWaker::new(),
            done: AtomicBool::new(false),
        }))
    }
}

impl AtomicSync {
    pub(crate) fn done(&self) {
        self.0
            .done
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.0.waker.wake();
    }
    pub(crate) fn get(&self) -> bool {
        self.0.done.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub(crate) fn reset(&self) {
        self.0
            .done
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Future for AtomicSync {
    type Output = ();
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.0.done.load(std::sync::atomic::Ordering::Relaxed) {
            return Poll::Ready(());
        }
        self.0.waker.register(cx.waker());
        if self.0.done.load(std::sync::atomic::Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl WebRtcSignalingChannel {
    /// The function waits for an Offer to be made, once received a user should poll for candidate using next_remote_candidate
    /// the function will ignore Stage::Update
    pub(crate) async fn wait_sdp_offer(&mut self) -> Result<Box<WebRtcSdp>, WebRtcError> {
        loop {
            // Once the headers have been sent by the sever we expect the first messages to show up on the channel rather quickly
            // if not then we should consider signaling to be disconnected
            let res = self
                .signaling_rx
                .next()
                .or(async {
                    let _ = Timer::after(Duration::from_secs(30)).await;
                    None
                })
                .await;
            match res {
                None => {
                    return Err(WebRtcError::SignalingDisconnected());
                }
                Some(req) => {
                    let req = req?;
                    if let Some(stage) = req.stage.clone() {
                        match stage {
                            answer_request::Stage::Init(s) => {
                                let sdp_decoded = self
                                    .engine
                                    .decode(s.sdp)
                                    .map_err(|e| WebRtcError::InvalidSDPOffer(e.to_string()))?;
                                let sdp_decoded: SdpOffer =
                                    serde_json::from_slice(sdp_decoded.as_slice())
                                        .map_err(|e| WebRtcError::InvalidSDPOffer(e.to_string()))?;

                                if sdp_decoded.sdp_type != "offer" {
                                    return Err(WebRtcError::InvalidSDPOffer(format!(
                                        "unexpected type {}",
                                        sdp_decoded.sdp_type
                                    )));
                                }

                                log::debug!("received an SDP offer {:?}", sdp_decoded);

                                let mut cursor = Cursor::new(sdp_decoded.sdp);
                                let sdp = sdp::SessionDescription::unmarshal(&mut cursor)
                                    .map_err(|e| WebRtcError::InvalidSDPOffer(e.to_string()))?;
                                return Ok(Box::new(WebRtcSdp::new(sdp, req.uuid)));
                            }
                            answer_request::Stage::Error(s) => {
                                if let Some(status) = s.status {
                                    return Err(WebRtcError::SignalingError(status.message));
                                }
                                return Err(WebRtcError::SignalingError("unknown".to_owned()));
                            }
                            _ => {
                                continue;
                            }
                        }
                    } else {
                        return Err(WebRtcError::InvalidSignalingRequest);
                    }
                }
            };
        }
    }

    pub(crate) async fn send_sdp_error_too_many_connections(
        &mut self,
        sdp: &WebRtcSdp,
    ) -> Result<(), WebRtcError> {
        let answer = AnswerResponse {
            uuid: sdp.uuid.clone(),
            stage: Some(answer_response::Stage::Error(AnswerResponseErrorStage {
                status: Some(Status {
                    code: Code::ResourceExhausted.into(),
                    message: "too many active connections".to_string(),
                    ..Default::default()
                }),
            })),
        };

        if let Err(e) = self.signaling_tx.send_message(answer).await {
            log::error!("error sending signaling message: {:?}", e);
            Err(WebRtcError::SignalingDisconnected())
        } else {
            log::warn!("too many active connections");
            Ok(())
        }
    }

    pub(crate) async fn send_sdp_answer(&mut self, sdp: &WebRtcSdp) -> Result<(), WebRtcError> {
        let answer = SdpOffer {
            sdp_type: "answer".to_owned(),
            sdp: sdp.sdp.marshal(),
        };
        let answer = self
            .engine
            .encode(serde_json::to_string(&answer).map_err(WebRtcError::AnswerMarshalError)?);

        let answer = AnswerResponse {
            uuid: sdp.uuid.clone(),
            stage: Some(answer_response::Stage::Init(AnswerResponseInitStage {
                sdp: answer,
            })),
        };
        match self.signaling_tx.send_message(answer).await {
            Err(e) => {
                log::error!("error sending signaling message: {:?}", e);
                Err(WebRtcError::SignalingDisconnected())
            }
            Ok(_) => Ok(()),
        }
    }

    pub(crate) async fn send_local_candidate(
        &mut self,
        candidate: &Candidate,
        ufrag: String,
        uuid: String,
    ) -> Result<(), WebRtcError> {
        let answer = AnswerResponse {
            uuid,
            stage: Some(answer_response::Stage::Update(AnswerResponseUpdateStage {
                candidate: Some(IceCandidate {
                    candidate: candidate.to_string(),
                    sdp_mid: Some("".to_owned()),
                    sdpm_line_index: Some(0),
                    username_fragment: Some(ufrag),
                }),
            })),
        };
        match self.signaling_tx.send_message(answer).await {
            Err(_) => Err(WebRtcError::SignalingDisconnected()),
            Ok(_) => Ok(()),
        }
    }
    pub(crate) async fn next_remote_candidate(&mut self) -> Result<Option<Candidate>, WebRtcError> {
        match self.signaling_rx.next().await {
            None => Err(WebRtcError::SignalingDisconnected()),
            Some(req) => {
                let req = req?;
                if let Some(stage) = req.stage {
                    match stage {
                        answer_request::Stage::Update(c) => {
                            if let Some(c) = c.candidate {
                                log::debug!("received candidate {}", c.candidate);
                                return c
                                    .candidate
                                    .try_into()
                                    .map_err(|_| WebRtcError::CannotParseCandidate)
                                    .map(Option::Some);
                            } else {
                                log::error!("received no candidates with this update request");
                                return Ok(None);
                            }
                        }
                        answer_request::Stage::Error(s) => {
                            if let Some(status) = s.status {
                                return Err(WebRtcError::SignalingError(status.message));
                            }
                            return Err(WebRtcError::SignalingError("unknown".to_owned()));
                        }
                        answer_request::Stage::Done(_) => {
                            return Ok(None);
                        }
                        _ => {
                            return Err(WebRtcError::SignalingError("unexpected stage".to_owned()))
                        }
                    }
                }
                Ok(None)
            }
        }
    }
    pub async fn send_done(&mut self, uuid: String) -> Result<(), WebRtcError> {
        let answer = AnswerResponse {
            uuid,
            stage: Some(answer_response::Stage::Done(AnswerResponseDoneStage {})),
        };
        match self.signaling_tx.send_message(answer).await {
            Err(_) => Err(WebRtcError::SignalingDisconnected()),
            Ok(_) => Ok(()),
        }
    }
}

pub struct WebRtcApi<S, E> {
    executor: E,
    signaling: Box<WebRtcSignalingChannel>,
    transport: WebRtcTransport,
    certificate: Rc<S>,
    local_creds: ICECredentials,
    remote_creds: Option<ICECredentials>,
    local_ip: Ipv4Addr,
    dtls: Option<Box<dyn DtlsConnector>>,
    ice_agent: AtomicSync,
    sdp: Box<WebRtcSdp>,
}

impl<'a, C, E> WebRtcApi<C, E>
where
    C: Certificate,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()>>>> + Clone + 'a,
{
    pub(crate) fn new(
        executor: E,
        signaling: Box<WebRtcSignalingChannel>,
        certificate: Rc<C>,
        local_ip: Ipv4Addr,
        dtls: Box<dyn DtlsConnector>,
        sdp: Box<WebRtcSdp>,
    ) -> Self {
        let udp = Arc::new(async_io::Async::<UdpSocket>::bind(([0, 0, 0, 0], 0)).unwrap());

        let transport = WebRtcTransport::new(udp);

        Self {
            executor,
            signaling,
            sdp,
            transport,
            certificate,
            remote_creds: None,
            local_creds: Default::default(),
            local_ip,
            dtls: Some(dtls),
            ice_agent: AtomicSync::default(),
        }
    }

    async fn run_ice_until_connected(&mut self, answer: &WebRtcSdp) -> Result<(), WebRtcError> {
        let (tx, rx) = async_channel::bounded(1);

        // TODO(NPM) consider returning an error? We should not take the channel more than once....
        let ice_transport = self.transport.get_stun_channel().unwrap();
        let mut ice_agent = ICEAgent::new(
            rx,
            ice_transport,
            self.local_creds.clone(),
            self.remote_creds.as_ref().unwrap().clone(),
            self.local_ip,
        );

        self.signaling.send_sdp_answer(answer).await?;

        log::info!("gathering local candidates");
        ice_agent.local_candidates().await.unwrap();

        for c in &ice_agent.local_candidates {
            log::debug!("sending local candidates {:?}", c);
            self.signaling
                .send_local_candidate(c, self.local_creds.u_frag.clone(), self.sdp.uuid.clone())
                .await?;
        }
        self.signaling.send_done(self.sdp.uuid.clone()).await?;
        let sync = AtomicSync::default();
        let sync_clone = sync.clone();
        let die_clone = self.ice_agent.clone();
        self.executor.execute(Box::pin(async move {
            ice_agent.run(sync, die_clone).await;
        }));

        while !sync_clone.get() {
            let candidate = self
                .signaling
                .next_remote_candidate()
                .or(async {
                    Timer::after(Duration::from_millis(50)).await;
                    Err(WebRtcError::CannotParseCandidate)
                })
                .await;
            match candidate {
                Ok(candidate) => {
                    if let Some(c) = candidate {
                        tx.send(c)
                            .await
                            .map_err(|e| WebRtcError::SignalingError(e.to_string()))?;
                    } else {
                        break;
                    }
                }
                Err(WebRtcError::CannotParseCandidate) => continue,
                Err(e) => {
                    return Err(e);
                }
            }
        }

        sync_clone.await;
        Ok(())
    }

    async fn open_data_channel(&mut self) -> Result<(Channel, SctpHandle), WebRtcError> {
        let mut dtls = self.dtls.take().unwrap();

        // TODO(NPM) consider returning an error? We should not take the channel more than once....
        let dtls_transport = self.transport.get_dtls_channel().unwrap();

        dtls.set_transport(dtls_transport);

        if let Ok(dtls_stream) = dtls
            .accept()
            .map_err(|e| WebRtcError::DtlsError(Box::new(e)))?
            .await
        {
            let (c_tx, c_rx) = async_channel::unbounded();

            let sctp = Box::new(SctpConnector::new(dtls_stream, c_tx));
            let mut sctp = sctp
                .listen()
                .await
                .map_err(|e| WebRtcError::DtlsError(Box::new(e)))?;
            let hnd = sctp.get_handle();
            self.executor.execute(Box::pin(async move {
                sctp.run().await;
            }));
            let channel = c_rx
                .recv()
                .await
                .map_err(|_| WebRtcError::DataChannelOpenError())?;
            return Ok((channel, hnd));
        }

        Err(WebRtcError::DataChannelOpenError())
    }

    pub(crate) async fn connect(
        mut self,
        answer: Box<WebRtcSdp>,
        robot: Arc<Mutex<LocalRobot>>,
    ) -> Result<WebRTCConnection, ServerError> {
        self.run_ice_until_connected(&answer)
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
        let srv = WebRtcGrpcServer::new(c.0, GrpcServer::new(robot, WebRtcGrpcBody::default()));
        Ok(WebRTCConnection::new(
            srv,
            self.transport,
            self.ice_agent,
            c.1,
        ))
    }

    pub async fn answer(
        &mut self,
        current_prio: u32,
    ) -> Result<(Box<WebRtcSdp>, u32), WebRtcError> {
        let attribute = self
            .sdp
            .sdp
            .media_descriptions
            .first()
            .ok_or_else(|| WebRtcError::InvalidSDPOffer("no media description".to_owned()))?;

        let caller_prio = attribute
            .attribute("x-priority")
            .flatten()
            .map_or(Ok(u32::MAX), |a| a.parse::<u32>())
            .unwrap_or(u32::MAX);

        // TODO use is_some_then when rust min version reach 1.70
        if current_prio >= caller_prio {
            self.signaling
                .send_sdp_error_too_many_connections(&self.sdp)
                .await?;

            // TODO(APP-6381): Without this delay, sdks receive a `ContextCancelled` error instead
            // of `ResourceExhausted`. It's possible a race condition on the App side is closing
            // the connection before the error is properly recorded for an sdk to see.
            async_io::Timer::after(Duration::from_millis(200)).await;

            return Err(WebRtcError::NoConnectionAvailable());
        }

        let answer = SessionDescription::new_jsep_session_description(false);

        let remote_creds = ICECredentials::new(
            attribute
                .attribute("ice-ufrag")
                .flatten()
                .ok_or_else(|| WebRtcError::InvalidSDPOffer("ice-ufrag absent".to_string()))?
                .to_owned(),
            attribute
                .attribute("ice-pwd")
                .flatten()
                .ok_or_else(|| WebRtcError::InvalidSDPOffer("ice-pwd absent".to_string()))?
                .to_owned(),
        );

        let _ = self.remote_creds.insert(remote_creds);

        // rfc8839 section 4.3.2
        let data_track_name = MediaName {
            media: "application".to_owned(),
            port: RangedPort {
                value: 9,
                range: None,
            },
            protos: vec!["UDP".to_owned(), "DTLS".to_owned(), "SCTP".to_owned()],
            formats: vec!["webrtc-datachannel".to_owned()],
        };

        let fp = self.certificate.get_fingerprint();

        let media = MediaDescription {
            media_name: data_track_name,
            media_title: None,
            // rfc8839 section 4.3.2
            connection_information: Some(ConnectionInformation {
                network_type: "IN".to_owned(),
                address_type: "IP4".to_owned(),
                address: Some(Address {
                    address: "0.0.0.0".to_owned(),
                    ttl: None,
                    range: None,
                }),
            }),
            bandwidth: vec![],
            encryption_key: None,
            attributes: vec![],
        }
        .with_value_attribute("setup".to_owned(), "passive".to_owned())
        .with_value_attribute("mid".to_string(), "0".to_owned())
        .with_property_attribute("sendrecv".to_owned())
        .with_property_attribute("sctp-port:5000".to_owned())
        .with_ice_credentials(
            self.local_creds.u_frag.clone(),
            self.local_creds.pwd.clone(),
        )
        .with_fingerprint(fp.get_algo().to_string(), fp.get_hash().to_string());

        let answer = answer.with_value_attribute("group".to_owned(), "BUNDLE 0".to_owned());

        let answer = answer.with_media(media);

        Ok((
            Box::new(WebRtcSdp::new(answer, self.sdp.uuid.clone())),
            caller_prio,
        ))
    }
}
