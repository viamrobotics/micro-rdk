#![allow(dead_code)]
use std::{
    fmt::Debug,
    io::{self, Cursor},
    net::Ipv4Addr,
    pin::Pin,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;

#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;
use crate::{
    common::grpc_client::{GrpcMessageSender, GrpcMessageStream},
    proto::rpc::webrtc::v1::{
        answer_request, answer_response, AnswerRequest, AnswerResponse, AnswerResponseDoneStage,
        AnswerResponseInitStage, AnswerResponseUpdateStage, IceCandidate,
    },
};

use base64::{engine::general_purpose, Engine};
use futures_lite::{Future, StreamExt};
use prost::{DecodeError, EncodeError};
use sdp::{
    description::{
        common::{Address, ConnectionInformation},
        media::{MediaName, RangedPort},
    },
    MediaDescription, SessionDescription,
};
use serde::{Deserialize, Serialize};
use smol::net::UdpSocket;
use smol_timeout::TimeoutExt;
use thiserror::Error;

use super::{
    candidates::Candidate,
    certificate::Certificate,
    dtls::DtlsConnector,
    exec::WebRtcExecutor,
    ice::{ICEAgent, ICECredentials},
    io::WebRtcTransport,
    sctp::{Channel, SctpConnector},
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
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    DtlsError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub(crate) struct WebRtcSignalingChannel {
    signaling_tx: GrpcMessageSender<AnswerResponse>,
    signaling_rx: GrpcMessageStream<AnswerRequest>,
    engine: general_purpose::GeneralPurpose,
}

impl Drop for WebRtcSignalingChannel {
    fn drop(&mut self) {
        log::error!("dropping signaling");
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

impl WebRtcSignalingChannel {
    /// The function waits for an Offer to be made, once received a user should poll for candidate using next_remote_candidate
    /// the function will ignore Stage::Update
    pub(crate) async fn wait_sdp_offer(&mut self) -> Result<WebRtcSdp, WebRtcError> {
        loop {
            match self.signaling_rx.next().await {
                None => {
                    return Err(WebRtcError::SignalingDisconnected());
                }
                Some(req) => {
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
                                return Ok(WebRtcSdp::new(sdp, req.uuid));
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
            }
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
        match self.signaling_tx.send_message(answer) {
            Err(e) => {
                log::error!("error sending signaling message: {:?}", e);
                Err(WebRtcError::SignalingDisconnected())
            }
            Ok(_) => Ok(()),
        }
    }
    pub(crate) fn send_local_candidate(
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
        match self.signaling_tx.send_message(answer) {
            Err(_) => Err(WebRtcError::SignalingDisconnected()),
            Ok(_) => Ok(()),
        }
    }
    pub(crate) async fn next_remote_candidate(&mut self) -> Result<Option<Candidate>, WebRtcError> {
        match self.signaling_rx.next().await {
            None => Err(WebRtcError::SignalingDisconnected()),
            Some(req) => {
                if let Some(stage) = req.stage {
                    match stage {
                        answer_request::Stage::Update(c) => {
                            if let Some(c) = c.candidate {
                                log::debug!("received candidate {}", c.candidate);
                                let c = c.candidate.try_into().ok();
                                return Ok(c);
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
    pub fn send_done(&mut self, uuid: String) -> Result<(), WebRtcError> {
        let answer = AnswerResponse {
            uuid,
            stage: Some(answer_response::Stage::Done(AnswerResponseDoneStage {})),
        };
        match self.signaling_tx.send_message(answer) {
            Err(_) => Err(WebRtcError::SignalingDisconnected()),
            Ok(_) => Ok(()),
        }
    }
}

#[cfg(feature = "native")]
type Executor<'a> = NativeExecutor<'a>;
#[cfg(feature = "esp32")]
type Executor<'a> = Esp32Executor<'a>;

pub struct WebRtcApi<S, D, E> {
    executor: E,
    signaling: Option<WebRtcSignalingChannel>,
    uuid: Option<String>,
    transport: WebRtcTransport,
    certificate: Rc<S>,
    local_creds: ICECredentials,
    remote_creds: Option<ICECredentials>,
    local_ip: Ipv4Addr,
    dtls: Option<D>,
}

impl<'a, C, D, E> WebRtcApi<C, D, E>
where
    C: Certificate,
    D: DtlsConnector,
    E: WebRtcExecutor<Pin<Box<dyn Future<Output = ()>>>> + Clone + 'a,
{
    pub(crate) fn new(
        executor: E,
        tx_half: GrpcMessageSender<AnswerResponse>,
        rx_half: GrpcMessageStream<AnswerRequest>,
        certificate: Rc<C>,
        local_ip: Ipv4Addr,
        dtls: D,
    ) -> Self {
        let udp = executor.block_on(UdpSocket::bind("0.0.0.0:0")).unwrap();
        let transport = WebRtcTransport::new(udp);
        let tx = transport.clone();
        let rx = transport.clone();
        executor.execute(Box::pin(async move { tx.read_loop().await }));

        executor.execute(Box::pin(async move { rx.write_loop().await }));

        Self {
            executor,
            signaling: Some(WebRtcSignalingChannel {
                signaling_tx: tx_half,
                signaling_rx: rx_half,
                engine: general_purpose::STANDARD,
            }),
            uuid: None,
            transport,
            certificate,
            remote_creds: None,
            local_creds: Default::default(),
            local_ip,
            dtls: Some(dtls),
        }
    }

    pub async fn run_ice_until_connected(&mut self, answer: &WebRtcSdp) -> Result<(), WebRtcError> {
        let (tx, rx) = smol::channel::bounded(1);

        //(TODO(RSDK-3060)) implement ICEError
        let ice_transport = self.transport.get_stun_channel().unwrap();
        let mut ice_agent = ICEAgent::new(
            rx,
            ice_transport,
            self.local_creds.clone(),
            self.remote_creds.as_ref().unwrap().clone(),
            self.local_ip,
        );

        log::info!("gathering local candidates");
        ice_agent.local_candidates().await.unwrap();

        self.signaling
            .as_mut()
            .ok_or(WebRtcError::SignalingDisconnected())?
            .send_sdp_answer(answer)
            .await?;
        for c in &ice_agent.local_candidates {
            log::debug!("sending local candidates {:?}", c);
            self.signaling
                .as_mut()
                .ok_or(WebRtcError::SignalingDisconnected())?
                .send_local_candidate(
                    c,
                    self.local_creds.u_frag.clone(),
                    self.uuid.as_ref().unwrap().clone(),
                )?;
        }
        self.signaling
            .as_mut()
            .ok_or(WebRtcError::SignalingDisconnected())?
            .send_done(self.uuid.as_ref().unwrap().clone())?;
        let sync = Arc::new(AtomicBool::new(false));
        let sync_clone = sync.clone();
        self.executor.execute(Box::pin(async move {
            ice_agent.run(sync).await;
        }));

        while !sync_clone.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(candidate) = self
                .signaling
                .as_mut()
                .ok_or(WebRtcError::SignalingDisconnected())?
                .next_remote_candidate()
                .timeout(Duration::from_millis(50))
                .await
            {
                match candidate {
                    Ok(candidate) => {
                        if let Some(c) = candidate {
                            log::debug!("received candidate : {}", &c);
                            tx.send(c).await.unwrap();
                        }
                    }
                    Err(e) => {
                        // TODO(RSDK-3854)
                        return Err(e);
                    }
                }
            }
        }
        let _ = self.signaling.take();
        Ok(())
    }

    pub async fn open_data_channel(&mut self) -> Result<Channel, WebRtcError> {
        let mut dtls = self.dtls.take().unwrap();

        let dtls_transport = self
            .transport
            .get_dtls_channel()
            .map_err(WebRtcError::Other)?;

        dtls.set_transport(dtls_transport);

        if let Ok(dtls_stream) = dtls.accept().await {
            let (c_tx, c_rx) = async_channel::unbounded();

            let sctp = Box::new(SctpConnector::new(dtls_stream, c_tx));
            let mut sctp = sctp.listen().await.unwrap();
            self.executor.execute(Box::pin(async move {
                sctp.run().await;
            }));
            return c_rx
                .recv()
                .await
                .map_err(|_| WebRtcError::DataChannelOpenError());
        }

        Err(WebRtcError::DataChannelOpenError())
    }

    pub(crate) fn into_transport(self) -> WebRtcTransport {
        self.transport
    }

    pub async fn answer(&mut self) -> Result<Box<WebRtcSdp>, WebRtcError> {
        let offer = self
            .signaling
            .as_mut()
            .ok_or(WebRtcError::SignalingDisconnected())?
            .wait_sdp_offer()
            .await?;

        let answer = SessionDescription::new_jsep_session_description(false);

        let attribute = offer
            .sdp
            .media_descriptions
            .get(0)
            .ok_or_else(|| WebRtcError::InvalidSDPOffer("no media description".to_owned()))?;

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
        let _ = self.uuid.insert(offer.uuid);

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

        Ok(Box::new(WebRtcSdp::new(
            answer,
            self.uuid.as_ref().unwrap().clone(),
        )))
    }
}
