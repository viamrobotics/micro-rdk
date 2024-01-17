#![allow(dead_code)]
#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;
#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::Stream;
use h2::{client::SendRequest, Reason, RecvStream, SendStream};
use hyper::header::HeaderMap;
use hyper::{http::status, Method, Request};

use smol::Task;
use std::{marker::PhantomData, task::Poll};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Error, Debug)]
pub enum GrpcClientError {
    #[error(transparent)]
    ProtoError(#[from] h2::Error),
    #[error(transparent)]
    ConversionError(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    MessageEncodingError(#[from] prost::EncodeError),
    #[error("http request error {0}")]
    HttpStatusError(status::StatusCode),
    #[error(transparent)]
    HyperHttpError(#[from] hyper::http::Error),
    #[error("grpc error code {code:?}, message {message:?}")]
    GrpcError { code: i8, message: String },
}

pub(crate) struct GrpcMessageSender<T> {
    sender_half: SendStream<Bytes>,
    _marker: PhantomData<T>,
}

impl<T> Drop for GrpcMessageSender<T> {
    fn drop(&mut self) {
        self.sender_half.send_reset(Reason::CANCEL);
    }
}

impl<T> GrpcMessageSender<T>
where
    T: prost::Message + std::default::Default,
{
    pub(crate) fn new(sender_half: SendStream<Bytes>) -> Self {
        Self {
            sender_half,
            _marker: PhantomData,
        }
    }
    pub(crate) fn send_message(&mut self, message: T) -> Result<(), GrpcClientError> {
        let body: Bytes = {
            let mut buf = BytesMut::with_capacity(message.encoded_len() + 5);
            buf.put_u8(0);
            buf.put_u32(message.encoded_len().try_into()?);
            let mut msg = buf.split_off(5);
            message.encode(&mut msg)?;
            buf.unsplit(msg);
            buf.into()
        };
        self.sender_half
            .send_data(body, false)
            .map_err(GrpcClientError::ProtoError)
    }
    pub(crate) fn send_empty_body(&mut self) -> Result<(), GrpcClientError> {
        self.sender_half
            .send_data(Bytes::new(), false)
            .map_err(GrpcClientError::ProtoError)
    }
}

pub(crate) struct GrpcMessageStream<T> {
    receiver_half: RecvStream,
    _marker: PhantomData<T>,
    buffer: Bytes,
}

impl<T> Drop for GrpcMessageStream<T> {
    fn drop(&mut self) {
        let capa = self.receiver_half.flow_control().used_capacity();
        let _ = self.receiver_half.flow_control().release_capacity(capa);
        self.buffer.clear();
    }
}

impl<T> Unpin for GrpcMessageStream<T> {}

impl<T> GrpcMessageStream<T> {
    pub(crate) fn new(receiver_half: RecvStream) -> Self {
        Self {
            receiver_half,
            _marker: PhantomData,
            buffer: Bytes::new(),
        }
    }
    pub(crate) fn by_ref(&mut self) -> &mut Self {
        self
    }
}

impl<T> Stream for GrpcMessageStream<T>
where
    T: prost::Message + std::default::Default,
{
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.buffer.is_empty() {
            let chunk = match self.receiver_half.poll_data(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(r) => match r {
                    Some(r) => match r {
                        Err(_) => return Poll::Ready(None),
                        Ok(r) => r,
                    },
                    None => return Poll::Ready(None),
                },
            };
            self.buffer = chunk;
        }

        // Split off the length prefixed message containing the compressed flag (B0) and the message length (B1-B4)
        let mut delim = self.buffer.split_to(5);
        // Discard compression flag
        let _ = delim.split_to(1);

        let len = u32::from_be_bytes(delim.as_ref().try_into().unwrap());

        let message = self.buffer.split_to(len as usize);
        let _ = self
            .receiver_half
            .flow_control()
            .release_capacity(message.len() + 5);

        let message = match T::decode(message) {
            Err(e) => {
                log::error!("decoding error {:?}", e);
                return Poll::Pending;
            }
            Ok(m) => m,
        };
        Poll::Ready(Some(message))
    }
}
#[cfg(feature = "native")]
type Executor<'a> = NativeExecutor<'a>;
#[cfg(feature = "esp32")]
type Executor<'a> = Esp32Executor<'a>;
pub struct GrpcClient<'a> {
    executor: Executor<'a>,
    http2_connection: SendRequest<Bytes>,
    #[allow(dead_code)]
    http2_task: Option<Task<()>>,
    uri: &'a str,
}

impl<'a> Drop for GrpcClient<'a> {
    fn drop(&mut self) {
        let _ = self.http2_task.take();
    }
}

impl<'a> GrpcClient<'a> {
    pub async fn new<T>(
        io: T,
        executor: Executor<'a>,
        uri: &'a str,
    ) -> Result<GrpcClient<'a>, GrpcClientError>
    where
        T: AsyncRead + AsyncWrite + Unpin + 'a,
    {
        let (http2_connection, conn) = {
            let builder = h2::client::Builder::new()
                .initial_connection_window_size(4096)
                .initial_window_size(4096)
                .max_concurrent_reset_streams(1)
                .max_concurrent_streams(1)
                .handshake(io);
            let conn = builder.await.unwrap();
            (conn.0, Box::new(conn.1))
        };

        let http2_task = executor.spawn(async move {
            if let Err(e) = conn.await {
                log::error!("GrpcClient failed with {:?}", e);
            }
        });
        Ok(Self {
            executor,
            http2_connection,
            http2_task: Some(http2_task),
            uri,
        })
    }

    pub(crate) fn build_request(
        &self,
        path: &str,
        jwt: Option<&str>,
        rpc_host: &str,
    ) -> Result<Request<()>, GrpcClientError> {
        let mut uri = self.uri.to_owned();
        uri.push_str(path);

        let mut r = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .header("rpc-host", rpc_host)
            .header("user-agent", "esp32");
        if let Some(jwt) = jwt {
            r = r.header("authorization", jwt);
        };

        r.body(()).map_err(GrpcClientError::HyperHttpError)
    }

    pub(crate) async fn send_request_bidi<R, P>(
        &mut self,
        r: Request<()>,
        message: Option<R>, // we shouldn't need this to get server headers when initiating a
                            // bidi stream
    ) -> Result<(GrpcMessageSender<R>, GrpcMessageStream<P>), GrpcClientError>
    where
        R: prost::Message + std::default::Default,
        P: prost::Message + std::default::Default,
    {
        let http2_connection = self.http2_connection.clone();
        let mut http2_connection = http2_connection.ready().await?;

        let (response, send) = http2_connection.send_request(r, false)?;

        let mut r: GrpcMessageSender<R> = GrpcMessageSender::new(send);

        if let Some(message) = message {
            r.send_message(message)?;
        } else {
            r.send_empty_body()?
        }

        let (part, body) = response.await?.into_parts();

        if part.status != status::StatusCode::OK {
            return Err(GrpcClientError::HttpStatusError(part.status));
        }
        let p: GrpcMessageStream<P> = GrpcMessageStream::new(body);

        Ok((r, p))
    }

    pub(crate) async fn send_request(
        &mut self,
        r: Request<()>,
        body: Bytes,
    ) -> Result<(Bytes, HeaderMap), GrpcClientError> {
        let http2_connection = self.http2_connection.clone();
        // verify if the server can accept a new HTTP2 stream
        let mut http2_connection = http2_connection.ready().await?;

        // send the header and let the server know more data are coming
        let (response, mut send) = http2_connection.send_request(r, false)?;
        // send the body of the request and let the server know we have nothing else to send
        send.send_data(body, true)?;

        let (part, mut body) = response.await?.into_parts();

        if part.status != status::StatusCode::OK {
            log::error!("received status code {}", part.status.to_string());
        }

        let mut response_buf = BytesMut::with_capacity(1024);

        // TODO read the first 5 bytes so we know how much data to expect and we can allocate appropriately
        while let Some(chunk) = body.data().await {
            let chunk = chunk?;
            response_buf.put_slice(&chunk);
            let _ = body.flow_control().release_capacity(chunk.len());
        }

        let trailers = body.trailers().await?;

        if let Some(trailers) = trailers {
            match trailers.get("grpc-status") {
                Some(status) => {
                    // if we get an unparsable grpc status message we default to -1 (not a valid grpc error code)
                    let grpc_code: i8 =
                        str::parse::<i8>(status.to_str().unwrap_or("")).unwrap_or(-1);
                    if grpc_code != 0 {
                        match trailers.get("grpc-message") {
                            Some(message) => {
                                return Err(GrpcClientError::GrpcError {
                                    code: grpc_code,
                                    message: message.to_str().unwrap_or("").to_owned(),
                                });
                            }
                            None => {
                                return Err(GrpcClientError::GrpcError {
                                    code: grpc_code,
                                    message: String::new(),
                                });
                            }
                        }
                    }
                }
                None => {
                    return Err(GrpcClientError::GrpcError {
                        code: 0,
                        message: "received grpc trailers without a grpc-status".to_owned(),
                    });
                }
            }
        }
        Ok((response_buf.into(), part.headers))
    }
}
