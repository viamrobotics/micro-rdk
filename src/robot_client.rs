use crate::{
    exec::Esp32Executor,
    proto::{
        app::v1::{AgentInfo, ConfigRequest, ConfigResponse},
        rpc::v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
    },
    tcp::Esp32Stream,
    tls::Esp32tls,
};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use esp_idf_sys::{vTaskDelay, vTaskDelete, xTaskCreatePinnedToCore};
use futures_lite::future::block_on;
use h2::client::{handshake, SendRequest};
use hyper::{Method, Request};
use prost::Message;
use smol::Task;
use std::ffi::c_void;

/// Robot client to interface with app.viam.com
struct RobotClient<'a> {
    /// a local executor to spawn future
    exec: Esp32Executor<'a>,
    /// an HTTP2 stream to a server
    h2: SendRequest<Bytes>,
    /// an connection to a server
    #[allow(dead_code)]
    http2_connection: Task<()>,
    /// a jwt string for further grpc requests
    jwt: Option<String>,
}

// Generated robot config during build process
include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

static CLIENT_TASK: &[u8] = b"client\0";

impl<'a> RobotClient<'a> {
    /// Create a new robot client
    fn new(exec: Esp32Executor<'a>, h2: SendRequest<Bytes>, http2_connection: Task<()>) -> Self {
        RobotClient {
            exec,
            h2,
            http2_connection,
            jwt: None,
        }
    }

    /// Make a request to app.viam.com
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

    /// read the robot config from the cloud
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

            buf.put_u8(0);
            buf.put_u32(req.encoded_len().try_into()?);

            let mut msg = buf.split_off(5);
            req.encode(&mut msg)?;
            buf.unsplit(msg);
            buf.into()
        };

        let mut r = self.send_request(r, body)?;
        let r = r.split_off(5);
        // for now we only read the config
        let _r = ConfigResponse::decode(r)?;
        log::info!("cfg {:?}", _r);

        Ok(())
    }

    /// get a JWT token from app.viam.com
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

            buf.put_u8(0);
            buf.put_u32(req.encoded_len().try_into()?);

            let mut msg = buf.split_off(5);
            req.encode(&mut msg)?;
            buf.unsplit(msg);

            buf.into()
        };

        let mut r = self.send_request(r, body)?;
        let r = r.split_off(5);
        let r = AuthenticateResponse::decode(r)?;

        self.jwt = Some(format!("Bearer {}", r.access_token));

        Ok(())
    }

    /// send a grpc request
    fn send_request(&mut self, r: Request<()>, body: Bytes) -> Result<Bytes> {
        let h2 = self.h2.clone();
        // verify if the server can accept a new HTTP2 stream
        let mut h2 = block_on(self.exec.run(async { h2.ready().await }))?;

        // send the header and let the server know more data are coming
        let (response, mut send) = h2.send_request(r, false)?;
        // send the body of the request and let the server know we have nothing else to send
        send.send_data(body, true)?;

        let (part, mut body) = block_on(self.exec.run(async { response.await }))?.into_parts();
        log::info!("parts received {:?}", part);

        let mut response_buf = BytesMut::with_capacity(1024);
        // TODO read the first 5 bytes so we know how much data to expect and we can allocate appropriately
        while let Some(chunk) = block_on(self.exec.run(async { body.data().await })) {
            let chunk = chunk?;
            response_buf.put_slice(&chunk);
            let _ = body.flow_control().release_capacity(chunk.len());
        }

        let _ = block_on(self.exec.run(async { body.trailers().await }));

        self.h2 = h2;

        Ok(response_buf.into())
    }
}

/// start the robot client
pub(crate) fn start() -> Result<()> {
    log::info!("starting up robot client");
    let ret = unsafe {
        xTaskCreatePinnedToCore(
            Some(client_entry),                      // C ABI compatible entry function
            CLIENT_TASK.as_ptr() as *const i8, // task name
            8192 * 4,                          // stack size
            std::ptr::null_mut(),              // no arguments
            20,                                // priority (low)
            std::ptr::null_mut(),              // we don't store the handle
            0,                                 // run it on core 0
        )
    };
    if ret != 1 {
        return Err(anyhow::anyhow!("wasn't able to create the client task"));
    }
    Ok(())
}

/// client main loop
fn clientloop() -> Result<()> {
    let mut tls = Box::new(Esp32tls::new(false));
    let conn = tls.open_ssl_context(None)?;
    let conn = Esp32Stream::TLSStream(Box::new(conn));
    let executor = Esp32Executor::new();

    let (h2, conn) = block_on(executor.run(async { handshake(conn).await }))?;
    let task = executor.spawn(async move {
        conn.await.unwrap();
    });

    let mut robot_client = RobotClient::new(executor, h2, task);

    robot_client.request_jwt_token()?;

    loop {
        robot_client.read_config()?;
        unsafe {
            vTaskDelay(10000);
        }
    }
}

/// C compatible entry function
extern "C" fn client_entry(_: *mut c_void) {
    if let Some(err) = clientloop().err() {
        log::error!("client returned with error {}", err);
    }
    unsafe {
        vTaskDelete(std::ptr::null_mut());
    }
}
