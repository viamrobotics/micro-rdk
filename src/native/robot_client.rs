#![allow(dead_code)]
use crate::{
    common::grpc_client::GrpcClient,
    native::exec::NativeExecutor,
    native::tcp::NativeStream,
    native::tls::NativeTls,
    proto::{
        app::v1::{AgentInfo, ConfigRequest, ConfigResponse},
        rpc::v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
    },
};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use prost::Message;
use std::{
    net::Ipv4Addr,
    thread::{self, JoinHandle},
};

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
}

impl RobotClientConfig {
    pub fn new(robot_secret: String, robot_id: String, ip: Ipv4Addr) -> Self {
        RobotClientConfig {
            robot_secret,
            robot_id,
            ip,
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
        self.jwt = Some(format!("Bearer {}", r.access_token));

        Ok(())
    }
    fn test_echo_request(&mut self) -> Result<()> {
        Ok(())
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

    let grpc_client = GrpcClient::new(conn, executor, "https://app.viam.com:443")?;

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
    use crate::native::exec::NativeExecutor;
    use crate::native::robot_client::GrpcClient;
    use crate::native::tcp::NativeStream;
    use crate::proto::rpc::examples::echo::v1::{EchoBiDiRequest, EchoBiDiResponse};
    use futures_lite::future::block_on;
    use futures_lite::StreamExt;
    use std::net::TcpStream;
    #[test_log::test]
    fn test_client_bidi() -> anyhow::Result<()> {
        let socket = TcpStream::connect("127.0.0.1:8080").unwrap();
        socket.set_nonblocking(true)?;
        let conn = NativeStream::LocalPlain(socket);
        let executor = NativeExecutor::new();
        let mut grpc_client = GrpcClient::new(conn, executor.clone(), "http://localhost")?;

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
}
