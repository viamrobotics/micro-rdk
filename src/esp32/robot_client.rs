#![allow(dead_code)]
use crate::{
    common::grpc_client::GrpcClient,
    esp32::exec::Esp32Executor,
    esp32::tcp::Esp32Stream,
    esp32::tls::Esp32Tls,
    proto::{
        app::v1::{AgentInfo, ConfigRequest, ConfigResponse},
        rpc::v1::{AuthenticateRequest, AuthenticateResponse, Credentials},
    },
};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use esp_idf_hal::task::{notify, wait_notification};
use esp_idf_sys::{vTaskDelete, xTaskCreatePinnedToCore, xTaskGetCurrentTaskHandle, TaskHandle_t};
use prost::Message;
use std::{ffi::c_void, net::Ipv4Addr, time::Duration};

/// Robot client to interface with app.viam.com
struct RobotClient<'a> {
    grpc_client: GrpcClient<'a>,
    /// a jwt string for further grpc requests
    jwt: Option<String>,
    config: &'a Box<RobotClientConfig>,
}

pub struct RobotClientConfig {
    robot_secret: String,
    robot_id: String,
    ip: Ipv4Addr,
    main_handle: Option<TaskHandle_t>,
}

impl RobotClientConfig {
    pub fn new(robot_secret: String, robot_id: String, ip: Ipv4Addr) -> Self {
        RobotClientConfig {
            robot_secret,
            robot_id,
            ip,
            main_handle: None,
        }
    }
    pub fn set_main_handle(&mut self, hnd: TaskHandle_t) {
        self.main_handle = Some(hnd)
    }
}

static CLIENT_TASK: &[u8] = b"client\0";

impl<'a> RobotClient<'a> {
    /// Create a new robot client
    fn new(grpc_client: GrpcClient<'a>, config: &'a Box<RobotClientConfig>) -> Self {
        RobotClient {
            grpc_client,
            jwt: None,
            config,
        }
    }
    /// read the robot config from the cloud
    fn read_config(&mut self) -> Result<()> {
        let r = self
            .grpc_client
            .build_request("/viam.app.v1.RobotService/Config", &self.jwt)?;

        let agent = AgentInfo {
            os: "esp32".to_string(),
            host: "esp32".to_string(),
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
}

/// start the robot client
pub fn start(ip: RobotClientConfig) -> Result<TaskHandle_t> {
    log::info!("starting up robot client");
    let ip = Box::into_raw(Box::new(ip));
    let mut hnd: TaskHandle_t = std::ptr::null_mut();
    let ret = unsafe {
        xTaskCreatePinnedToCore(
            Some(client_entry),                // C ABI compatible entry function
            CLIENT_TASK.as_ptr() as *const i8, // task name
            8192 * 3,                          // stack size
            ip as *mut c_void,                 // pass ip as argument
            20,                                // priority (low)
            &mut hnd,                          // we don't store the handle
            0,                                 // run it on core 0
        )
    };
    if ret != 1 {
        return Err(anyhow::anyhow!("wasn't able to create the client task"));
    }
    log::error!("got handle {:?}", hnd);
    Ok(hnd)
}

/// client main loop
fn clientloop(config: &Box<RobotClientConfig>) -> Result<()> {
    let mut tls = Box::new(Esp32Tls::new_client());
    let conn = tls.open_ssl_context(None)?;
    let conn = Esp32Stream::TLSStream(Box::new(conn));
    let executor = Esp32Executor::new();

    let grpc_client = GrpcClient::new(conn, executor, "https://app.viam.com:443")?;

    let mut robot_client = RobotClient::new(grpc_client, config);

    robot_client.request_jwt_token()?;
    robot_client.read_config()?;

    if config.main_handle.is_none() {
        loop {
            if let Some(_r) = wait_notification(Some(Duration::from_secs(30))) {
                log::info!("connection incomming the client task will stop");
                break;
            }
        }
    }
    log::error!("current task handle {:?}", unsafe {
        xTaskGetCurrentTaskHandle()
    });
    Ok(())
}

/// C compatible entry function
extern "C" fn client_entry(config: *mut c_void) {
    let config: Box<RobotClientConfig> = unsafe { Box::from_raw(config as *mut RobotClientConfig) };
    if let Some(err) = clientloop(&config).err() {
        log::error!("client returned with error {}", err);
    }
    if let Some(hnd) = config.main_handle {
        unsafe {
            let _ = notify(hnd, 0);
        }
    }
    unsafe {
        vTaskDelete(std::ptr::null_mut());
    }
}
