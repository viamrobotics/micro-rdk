use std::ffi::c_void;

use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use esp_idf_sys::{vTaskDelay, vTaskDelete, xTaskCreatePinnedToCore};
use futures_lite::future::block_on;
use h2::client::{handshake, SendRequest};
use hyper::{Method, Request};
use prost::Message;
use smol::Task;

use crate::{
    exec::Esp32Executor,
    proto::{
        app::v1::{AgentInfo, ConfigRequest, ConfigResponse},
        rpc::v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
    },
    tcp::Esp32Stream,
    tls::Esp32tls,
};

struct RobotClient<'a> {
    exec: Option<Esp32Executor<'a>>,
    h2: Option<SendRequest<Bytes>>,
    conn: Option<Task<()>>,
    jwt: Option<String>,
}

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

static CLIENT_TASK: &[u8] = b"client\0";

impl<'a> RobotClient<'a> {
    fn new() -> Self {
        RobotClient {
            exec: None,
            h2: None,
            conn: None,
            jwt: None,
        }
    }
    fn build_request(&self, path: &str) -> Result<Request<()>> {
        let mut uri = "https://app.viam.com:443".to_owned();
        uri.push_str(path);
        let mut r = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .header("user-agent", "esp32");
        if let Some(jwt) = &self.jwt {
            r = r.header("authorization", jwt.clone());
        };
        r.body(())
            .map_err(|e| anyhow::anyhow!("cannot build request {}", e))
    }
    fn read_config(&mut self) -> Result<()> {
        let r = self.build_request("/viam.app.v1.RobotService/Config")?;
        let agent = AgentInfo {
            os: "esp32".to_string(),
            host: "esp32".to_string(),
            ips: vec!["0.0.0.0".to_string()],
            version: "0.0.2".to_string(),
            git_revision: "".to_string(),
        };
        let req = ConfigRequest {
            agent_info: Some(agent),
            id: ROBOT_ID.to_string(),
        };
        let body: Bytes = {
            let mut buf = BytesMut::with_capacity(req.encoded_len() + 5);
            let len = req.encoded_len().to_be_bytes();
            buf.put_u8(0);
            buf.put_u32(0.try_into().unwrap());
            buf[0] = 0;
            buf[1] = len[0];
            buf[2] = len[1];
            buf[3] = len[2];
            buf[4] = len[3];
            let mut msg = buf.split_off(5);
            req.encode(&mut msg).unwrap();
            buf.unsplit(msg);
            buf.into()
        };
        let mut r = self.send_request(r, body)?;
        let r = r.split_off(5);
        let r = ConfigResponse::decode(r)?;
        log::info!("config {:?}", r);
        Ok(())
    }
    fn request_jwt_token(&mut self) -> Result<()> {
        let r = self.build_request("/proto.rpc.v1.AuthService/Authenticate")?;
        let body: Bytes = {
            let cred = Credentials {
                r#type: "robot-secret".to_string(),
                payload: ROBOT_SECRET.to_string(),
            };
            let req = AuthenticateRequest {
                entity: ROBOT_ID.to_string(),
                credentials: Some(cred),
            };
            let mut buf = BytesMut::with_capacity(req.encoded_len() + 5);
            let len = req.encoded_len().to_be_bytes();
            buf.put_u8(0);
            buf.put_u32(0.try_into().unwrap());
            buf[0] = 0;
            buf[1] = len[0];
            buf[2] = len[1];
            buf[3] = len[2];
            buf[4] = len[3];
            log::info!("Buf {} {} {} {} {}", buf[0], buf[1], buf[2], buf[3], buf[4]);
            let mut msg = buf.split_off(5);
            req.encode(&mut msg).unwrap();
            buf.unsplit(msg);
            log::info!(
                "Buf {} {} {} {} {}",
                buf.len(),
                buf[1],
                buf[2],
                buf[3],
                buf[4]
            );
            buf.into()
        };
        let mut r = self.send_request(r, body)?;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r)?;
        self.jwt = Some(format!("Bearer {}", r.access_token));
        Ok(())
    }
    fn send_request(&mut self, r: Request<()>, body: Bytes) -> Result<Bytes> {
        if self.h2.is_none() {
            anyhow::bail!("no h2 stream available need to kill the connection");
        }
        let h2 = self.h2.take().unwrap();
        let mut h2 = block_on(self.exec.as_ref().unwrap().run(async { h2.ready().await }))?;
        let (rsp, mut send) = h2.send_request(r, false)?;
        send.send_data(body, true)?;
        let (p, mut body) =
            block_on(self.exec.as_ref().unwrap().run(async { rsp.await }))?.into_parts();
        log::info!("Header received {:?}", p);
        let mut rsp = BytesMut::with_capacity(1024);
        let mut flow_control = body.flow_control().clone();
        while let Some(chunk) =
            block_on(self.exec.as_ref().unwrap().run(async { body.data().await }))
        {
            let chunk = chunk?;
            rsp.put_slice(&chunk);
            let _ = flow_control.release_capacity(chunk.len());
        }
        let trailers = block_on(
            self.exec
                .as_ref()
                .unwrap()
                .run(async { body.trailers().await }),
        );
        log::info!("tariler received {:?}", trailers);
        self.h2 = Some(h2);
        Ok(rsp.into())
    }
}

pub(crate) fn start() -> Result<()> {
    log::info!("starting up robot client");
    let ret = unsafe {
        xTaskCreatePinnedToCore(
            Some(client),
            CLIENT_TASK.as_ptr() as *const i8,
            8192 * 4,
            std::ptr::null_mut(),
            20,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret != 1 {
        return Err(anyhow::anyhow!("wasn't able to create the client task"));
    }
    Ok(())
}

fn clientloop() -> Result<()> {
    let mut robot_client = RobotClient::new();
    robot_client.exec = Some(Esp32Executor::new());
    let mut tls = Box::new(Esp32tls::new(false));
    let conn = tls.open_ssl_context(None)?;
    let conn = Esp32Stream::TLSStream(Box::new(conn));

    //let tcp_stream = TcpStream::connect("10.0.2.2:12345")?;
    //tcp_stream.set_nonblocking(true);
    //let conn = Esp32Stream::LocalPlain(tcp_stream);

    let (h2, conn) = block_on(
        robot_client
            .exec
            .as_ref()
            .unwrap()
            .run(async { handshake(conn).await }),
    )?;

    let task = robot_client.exec.as_ref().unwrap().spawn(async move {
        conn.await.unwrap();
    });

    robot_client.conn = Some(task);
    robot_client.h2 = Some(h2);
    robot_client.request_jwt_token()?;
    loop {
        robot_client.read_config()?;
        unsafe {
            vTaskDelay(10000);
        }
    }
}

extern "C" fn client(_: *mut c_void) {
    unsafe {
        if let Some(err) = clientloop().err() {
            log::error!("client returned with error {}", err);
        }
        vTaskDelete(std::ptr::null_mut());
    }
}
