#![allow(dead_code)]
#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;
#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;
use async_channel::Sender;
use async_io::Timer;
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::{ready, Future, FutureExt, Stream};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use hyper::body::{Body, Incoming};
use hyper::client::conn::http2::SendRequest;
use hyper::header::HeaderMap;
use hyper::rt::{self, Sleep};
use hyper::{http::status, Method, Request};

use async_executor::Task;
use std::pin::Pin;
use std::time::Instant;
use std::{marker::PhantomData, task::Poll};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GrpcClientError {
    #[error(transparent)]
    ConversionError(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    MessageEncodingError(#[from] prost::EncodeError),
    #[error(transparent)]
    MessageDecodingError(#[from] prost::DecodeError),
    #[error("http request error {0}")]
    HttpStatusError(status::StatusCode),
    #[error(transparent)]
    HyperError(#[from] hyper::Error),
    #[error(transparent)]
    HyperHttpError(#[from] hyper::http::Error),
    #[error("grpc error code {code:?}, message {message:?}")]
    GrpcError { code: i8, message: String },
    #[error(transparent)]
    ErrorSendingToAStream(#[from] async_channel::SendError<Bytes>),
    #[error("frame error {0}")]
    FrameError(String),
}

pub(crate) struct GrpcMessageSender<T> {
    sender_half: Sender<Bytes>,
    _marker: PhantomData<T>,
}

impl<T> GrpcMessageSender<T>
where
    T: prost::Message + std::default::Default,
{
    pub(crate) fn new(sender_half: Sender<Bytes>) -> Self {
        Self {
            sender_half,
            _marker: PhantomData,
        }
    }
    pub(crate) async fn send_message(&mut self, message: T) -> Result<(), GrpcClientError> {
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
            .send(body)
            .await
            .map_err(GrpcClientError::ErrorSendingToAStream)
    }
    pub(crate) async fn send_empty_body(&mut self) -> Result<(), GrpcClientError> {
        self.sender_half
            .send(Bytes::new())
            .await
            .map_err(GrpcClientError::ErrorSendingToAStream)
    }
}

pub(crate) struct GrpcMessageStream<T> {
    receiver_half: Incoming,
    _marker: PhantomData<T>,
    buffer: BytesMut,
}

impl<T> Unpin for GrpcMessageStream<T> {}

impl<T> GrpcMessageStream<T>
where
    T: prost::Message + std::default::Default,
{
    pub(crate) fn new(receiver_half: Incoming) -> Self {
        Self {
            receiver_half,
            _marker: PhantomData,
            buffer: BytesMut::new(),
        }
    }
    pub(crate) fn by_ref(&mut self) -> &mut Self {
        self
    }
    fn parse_message(&mut self) -> Option<Result<T, GrpcClientError>> {
        if self.buffer.len() > 5 {
            let len =
                u32::from_be_bytes(self.buffer.split_at(5).0[1..5].try_into().unwrap()) as usize;
            if len + 5 <= self.buffer.len() {
                let _ = self.buffer.split_to(5); // discard 5 bytes
                let msg = self.buffer.split_to(len).freeze();
                let message = T::decode(msg).map_err(Into::into);
                return Some(message);
            }
        }
        None
    }
}

impl<T> Stream for GrpcMessageStream<T>
where
    T: prost::Message + std::default::Default,
{
    type Item = Result<T, GrpcClientError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let message = self.parse_message();
        if message.is_some() {
            return Poll::Ready(message);
        }
        let chunk = ready!(Pin::new(&mut self.receiver_half).poll_frame(cx));

        // None would indicated a terminated HTTP2 stream
        if chunk.is_none() {
            return Poll::Ready(None);
        }

        // safe to unwrap since we know chunk is not None
        let frame = match chunk.unwrap() {
            Err(e) => {
                return Poll::Ready(Some(Err(e.into())));
            }
            Ok(frame) => frame,
        };

        // if we have trailers this RPC can be closed
        let data = match frame.into_data() {
            Ok(data) => data,
            Err(e) => {
                if let Some(trailers) = e.trailers_ref() {
                    if trailers.contains_key("grpc-message") && trailers.contains_key("grpc-status")
                    {
                        return Poll::Ready(Some(Err(GrpcClientError::GrpcError {
                            code: trailers
                                .get("grpc-status")
                                .unwrap()
                                .to_str()
                                .unwrap_or("")
                                .parse::<i8>()
                                .unwrap_or(127), // if status code cannot be parsed return 127
                            message: trailers
                                .get("grpc-message")
                                .unwrap()
                                .to_str()
                                .unwrap_or("couldn't parse message") // message couldn't be extracted from header
                                .to_owned(),
                        })));
                    }
                }
                return Poll::Ready(Some(Err(GrpcClientError::FrameError(format!("{:?}", e)))));
            }
        };

        self.buffer.extend(data);
        let message = self.parse_message();
        if message.is_some() {
            return Poll::Ready(message);
        }

        Poll::Pending
    }
}
#[cfg(feature = "native")]
type Executor = NativeExecutor;
#[cfg(feature = "esp32")]
type Executor = Esp32Executor;
pub struct GrpcClient {
    executor: Executor,
    http2_connection: SendRequest<BoxBody<Bytes, hyper::Error>>,
    #[allow(dead_code)]
    http2_task: Option<Task<()>>,
    uri: String,
}

struct AsyncioSleep(Timer);

impl Sleep for AsyncioSleep {}

impl AsyncioSleep {
    fn reset(mut self: Pin<&mut Self>, deadline: Instant) {
        self.0.set_at(deadline)
    }
}

impl Future for AsyncioSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let _ = ready!(self.0.poll(cx));
        Poll::Ready(())
    }
}

#[derive(Default, Clone, Debug)]
struct H2Timer;

impl rt::Timer for H2Timer {
    fn sleep(&self, duration: std::time::Duration) -> std::pin::Pin<Box<dyn rt::Sleep>> {
        Box::pin(AsyncioSleep(Timer::after(duration)))
    }
    fn sleep_until(&self, deadline: std::time::Instant) -> std::pin::Pin<Box<dyn rt::Sleep>> {
        Box::pin(AsyncioSleep(Timer::at(deadline)))
    }
    fn reset(
        &self,
        sleep: &mut std::pin::Pin<Box<dyn rt::Sleep>>,
        new_deadline: std::time::Instant,
    ) {
        if let Some(timer) = sleep.as_mut().downcast_mut_pin::<AsyncioSleep>() {
            timer.reset(new_deadline)
        }
    }
}

impl GrpcClient {
    pub async fn new<T>(io: T, executor: Executor, uri: &str) -> Result<GrpcClient, GrpcClientError>
    where
        T: rt::Read + rt::Write + Unpin + 'static,
    {
        let (http2_connection, conn) = {
            let client = hyper::client::conn::http2::Builder::new(executor.clone())
                .initial_stream_window_size(4096)
                .initial_connection_window_size(4096)
                .max_concurrent_reset_streams(2)
                .max_send_buf_size(4096)
                .keep_alive_interval(Some(std::time::Duration::from_secs(120))) // will send ping frames every 120 seconds
                .keep_alive_timeout(std::time::Duration::from_secs(300)) // if ping frame is not answered after 300 seconds the connection will be dropped
                .timer(H2Timer)
                .handshake(io)
                .await
                .unwrap();
            (client.0, Box::new(client.1))
        };

        let http2_task = executor.spawn(async {
            if let Err(e) = conn.await {
                log::error!("GrpcClient failed with {:?}", e);
            }
        });
        Ok(Self {
            executor,
            http2_connection,
            http2_task: Some(http2_task),
            uri: uri.to_string(),
        })
    }

    pub(crate) fn build_request<B: Body>(
        &self,
        path: &str,
        jwt: Option<&str>,
        rpc_host: &str,
        body: B,
    ) -> Result<Request<B>, GrpcClientError> {
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

        r.body(body).map_err(GrpcClientError::HyperHttpError)
    }

    pub(crate) async fn send_request_bidi<R, P>(
        &self,
        r: Request<BoxBody<Bytes, hyper::Error>>,
        sender: Sender<Bytes>,
    ) -> Result<(GrpcMessageSender<R>, GrpcMessageStream<P>), GrpcClientError>
    where
        R: prost::Message + std::default::Default,
        P: prost::Message + std::default::Default,
    {
        let mut http2_connection = self.http2_connection.clone();
        http2_connection.ready().await?;

        let response = http2_connection.send_request(r).await?;

        let r: GrpcMessageSender<R> = GrpcMessageSender::new(sender);

        let (part, body) = response.into_parts();

        if part.status != status::StatusCode::OK {
            return Err(GrpcClientError::HttpStatusError(part.status));
        }
        let p: GrpcMessageStream<P> = GrpcMessageStream::new(body);

        Ok((r, p))
    }

    pub(crate) async fn send_request(
        &self,
        r: Request<BoxBody<Bytes, hyper::Error>>,
    ) -> Result<(Bytes, HeaderMap), GrpcClientError> {
        let mut http2_connection = self.http2_connection.clone();
        // verify if the server can accept a new HTTP2 stream
        http2_connection.ready().await?;

        // send the header and let the server know more data are coming
        let response = http2_connection.send_request(r).await?;
        // send the body of the request and let the server know we have nothing else to send

        let (part, body) = response.into_parts();

        if part.status != status::StatusCode::OK {
            log::error!("received status code {}", part.status.to_string());
            return Err(GrpcClientError::HttpStatusError(part.status));
        }

        let body = body.collect().await?;

        let trailers = body.trailers();

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
        Ok((body.to_bytes(), part.headers))
    }
}

#[cfg(test)]
mod tests {
    use async_io::{Async, Timer};
    use bytes::Bytes;
    use futures_lite::stream::StreamExt;
    use futures_lite::{ready, FutureExt};
    use http_body_util::{combinators::BoxBody, BodyExt, BodyStream, StreamBody};
    use hyper::{
        body::{Body, Frame},
        header::CONTENT_TYPE,
        server::conn::http2,
        service::service_fn,
        Request, Response,
    };
    use std::{
        net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
        str::FromStr,
        task::Poll,
        time::Duration,
    };

    use crate::{
        common::app_client::encode_request,
        native::{exec::NativeExecutor, tcp::NativeStream},
        proto::rpc::examples::echo::v1::{EchoBiDiRequest, EchoBiDiResponse},
    };

    use super::GrpcClient;
    use prost::Message;

    pin_project_lite::pin_project! {
    struct BiDiEchoResp{
        timer: Option<Timer>,
        body: Option<Bytes>,
        #[pin]
        req: Request<hyper::body::Incoming>,
    }
    }

    impl Body for BiDiEchoResp {
        type Data = Bytes;
        type Error = hyper::Error;
        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
            let this = self.project();
            if let Some(&mut ref mut timer) = this.timer.as_mut() {
                let _ = ready!(timer.poll(cx));
            }
            let _ = this.timer.take();
            // simulate split body (eg multiple HTTP2 frames)
            if let Some(mut body) = this.body.take() {
                let split = body.iter().position(|k| *k == b'S');
                if let Some(idx) = split {
                    let _ = this.body.insert(body.split_off(idx + 1));
                    let _ = this.timer.insert(Timer::after(Duration::from_millis(150)));
                }
                let frame = Frame::data(body);
                return Poll::Ready(Some(Ok(frame)));
            }
            let r = ready!(this.req.poll_frame(cx));
            if r.is_none() {
                return Poll::Ready(None);
            }
            let frame = match r.unwrap() {
                Err(e) => {
                    return Poll::Ready(Some(Err(e)));
                }
                Ok(frame) => frame,
            };
            // for testing purpose we will always have a full body in  this request
            // the question for use would be whether or not we simulate splitting
            let mut data = frame.into_data().unwrap();
            // Split off the length prefixed message containing the compressed flag (B0) and the message length (B1-B4)
            let mut delim = data.split_to(5);
            // Discard compression flag
            let _ = delim.split_to(1);

            let len = u32::from_be_bytes(delim.as_ref().try_into().unwrap());

            let data = data.split_to(len as usize);

            let msg = EchoBiDiRequest::decode(data).unwrap();

            // copy the content of request in response (echo)
            let msg = EchoBiDiResponse {
                message: msg.message,
            };

            let mut data = encode_request(msg).unwrap();
            let split = data.iter().position(|k| *k == b'S');
            if let Some(idx) = split {
                let _ = this.body.insert(data.split_off(idx + 1));
                let _ = this.timer.insert(Timer::after(Duration::from_millis(150)));
            }
            let frame = Frame::data(data);
            Poll::Ready(Some(Ok(frame)))
        }
    }

    async fn server_fn_echo(
        req: Request<hyper::body::Incoming>,
    ) -> Result<Response<BodyStream<BoxBody<Bytes, hyper::Error>>>, hyper::Error> {
        let r = match req.uri().path() {
            "/proto.rpc.examples.echo.v1.EchoService/EchoBiDi" => Ok::<_, hyper::Error>(
                Response::builder()
                    .status(200)
                    .header(CONTENT_TYPE, "application/grpc")
                    .body(BodyStream::new(BoxBody::new(BiDiEchoResp {
                        req,
                        timer: None,
                        body: None,
                    })))
                    .unwrap(),
            ),
            _ => panic!("unimplemented"),
        };
        r
    }

    async fn grpc_client_test_server(tcp_server: Async<TcpListener>, executor: NativeExecutor) {
        loop {
            let incoming = tcp_server.accept().await;
            assert!(incoming.is_ok());
            let incoming = incoming.unwrap();
            let service = service_fn(server_fn_echo);
            let io = NativeStream::LocalPlain(incoming.0);
            let cloned_exec = executor.clone();
            let srv = http2::Builder::new(cloned_exec).serve_connection(io, service);
            let ret = srv.await;
            assert!(ret.is_ok());
        }
    }

    async fn test_grpc_client_split_streaming_rpcs_async(port: u16, exec: NativeExecutor) {
        let _ = Timer::after(Duration::from_millis(200)).await;
        let addr = SocketAddrV4::new(Ipv4Addr::from_str("127.0.0.1").unwrap(), port);
        let tcp_client = Async::<TcpStream>::connect(addr).await;
        assert!(tcp_client.is_ok());
        let tcp_client = tcp_client.unwrap();
        let tcp_stream = NativeStream::LocalPlain(tcp_client);
        let client = GrpcClient::new(tcp_stream, exec, "http://localhost").await;
        assert!(client.is_ok());
        let client = client.unwrap();

        let (sender, receiver) = async_channel::bounded::<Bytes>(1);

        let r = client.build_request(
            "/proto.rpc.examples.echo.v1.EchoService/EchoBiDi",
            None,
            "",
            BodyExt::boxed(StreamBody::new(receiver.map(|b| Ok(Frame::data(b))))),
        );
        assert!(r.is_ok());

        let r = client
            .send_request_bidi::<EchoBiDiRequest, EchoBiDiResponse>(r.unwrap(), sender)
            .await;

        assert!(r.is_ok());
        let (mut tx, mut rx) = r.unwrap();
        let p = tx
            .send_message(EchoBiDiRequest {
                message: "12345".to_owned(),
            })
            .await;
        assert!(p.is_ok());
        let r = rx.next().await;
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap().message, "12345");

        let p = tx
            .send_message(EchoBiDiRequest {
                message: "54321".to_owned(),
            })
            .await;
        assert!(p.is_ok());
        let r = rx.next().await;
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap().message, "54321");

        let p = tx
            .send_message(EchoBiDiRequest {
                message: "54321S12345123456789012345".to_owned(),
            })
            .await;
        assert!(p.is_ok());
        let r = rx.next().await;
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap().message, "54321S12345123456789012345");

        let p = tx
            .send_message(EchoBiDiRequest {
                message: "5432112345".to_owned(),
            })
            .await;
        assert!(p.is_ok());
        let r = rx.next().await;
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap().message, "5432112345");

        let p = tx
            .send_message(EchoBiDiRequest {
                message: "54321S12345123S4567890S12345".to_owned(),
            })
            .await;
        assert!(p.is_ok());
        let r = rx.next().await;
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap().message, "54321S12345123S4567890S12345");

        let p = tx
            .send_message(EchoBiDiRequest {
                message: "hello world".to_owned(),
            })
            .await;
        assert!(p.is_ok());
        let r = rx.next().await;
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r.is_ok());
        assert_eq!(r.unwrap().message, "hello world");

        let _ = Timer::after(Duration::from_millis(200)).await;
    }

    #[test_log::test]
    fn test_grpc_client_split_streaming_rpcs() {
        let exec = NativeExecutor::default();

        let tcp_server = TcpListener::bind("127.0.0.1:0");
        assert!(tcp_server.is_ok());
        let tcp_server = tcp_server.unwrap();
        let port = tcp_server.local_addr().unwrap().port();
        let tcp_server: Async<TcpListener> = tcp_server.try_into().unwrap();
        let cloned_exec = exec.clone();
        exec.spawn(async move { grpc_client_test_server(tcp_server, cloned_exec).await })
            .detach();

        let cloned_exec = exec.clone();
        exec.block_on(async move {
            test_grpc_client_split_streaming_rpcs_async(port, cloned_exec).await
        });
    }
}
