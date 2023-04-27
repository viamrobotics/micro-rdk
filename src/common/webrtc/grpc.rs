#![allow(dead_code)]
#![allow(clippy::read_zero_byte_vec)]
use bytes::Bytes;
use futures_lite::AsyncReadExt;
use prost::Message;

use crate::{
    common::grpc::GrpcResponse,
    google::rpc::Status,
    proto::rpc::webrtc::{
        self,
        v1::{Metadata, RequestHeaders},
    },
};

use super::{api::WebRTCError, sctp::Channel};

#[derive(Debug, Default)]
pub struct WebRTCGrpcBody {
    data: Option<Bytes>,
    status: Status,
    trailers: Option<Metadata>,
}

impl WebRTCGrpcBody {
    fn new() -> Self {
        WebRTCGrpcBody {
            data: None,
            status: Status {
                code: 0,
                message: String::new(),
                details: Vec::new(),
            },
            trailers: None,
        }
    }
}

impl GrpcResponse for WebRTCGrpcBody {
    fn put_data(&mut self, data: bytes::Bytes) {
        let _ = self.data.insert(data);
    }
    fn set_status(&mut self, code: i32, message: Option<&'_ str>) {
        self.status.code = code;
        if let Some(message) = message {
            self.status.message = message.to_owned();
        }
    }
    fn insert_trailer(&mut self, _: &'static str, _: &'_ str) {}
    fn get_data(&mut self) -> Bytes {
        self.data.take().unwrap()
    }
}

pub struct WebRTCGrpcServer<S> {
    service: S,
    channel: Channel,
    stream: Option<webrtc::v1::Stream>,
    headers: Option<RequestHeaders>,
}

pub trait WebRTCGrpcService {
    fn unary_rpc(&mut self, method: &str, data: &Bytes) -> Result<Bytes, Status>;
}

impl<S> WebRTCGrpcServer<S>
where
    S: WebRTCGrpcService,
{
    pub fn new(channel: Channel, service: S) -> Self {
        Self {
            service,
            channel,
            stream: None,
            headers: None,
        }
    }
    async fn send_response(
        &mut self,
        buf: &mut Vec<u8>,
        response: webrtc::v1::Response,
    ) -> Result<(), WebRTCError> {
        let len = response.encoded_len();
        response.encode(buf).map_err(WebRTCError::GprcEncodeError)?;
        self.channel.write(&buf[..len]).await;
        Ok(())
    }
    pub async fn next_request(&mut self) -> Result<(), WebRTCError> {
        let mut msg_buffer = Vec::with_capacity(1200);

        let wrtc_msg = {
            unsafe { msg_buffer.set_len(1200) };
            let read = self
                .channel
                .read(&mut msg_buffer)
                .await
                .map_err(WebRTCError::IoError)?;
            webrtc::v1::Request::decode(&msg_buffer[..read])
                .map_err(WebRTCError::GrpcDecodeError)?
        };

        if let Some(wrtc_type) = wrtc_msg.r#type {
            match wrtc_type {
                webrtc::v1::request::Type::Headers(hdr) => {
                    let header_response = webrtc::v1::Response {
                        stream: wrtc_msg.stream.clone(),
                        r#type: Some(webrtc::v1::response::Type::Headers(
                            webrtc::v1::ResponseHeaders { metadata: None },
                        )),
                    };
                    let _ = self.stream.insert(wrtc_msg.stream.unwrap());
                    let _ = self.headers.insert(hdr);
                    msg_buffer.clear();
                    self.send_response(&mut msg_buffer, header_response).await?;
                }
                webrtc::v1::request::Type::Message(msg) => {
                    let stream = wrtc_msg.stream.unwrap();
                    if stream != *self.stream.as_ref().unwrap() {
                        log::error!("unexpected stream id {:?}", stream);
                    }
                    log::info!(
                        "do we have a message {:?} is it eos {:?} msg {:?}",
                        msg.has_message,
                        msg.eos,
                        msg
                    );

                    if let Some(pkt) = msg.packet_message {
                        let status = match self
                            .service
                            .unary_rpc(&self.headers.as_ref().unwrap().method, &pkt.data)
                        {
                            Ok(data) => {
                                let message_response = webrtc::v1::Response {
                                    stream: Some(stream.clone()),
                                    r#type: Some(webrtc::v1::response::Type::Message(
                                        webrtc::v1::ResponseMessage {
                                            packet_message: Some(webrtc::v1::PacketMessage {
                                                data,
                                                eom: true,
                                            }),
                                        },
                                    )),
                                };

                                msg_buffer.clear();
                                self.send_response(&mut msg_buffer, message_response)
                                    .await?;
                                Status {
                                    code: 0,
                                    ..Default::default()
                                }
                            }
                            Err(status) => status,
                        };
                        // this is a work around so app.viam.com don't drop the connection because
                        // we sent trailers. When we support Server Side streaming this would need to be
                        // removed
                        if &self.headers.as_ref().unwrap().method
                            != "/viam.robot.v1.RobotService/StreamStatus"
                        {
                            let trailer_response = webrtc::v1::Response {
                                stream: Some(stream.clone()),
                                r#type: Some(webrtc::v1::response::Type::Trailers(
                                    webrtc::v1::ResponseTrailers {
                                        status: Some(status),
                                        metadata: None,
                                    },
                                )),
                            };
                            msg_buffer.clear();
                            self.send_response(&mut msg_buffer, trailer_response)
                                .await?;
                        }
                    }
                }
                webrtc::v1::request::Type::RstStream(rst) => {
                    log::info!("reseting stream ");
                    if rst {
                        let _ = self.stream.take();
                    }
                }
            }
        }
        Ok(())
    }
}