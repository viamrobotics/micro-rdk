#![allow(dead_code)]
use std::{
    fmt::Debug,
    io::{self, Cursor},
    net::Ipv4Addr,
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
use futures_lite::{future::block_on, StreamExt};
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
    ice::{ICEAgent, ICECredentials},
    io::WebRTCTransport,
    sctp::{Channel, SctpProto},
};

#[derive(Error, Debug)]
pub enum WebRTCError {
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

pub(crate) struct WebRTCSignaling {
    signaling_tx: GrpcMessageSender<AnswerResponse>,
    signaling_rx: GrpcMessageStream<AnswerRequest>,
    engine: general_purpose::GeneralPurpose,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SdpOffer {
    pub r#type: String,
    pub sdp: String,
}

#[derive(Debug, Clone)]
pub struct WebRTCSdp {
    sdp: SessionDescription,
    uuid: String,
}

impl WebRTCSdp {
    pub fn new(sdp: SessionDescription, uuid: String) -> Self {
        WebRTCSdp { sdp, uuid }
    }
}

impl WebRTCSignaling {
    pub(crate) async fn wait_sdp_offer(&mut self) -> Result<WebRTCSdp, WebRTCError> {
        loop {
            match self.signaling_rx.next().await {
                None => {
                    return Err(WebRTCError::SignalingDisconnected());
                }
                Some(req) => {
                    if let Some(stage) = req.stage.clone() {
                        match stage {
                            answer_request::Stage::Init(s) => {
                                let sdp_decoded = self
                                    .engine
                                    .decode(s.sdp)
                                    .map_err(|e| WebRTCError::InvalidSDPOffer(e.to_string()))?;
                                let sdp_decoded: SdpOffer =
                                    serde_json::from_slice(sdp_decoded.as_slice())
                                        .map_err(|e| WebRTCError::InvalidSDPOffer(e.to_string()))?;

                                if sdp_decoded.r#type != "offer" {
                                    return Err(WebRTCError::InvalidSDPOffer(format!(
                                        "unexpected type {}",
                                        sdp_decoded.r#type
                                    )));
                                }

                                log::debug!("received an SDP offer {:?}", sdp_decoded);

                                let mut cursor = Cursor::new(sdp_decoded.sdp);
                                let sdp = sdp::SessionDescription::unmarshal(&mut cursor)
                                    .map_err(|e| WebRTCError::InvalidSDPOffer(e.to_string()))?;
                                return Ok(WebRTCSdp::new(sdp, req.uuid));
                            }
                            answer_request::Stage::Error(s) => {
                                if let Some(status) = s.status {
                                    return Err(WebRTCError::SignalingError(status.message));
                                }
                                return Err(WebRTCError::SignalingError("unknown".to_owned()));
                            }
                            _ => {
                                continue;
                            }
                        }
                    } else {
                        return Err(WebRTCError::InvalidSignalingRequest);
                    }
                }
            }
        }
    }
    pub(crate) async fn send_sdp_answer(&mut self, sdp: WebRTCSdp) -> Result<(), WebRTCError> {
        let answer = SdpOffer {
            r#type: "answer".to_owned(),
            sdp: sdp.sdp.marshal(),
        };
        let answer = self
            .engine
            .encode(serde_json::to_string(&answer).map_err(WebRTCError::AnswerMarshalError)?);

        let answer = AnswerResponse {
            uuid: sdp.uuid,
            stage: Some(answer_response::Stage::Init(AnswerResponseInitStage {
                sdp: answer,
            })),
        };
        match self.signaling_tx.send_message(answer) {
            Err(e) => {
                log::error!("error sending signaling message: {:?}", e);
                Err(WebRTCError::SignalingDisconnected())
            }
            Ok(_) => Ok(()),
        }
    }
    pub(crate) fn send_local_candidate(
        &mut self,
        candidate: &Candidate,
        ufrag: String,
        uuid: String,
    ) -> Result<(), WebRTCError> {
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
            Err(_) => Err(WebRTCError::SignalingDisconnected()),
            Ok(_) => Ok(()),
        }
    }
    pub(crate) async fn next_remote_candidate(&mut self) -> Result<Option<Candidate>, WebRTCError> {
        match self.signaling_rx.next().await {
            None => Err(WebRTCError::SignalingDisconnected()),
            Some(req) => {
                if let Some(stage) = req.stage {
                    match stage {
                        answer_request::Stage::Update(c) => {
                            if let Some(c) = c.candidate {
                                log::error!("received candidaate {}", c.candidate);
                                let c = c.candidate.try_into().ok();
                                return Ok(c);
                            } else {
                                log::error!("received no candidates with this update request");
                                return Ok(None);
                            }
                        }
                        answer_request::Stage::Error(s) => {
                            if let Some(status) = s.status {
                                return Err(WebRTCError::SignalingError(status.message));
                            }
                            return Err(WebRTCError::SignalingError("unknown".to_owned()));
                        }
                        answer_request::Stage::Done(_) => {
                            return Ok(None);
                        }
                        _ => {
                            return Err(WebRTCError::SignalingError("unexpected stage".to_owned()))
                        }
                    }
                }
                Ok(None)
            }
        }
    }
    pub fn send_done(&mut self, uuid: String) -> Result<(), WebRTCError> {
        let answer = AnswerResponse {
            uuid,
            stage: Some(answer_response::Stage::Done(AnswerResponseDoneStage {})),
        };
        match self.signaling_tx.send_message(answer) {
            Err(_) => Err(WebRTCError::SignalingDisconnected()),
            Ok(_) => Ok(()),
        }
    }
}

#[cfg(feature = "native")]
type Executor<'a> = NativeExecutor<'a>;
#[cfg(feature = "esp32")]
type Executor<'a> = Esp32Executor<'a>;

pub struct WebRTCApi<'a, S, D> {
    executor: Executor<'a>,
    signaling: Option<WebRTCSignaling>,
    uuid: Option<String>,
    transport: WebRTCTransport,
    certificate: Rc<S>,
    local_creds: ICECredentials,
    remote_creds: Option<ICECredentials>,
    local_ip: Ipv4Addr,
    dtls: Option<D>,
}

impl<'a, S, D> WebRTCApi<'a, S, D>
where
    S: Certificate,
    D: DtlsConnector,
{
    pub(crate) fn new(
        executor: Executor<'a>,
        tx_half: GrpcMessageSender<AnswerResponse>,
        rx_half: GrpcMessageStream<AnswerRequest>,
        certificate: Rc<S>,
        local_ip: Ipv4Addr,
        dtls: D,
    ) -> Self {
        let udp = block_on(executor.run(async { UdpSocket::bind("0.0.0.0:61205").await.unwrap() }));
        let transport = WebRTCTransport::new(udp);
        let tx = transport.clone();
        let rx = transport.clone();
        executor.spawn(async move { tx.read_loop().await }).detach();
        executor
            .spawn(async move { rx.write_loop().await })
            .detach();
        Self {
            executor: executor.clone(),
            signaling: Some(WebRTCSignaling {
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

    pub async fn run_ice_until_connected(&mut self) -> Result<(), WebRTCError> {
        let (tx, rx) = smol::channel::bounded(1);

        //(TODO) implement ICEError
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

        for c in &ice_agent.local_candidates {
            self.signaling
                .as_mut()
                .ok_or(WebRTCError::SignalingDisconnected())?
                .send_local_candidate(
                    c,
                    self.local_creds.u_frag.clone(),
                    self.uuid.as_ref().unwrap().clone(),
                )?;
        }
        let sync = Arc::new(AtomicBool::new(false));
        let sync_clone = sync.clone();
        self.executor
            .spawn(async move {
                ice_agent.run(sync).await;
            })
            .detach();

        while !sync_clone.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(candidate) = self
                .signaling
                .as_mut()
                .ok_or(WebRTCError::SignalingDisconnected())?
                .next_remote_candidate()
                .timeout(Duration::from_millis(250))
                .await
            {
                match candidate {
                    Ok(candidate) => {
                        if let Some(c) = candidate {
                            tx.send(c).await.unwrap();
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "received error while gathering remote candidates continuing anyway {:?}",
                            e
                        );
                    }
                }
            }
        }
        self.signaling
            .as_mut()
            .ok_or(WebRTCError::SignalingDisconnected())?
            .send_done(self.uuid.take().unwrap())?;
        let _ = self.signaling.take();
        Ok(())
    }

    pub async fn open_data_channel(&mut self) -> Result<Channel, WebRTCError> {
        let mut dtls = self.dtls.take().unwrap();

        let dtls_transport = self
            .transport
            .get_dtls_channel()
            .map_err(WebRTCError::Other)?;

        dtls.set_transport(dtls_transport);

        if let Ok(dtls_stream) = dtls.accept().await {
            let (c_tx, c_rx) = async_channel::unbounded();

            let mut sctp = Box::new(SctpProto::new(dtls_stream, self.executor.clone(), c_tx));
            sctp.listen().await.unwrap();
            self.executor
                .spawn(async move {
                    sctp.run().await;
                })
                .detach();
            return Ok(c_rx.recv().await.unwrap());
        }

        Err(WebRTCError::DataChannelOpenError())
    }

    pub async fn answer(&mut self) -> Result<(), WebRTCError> {
        let offer = self
            .signaling
            .as_mut()
            .ok_or(WebRTCError::SignalingDisconnected())?
            .wait_sdp_offer()
            .await?;
        let answer = SessionDescription::new_jsep_session_description(false);

        let remote_creds = ICECredentials::new(
            offer.sdp.media_descriptions[0]
                .attribute("ice-ufrag")
                .unwrap()
                .unwrap()
                .to_owned(),
            offer.sdp.media_descriptions[0]
                .attribute("ice-pwd")
                .unwrap()
                .unwrap()
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

        let answer = WebRTCSdp::new(answer, self.uuid.as_ref().unwrap().clone());
        self.signaling
            .as_mut()
            .ok_or(WebRTCError::SignalingDisconnected())?
            .send_sdp_answer(answer)
            .await?;
        Ok(())
    }
}
