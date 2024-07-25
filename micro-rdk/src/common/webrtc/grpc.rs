#![allow(dead_code)]
#![allow(clippy::read_zero_byte_vec)]
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use futures_lite::AsyncReadExt;
use prost::Message;

use crate::{
    common::grpc::{GrpcResponse, ServerError},
    google::rpc::Status,
    proto::rpc::webrtc::{
        self,
        v1::{Metadata, RequestHeaders, RequestMessage, Stream},
    },
};

use super::{api::WebRtcError, sctp::Channel};

#[cfg(feature = "camera")]
// sizeof(fake_image) + headers/encodings
static WEBRTC_GRPC_BUFFER_SIZE: usize = 1024 * 11;
#[cfg(not(feature = "camera"))]
static WEBRTC_GRPC_BUFFER_SIZE: usize = 9216;

#[derive(Debug, Default)]
pub struct WebRtcGrpcBody {
    data: Option<Bytes>,
    status: Status,
    trailers: Option<Metadata>,
}

impl WebRtcGrpcBody {
    fn new() -> Self {
        WebRtcGrpcBody {
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

impl GrpcResponse for WebRtcGrpcBody {
    fn put_data(&mut self, data: bytes::Bytes) {
        let _ = self.data.insert(data);
    }
    fn set_status(&mut self, code: i32, message: Option<String>) {
        self.status.code = code;
        if let Some(message) = message {
            self.status.message = message
        }
    }
    fn insert_trailer(&mut self, _: &'static str, _: &'_ str) {}
    fn get_data(&mut self) -> Bytes {
        self.data.take().unwrap()
    }
}

#[derive(Debug)]
struct RpcCall(
    webrtc::v1::RequestHeaders,
    Option<Instant>,
    Option<RequestMessage>,
);

pub struct WebRtcGrpcServer<S> {
    service: S,
    channel: Channel,
    stream: Option<webrtc::v1::Stream>,
    headers: Option<RequestHeaders>,
    streams: HashMap<u32, RpcCall>,
    buffer: BytesMut,
}

pub trait WebRtcGrpcService {
    fn unary_rpc(&mut self, method: &str, data: &Bytes) -> Result<Bytes, ServerError>;
    fn server_stream_rpc(
        &mut self,
        method: &str,
        data: &Bytes,
    ) -> Result<(Bytes, Instant), ServerError>;
}

impl<S> WebRtcGrpcServer<S>
where
    S: WebRtcGrpcService,
{
    pub fn new(channel: Channel, service: S) -> Self {
        Self {
            service,
            channel,
            stream: None,
            headers: None,
            streams: HashMap::new(),
            buffer: BytesMut::zeroed(WEBRTC_GRPC_BUFFER_SIZE),
        }
    }
    async fn send_response(&mut self, response: webrtc::v1::Response) -> Result<(), WebRtcError> {
        let len = response.encoded_len();
        let b = self.buffer.split_off(len);
        self.buffer.clear();
        response
            .encode(&mut self.buffer)
            .map_err(WebRtcError::GprcEncodeError)?;
        self.channel.write(&self.buffer[..len]).await?;
        self.buffer.unsplit(b);
        Ok(())
    }
    async fn process_rpc_request(
        &mut self,
        stream: Stream,
        msg: &RequestMessage,
        hdr: &RequestHeaders,
    ) -> Result<(Status, Option<Instant>), WebRtcError> {
        let method = &hdr.method;
        log::debug!("processing req {:?}", method);
        let ret = if let Some(pkt) = msg.packet_message.as_ref() {
            if method.contains("Stream") {
                match self.service.server_stream_rpc(method, &pkt.data) {
                    Ok(data) => {
                        self.send_rpc_response(data.0, stream).await?;
                        (
                            Status {
                                code: 0,
                                ..Default::default()
                            },
                            Some(data.1),
                        )
                    }
                    Err(e) => (e.to_status(), None),
                }
            } else {
                match self.service.unary_rpc(method, &pkt.data) {
                    Ok(data) => {
                        self.send_rpc_response(data, stream).await?;
                        (
                            Status {
                                code: 0,
                                ..Default::default()
                            },
                            None,
                        )
                    }
                    Err(e) => (e.to_status(), None),
                }
            }
        } else {
            (
                Status {
                    code: 0,
                    ..Default::default()
                },
                None,
            )
        };
        Ok(ret)
    }
    async fn send_rpc_response(&mut self, data: Bytes, stream: Stream) -> Result<(), WebRtcError> {
        let message_response = webrtc::v1::Response {
            stream: Some(stream),
            r#type: Some(webrtc::v1::response::Type::Message(
                webrtc::v1::ResponseMessage {
                    packet_message: Some(webrtc::v1::PacketMessage { data, eom: true }),
                },
            )),
        };
        self.send_response(message_response).await
    }
    async fn send_trailers(&mut self, stream: Stream, status: Status) -> Result<(), WebRtcError> {
        let trailer_response = webrtc::v1::Response {
            stream: Some(stream),
            r#type: Some(webrtc::v1::response::Type::Trailers(
                webrtc::v1::ResponseTrailers {
                    status: Some(status),
                    metadata: None,
                },
            )),
        };
        self.send_response(trailer_response).await
    }

    async fn next_rpc_call(&mut self) -> Result<u32, WebRtcError> {
        loop {
            let read = self
                .channel
                .read(&mut self.buffer)
                .await
                .map_err(WebRtcError::IoError)?;
            let req = webrtc::v1::Request::decode(&self.buffer[..read])
                .map_err(WebRtcError::GrpcDecodeError)?;
            if let Some(wrtc_type) = req.r#type {
                match wrtc_type {
                    webrtc::v1::request::Type::Headers(hdr) => {
                        let header_response = webrtc::v1::Response {
                            stream: req.stream.clone(),
                            r#type: Some(webrtc::v1::response::Type::Headers(
                                webrtc::v1::ResponseHeaders { metadata: None },
                            )),
                        };
                        let _ = self.streams.insert(
                            req.stream.as_ref().unwrap().id as u32,
                            RpcCall(hdr, None, None),
                        );

                        self.send_response(header_response).await?;
                    }
                    webrtc::v1::request::Type::Message(msg) => {
                        let stream = req.stream.unwrap();
                        let key = stream.id as u32;

                        if let Some(call) = self.streams.get_mut(&key) {
                            let _ = call.2.insert(msg);
                            return Ok(key);
                        } else {
                            log::info!("discarding stream {}", key);
                        }
                    }
                    webrtc::v1::request::Type::RstStream(rst) => {
                        log::debug!("reseting the stream");
                        if rst {
                            let stream = req.stream.unwrap();
                            let key = stream.id as u32;
                            let _ = self.streams.remove(&key);
                            self.send_trailers(
                                stream,
                                Status {
                                    code: 0,
                                    ..Default::default()
                                },
                            )
                            .await?;
                        }
                    }
                }
            }
        }
    }

    pub async fn next_request(&mut self) -> Result<(), WebRtcError> {
        let next_stream = self
            .streams
            .iter()
            .min_by(|a, b| {
                a.1 .1
                    .as_ref()
                    .map_or(Duration::MAX, |i| {
                        i.saturating_duration_since(Instant::now())
                    })
                    .cmp(&b.1 .1.as_ref().map_or(Duration::MAX, |i| {
                        i.saturating_duration_since(Instant::now())
                    }))
            })
            .map(|(k, v)| {
                (
                    *k,
                    v.1.map_or_else(async_io::Timer::never, async_io::Timer::at),
                )
            })
            .unwrap_or((0, async_io::Timer::never()));

        let id = futures_lite::future::or(async { self.next_rpc_call().await }, async {
            next_stream.1.await;
            Ok(next_stream.0)
        })
        .await?;
        if let Some(mut call) = self.streams.remove(&id) {
            let r = self
                .process_rpc_request(Stream { id: id as u64 }, call.2.as_ref().unwrap(), &call.0)
                .await?;
            if let Some(next) = r.1 {
                let _ = call.1.insert(next);
                let _ = self.streams.insert(id, call);
            } else {
                self.send_trailers(Stream { id: id as u64 }, r.0).await?;
            }
        }
        Ok(())
    }
}
