use super::api::{WebRtcSdp, WebRtcSignalingChannel};
use crate::{
    common::{
        exec::Executor,
        grpc::{GrpcError, ServerError},
        webrtc::api::{SdpOffer, WebRtcError},
    },
    proto::rpc::webrtc::v1::{
        CallRequest, CallResponse, CallUpdateRequest, CallUpdateResponse,
        OptionalWebRtcConfigRequest, OptionalWebRtcConfigResponse,
    },
};
use async_channel::{RecvError, Sender};
use base64::{engine::general_purpose, Engine};
use either::Either;
use std::{collections::HashMap, io::Cursor, sync::Mutex};
use uuid::Uuid;

pub(crate) struct LocalSignaling {
    pub(crate) tx: async_channel::Sender<Result<CallResponse, ServerError>>,
    pub(crate) rx: async_channel::Receiver<CallUpdateRequest>,
}

pub(crate) struct SignalingServer {
    pub executor: Executor,
    sender: Sender<Box<WebRtcSignalingChannel>>,
    pending: Mutex<HashMap<String, async_channel::Sender<CallUpdateRequest>>>,
}

impl SignalingServer {
    pub fn new(executor: Executor, sender: Sender<Box<WebRtcSignalingChannel>>) -> Self {
        Self {
            executor,
            sender,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn optional_webrtc_config(
        &self,
        _request: OptionalWebRtcConfigRequest,
    ) -> Result<OptionalWebRtcConfigResponse, ServerError> {
        Ok(OptionalWebRtcConfigResponse { config: None })
    }

    pub async fn call(
        &self,
        request: CallRequest,
        responses: Sender<Result<CallResponse, ServerError>>,
    ) -> Result<(), ServerError> {
        if request.disable_trickle {
            return Err(ServerError::new(
                GrpcError::RpcInvalidArgument,
                Some(Box::new(WebRtcError::InvalidSDPOffer(
                    "micro-rdk only supports trickle ICE".into(),
                ))),
            ));
        }

        // TODO(RSDK-9247): Make this a lazy global.
        let engine: Box<general_purpose::GeneralPurpose> = general_purpose::STANDARD.into();
        let sdp_decoded = engine.decode(request.sdp.as_str()).map_err(|e| {
            ServerError::new(
                GrpcError::RpcInvalidArgument,
                Some(Box::new(WebRtcError::InvalidSDPOffer(e.to_string()))),
            )
        })?;
        let sdp_decoded: SdpOffer =
            serde_json::from_slice(sdp_decoded.as_slice()).map_err(|e| {
                ServerError::new(
                    GrpcError::RpcInvalidArgument,
                    Some(Box::new(WebRtcError::InvalidSDPOffer(e.to_string()))),
                )
            })?;
        if sdp_decoded.sdp_type != "offer" {
            return Err(ServerError::new(
                GrpcError::RpcInvalidArgument,
                Some(Box::new(WebRtcError::InvalidSDPOffer(format!(
                    "unexpected type {}",
                    sdp_decoded.sdp_type
                )))),
            ));
        }
        let mut cursor = Cursor::new(sdp_decoded.sdp);
        let sdp = sdp::SessionDescription::unmarshal(&mut cursor).map_err(|e| {
            ServerError::new(
                GrpcError::RpcInvalidArgument,
                Some(Box::new(WebRtcError::InvalidSDPOffer(e.to_string()))),
            )
        })?;
        let sdp = Box::new(WebRtcSdp::new(sdp, Uuid::new_v4().into()));

        // TODO: Should the channels be bounded?
        let (update_tx, update_rx) = async_channel::unbounded::<CallUpdateRequest>();
        let (response_tx, response_rx) =
            async_channel::bounded::<Result<CallResponse, ServerError>>(1);

        let local_signaling = LocalSignaling {
            tx: response_tx,
            rx: update_rx,
        };

        let channel = Box::new(WebRtcSignalingChannel::new(
            Either::Right(local_signaling),
            sdp,
        ));

        match self.sender.send(channel).await {
            Ok(()) => {
                let uuid: Option<String> = None;
                let mut uuid_guard = scopeguard::guard(uuid, |uuid| {
                    if let Some(uuid) = uuid {
                        self.pending.lock().unwrap().remove(uuid.as_str());
                    }
                });

                loop {
                    match response_rx.recv().await {
                        Ok(Ok(response)) => {
                            match uuid_guard.as_ref() {
                                Some(uuid) => {
                                    if uuid != &response.uuid {
                                        break Err(ServerError::new(
                                            GrpcError::RpcInternal,
                                            Some(format!("WebRtcSignalingChannel returned an inconsistent SDP UUID: expected {}, observed {}", uuid, response.uuid).into())
                                        ));
                                    }
                                }
                                None => {
                                    self.pending.lock().unwrap().insert(
                                        uuid_guard.insert(response.uuid.clone()).clone(),
                                        update_tx.clone(),
                                    );
                                }
                            }

                            // We got a CallResponse out of the
                            // machinery - send it along.
                            //
                            // TODO: error handling
                            let _ = responses.send(Ok(response)).await;
                        }
                        Ok(Err(server_err)) => {
                            // We got an error out of the machinery,
                            // so fail the RPC.
                            break Err(server_err);
                        }
                        Err(RecvError) => {
                            // The channel is closed, so the state machine
                            // is complete. End the RPC with success and close the response stream.
                            responses.close();
                            break Ok(());
                        }
                    }
                }
            }
            Err(_) => Err(ServerError::new(
                GrpcError::RpcInternal,
                Some("Failed sending inbound call request to signaling channel".into()),
            )),
        }
    }

    pub fn call_update(
        &self,
        request: CallUpdateRequest,
    ) -> Result<CallUpdateResponse, ServerError> {
        // Clone the channel under the lock, but don't hold the lock beyond that.
        let update_tx = self
            .pending
            .lock()
            .unwrap()
            .get(&request.uuid)
            .and_then(|v| v.clone().into());
        match update_tx {
            Some(update_tx) => {
                let _ = update_tx.send_blocking(request);
                Ok(CallUpdateResponse {})
            }
            None => {
                // It is tempting to reply with `GrpcError::RpcInvalidArgument` here, but resist!
                // Clients (at least goutils) appear to interpret the failed `CallUpdate` RPC as
                // failing the entire WebRTC conversation, when really we have just raced between
                // deciding that we are complete in the `Call` RPC on the server side, and the
                // client deciding to send us another `CallUpdateRequest`.
                Ok(CallUpdateResponse {})
            }
        }
    }
}
