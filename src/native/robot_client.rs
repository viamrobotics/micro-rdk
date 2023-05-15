#![allow(dead_code)]
use crate::{
    common::{grpc_client::GrpcClient, webrtc::api::WebRtcApi},
    native::exec::NativeExecutor,
    native::tcp::NativeStream,
    native::{certificate::WebRtcCertificate, tls::NativeTls},
    proto::{
        app::v1::{AgentInfo, ConfigRequest, ConfigResponse},
        rpc::{
            v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
            webrtc::v1::{AnswerRequest, AnswerResponse},
        },
    },
};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};

use futures_lite::future::block_on;
use prost::Message;

use std::{
    net::Ipv4Addr,
    rc::Rc,
    thread::{self, JoinHandle},
};

use super::dtls::Dtls;

/// Robot client to interface with app.viam.com
struct RobotClient<'a> {
    /// a local executor to spawn future
    grpc_client: GrpcClient<'a>,
    /// an HTTP2 stream to a server
    jwt: Option<String>,
    config: &'a RobotClientConfig,
}

pub struct RobotClientConfig {
    robot_secret: String,
    robot_id: String,
    ip: Ipv4Addr,
    pub robot_fqdn: &'static str,
}

impl RobotClientConfig {
    pub fn new(
        robot_secret: String,
        robot_id: String,
        ip: Ipv4Addr,
        robot_fqdn: &'static str,
    ) -> Self {
        RobotClientConfig {
            robot_secret,
            robot_id,
            ip,
            robot_fqdn,
        }
    }
}

static CLIENT_TASK: &[u8] = b"client\0";

impl<'a> RobotClient<'a> {
    /// Create a new robot client
    fn new(grpc_client: GrpcClient<'a>, config: &'a RobotClientConfig) -> Self {
        RobotClient {
            grpc_client,
            jwt: None,
            config,
        }
    }

    /// Make a request to app.viam.com

    /// read the robot config from the cloud
    fn read_config(&mut self) -> Result<()> {
        let r = self
            .grpc_client
            .build_request("/viam.app.v1.RobotService/Config", &self.jwt)?;

        let agent = AgentInfo {
            os: "esp32-native".to_string(),
            host: "esp32-native".to_string(),
            ips: vec![self.config.ip.to_string()],
            version: "0.0.2".to_string(),
            git_revision: "".to_string(),
        };

        let req = ConfigRequest {
            agent_info: Some(agent),
            id: self.config.robot_id.clone(),
        };

        let body: Bytes = {
            let mut buf = BytesMut::with_capacity(req.encoded_len() + 5);

            buf.put_u8(0);
            buf.put_u32(req.encoded_len().try_into()?);

            let mut msg = buf.split_off(5);
            req.encode(&mut msg)?;
            buf.unsplit(msg);
            buf.into()
        };

        let mut r = self.grpc_client.send_request(r, body)?;
        let r = r.split_off(5);
        // for now we only read the config
        let _r = ConfigResponse::decode(r)?;
        log::info!("cfg {:?}", _r);

        Ok(())
    }

    /// get a JWT token from app.viam.com
    fn request_jwt_token(&mut self) -> Result<()> {
        let r = self
            .grpc_client
            .build_request("/proto.rpc.v1.AuthService/Authenticate", &None)?;
        let body: Bytes = {
            let cred = Credentials {
                r#type: "robot-secret".to_string(),
                payload: self.config.robot_secret.clone(),
            };

            let req = AuthenticateRequest {
                entity: self.config.robot_id.clone(),
                credentials: Some(cred),
            };

            let mut buf = BytesMut::with_capacity(req.encoded_len() + 5);

            buf.put_u8(0);
            buf.put_u32(req.encoded_len().try_into()?);

            let mut msg = buf.split_off(5);
            req.encode(&mut msg)?;
            buf.unsplit(msg);

            buf.into()
        };
        let mut r = self.grpc_client.send_request(r, body)?;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r)?;
        log::info!("has tocken {:?}", &r.access_token);
        self.jwt = Some(format!("Bearer {}", r.access_token));

        Ok(())
    }
    fn test_echo_request(&mut self) -> Result<()> {
        Ok(())
    }
    fn start_answering_signaling<'b>(
        &mut self,
        executor: NativeExecutor<'b>,
    ) -> Result<WebRtcApi<'b, WebRtcCertificate, Dtls>> {
        let r = self
            .grpc_client
            .build_request("/proto.rpc.webrtc.v1.SignalingService/Answer", &self.jwt)?;
        log::info!("Spawning signaling");
        let (tx_half, rx_half) = self
            .grpc_client
            .send_request_bidi::<AnswerResponse, AnswerRequest>(r, None)?;
        let cloned_exec = executor.clone();
        let our_ip = match local_ip_address::local_ip()? {
            std::net::IpAddr::V4(v4) => v4,
            _ => {
                return Err(anyhow::anyhow!("our_ip is not an IpV4Addr"));
            }
        };
        let certificate = WebRtcCertificate::new().unwrap();
        //let dtls_t = self.transport.get_dtls_channel().unwrap();
        let dtls = Dtls::new(Rc::new(certificate.clone())).unwrap();
        let mut webrtc = WebRtcApi::new(
            executor,
            tx_half,
            rx_half,
            Rc::new(certificate),
            our_ip,
            dtls,
        );
        let answer = block_on(cloned_exec.run(async { webrtc.answer().await }));
        log::info!("answer {:?}", answer);
        let connected = block_on(cloned_exec.run(async { webrtc.run_ice_until_connected().await }));
        log::info!("connected {:?}", connected);

        Ok(webrtc)
    }
}

/// start the robot client
pub fn start(ip: RobotClientConfig) -> Result<JoinHandle<()>> {
    let handle = thread::spawn(|| client_entry(ip));
    Ok(handle)
}

/// client main loop
fn clientloop(config: &RobotClientConfig) -> Result<()> {
    let tls = Box::new(NativeTls::new_client());
    let conn = tls.open_ssl_context(None)?;
    let conn = NativeStream::TLSStream(Box::new(conn));
    let executor = NativeExecutor::new();

    let fqdn_split: Vec<&str> = config.robot_fqdn.rsplitn(4, '-').collect();
    let fqdn: String = fqdn_split
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<&str>>()
        .join(".");

    let grpc_client = GrpcClient::new(conn, executor, "https://app.viam.com:443", fqdn)?;

    let mut robot_client = RobotClient::new(grpc_client, config);

    robot_client.request_jwt_token()?;
    robot_client.read_config()?;
    Ok(())
}

fn client_entry(config: RobotClientConfig) {
    if let Some(err) = clientloop(&config).err() {
        log::error!("client returned with error {}", err);
    }
}

#[cfg(test)]
mod tests {
    use crate::common::board::FakeBoard;
    use crate::common::grpc::GrpcServer;
    use crate::common::robot::{LocalRobot, ResourceMap, ResourceType};
    use crate::common::webrtc::grpc::{WebRtcGrpcBody, WebRtcGrpcServer};
    use crate::native::exec::NativeExecutor;
    use crate::native::robot_client::GrpcClient;
    use crate::native::tcp::NativeStream;
    use crate::native::tls::NativeTls;

    use crate::proto::common::v1::ResourceName;
    use crate::proto::rpc::examples::echo::v1::{EchoBiDiRequest, EchoBiDiResponse};

    use futures_lite::future::block_on;
    use futures_lite::StreamExt;

    use std::collections::HashMap;
    use std::net::{Ipv4Addr, TcpStream};

    use std::sync::{Arc, Mutex};

    use super::{RobotClient, RobotClientConfig};
    #[test_log::test]
    #[ignore]
    fn test_client_bidi() -> anyhow::Result<()> {
        let socket = TcpStream::connect("127.0.0.1:8080").unwrap();
        socket.set_nonblocking(true)?;
        let conn = NativeStream::LocalPlain(socket);
        let executor = NativeExecutor::new();
        let mut grpc_client = GrpcClient::new(
            conn,
            executor.clone(),
            "http://localhost",
            "some_fqdn".to_owned(),
        )?;

        let r =
            grpc_client.build_request("/proto.rpc.examples.echo.v1.EchoService/EchoBiDi", &None)?;

        let (mut sender_half, mut recv_half) = grpc_client
            .send_request_bidi::<EchoBiDiRequest, EchoBiDiResponse>(
                r,
                Some(EchoBiDiRequest {
                    message: "1".to_string(),
                }),
            )?;

        let (p, mut recv_half) = block_on(executor.run(async {
            let p = recv_half.next().await.unwrap().message;
            (p, recv_half)
        }));
        assert_eq!("1", p);

        let recv_half_ref = recv_half.by_ref();

        sender_half.send_message(EchoBiDiRequest {
            message: "hello".to_string(),
        })?;

        let p = block_on(executor.run(async {
            recv_half_ref
                .take(5)
                .map(|m| m.message)
                .collect::<String>()
                .await
        }));

        assert_eq!("hello", p);

        sender_half.send_message(EchoBiDiRequest {
            message: "123456".to_string(),
        })?;
        let p = block_on(executor.run(async {
            recv_half_ref
                .take(6)
                .map(|m| m.message)
                .collect::<String>()
                .await
        }));

        assert_eq!("123456", p);
        Ok(())
    }

    #[test_log::test]
    #[ignore]
    fn test_webrtc_signaling() -> anyhow::Result<()> {
        let cfg = RobotClientConfig {
            robot_secret: "<Some secret>".to_string(),
            robot_id: "<Some robot>".to_string(),
            ip: Ipv4Addr::new(0, 0, 0, 0),
            robot_fqdn: "some_fqdn",
        };

        let robot = {
            let board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
            let mut res: ResourceMap = HashMap::with_capacity(1);
            res.insert(
                ResourceName {
                    namespace: "rdk".to_string(),
                    r#type: "component".to_string(),
                    subtype: "board".to_string(),
                    name: "b".to_string(),
                },
                ResourceType::Board(board),
            );
            Arc::new(Mutex::new(LocalRobot::new(res)))
        };

        let executor = NativeExecutor::new();

        let mut webrtc = {
            let tls = Box::new(NativeTls::new_client());
            let conn = tls.open_ssl_context(None)?;
            let conn = NativeStream::TLSStream(Box::new(conn));
            let grpc_client = GrpcClient::new(
                conn,
                executor.clone(),
                "https://app.viam.com:443",
                "some_fqdn".to_owned(),
            )?;
            let mut robot_client = RobotClient::new(grpc_client, &cfg);

            robot_client.request_jwt_token()?;
            robot_client.read_config()?;

            let p = robot_client
                .start_answering_signaling(executor.clone())
                .unwrap();

            drop(robot_client);
            p
        };
        let channel = block_on(executor.run(async { webrtc.open_data_channel().await })).unwrap();
        log::info!("channel opened {:?}", channel);

        let mut webrtc_grpc =
            WebRtcGrpcServer::new(channel, GrpcServer::new(robot, WebRtcGrpcBody::default()));

        loop {
            block_on(executor.run(async { webrtc_grpc.next_request().await.unwrap() }));
        }
    }
}
