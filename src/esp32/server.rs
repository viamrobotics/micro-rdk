#![allow(unused)]
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use esp_idf_hal::task::{notify, wait_notification};
use esp_idf_svc::mdns::EspMdns;
use esp_idf_sys::{vTaskDelay, xTaskGetCurrentTaskHandle, TaskHandle_t};
use futures_lite::future::block_on;
use hyper::server::conn::Http;

use super::{
    exec::Esp32Executor,
    grpc::GrpcServer,
    robot::Esp32Robot,
    robot_client::RobotClientConfig,
    tcp::Esp32Listener,
    tls::{Esp32Tls, Esp32TlsServerConfig},
};

pub struct CloudConfig<'a> {
    robot_name: &'a str,
    robot_fqdn: &'a str,
    robot_id: &'a str,
    robot_secret: &'a str,
    robot_tls_config: Option<Esp32TlsServerConfig>,
}

impl<'a> CloudConfig<'a> {
    pub fn new(
        robot_name: &'a str,
        robot_fqdn: &'a str,
        robot_id: &'a str,
        robot_secret: &'a str,
    ) -> Self {
        CloudConfig {
            robot_name,
            robot_fqdn,
            robot_id,
            robot_secret,
            robot_tls_config: None,
        }
    }
    pub fn set_tls_config(&mut self, tls_cfg: Esp32TlsServerConfig) {
        self.robot_tls_config = Some(tls_cfg)
    }
}

pub struct Esp32Server<'a> {
    robot: Arc<Mutex<Esp32Robot>>,
    cloud_cfg: CloudConfig<'a>,
}

impl<'a> Esp32Server<'a> {
    pub fn new(robot: Esp32Robot, cloud_cfg: CloudConfig<'a>) -> Self {
        Esp32Server {
            robot: Arc::new(Mutex::new(robot)),
            cloud_cfg,
        }
    }
    pub fn start(&self, ip: Ipv4Addr) -> anyhow::Result<()> {
        let mut client_cfg = {
            RobotClientConfig::new(
                self.cloud_cfg.robot_secret.to_owned(),
                self.cloud_cfg.robot_id.to_owned(),
                ip,
            )
        };
        client_cfg.set_main_handle(unsafe { xTaskGetCurrentTaskHandle() });
        let hnd = match super::robot_client::start(client_cfg) {
            Err(e) => {
                log::error!("couldn't start robot client {:?} will start the server", e);
                None
            }
            Ok(hnd) => Some(hnd),
        };
        let _ = wait_notification(Some(Duration::from_secs(30)));
        let _mdns = {
            let mut mdns = EspMdns::take()?;
            mdns.set_hostname(self.cloud_cfg.robot_name)?;
            mdns.set_instance_name(self.cloud_cfg.robot_name)?;
            mdns.add_service(None, "_rpc", "_tcp", 80, &[])?;
            mdns
        };
        if let Err(e) = self.runserver(None) {
            log::error!("robot server failed with error {:?}", e);
            return Err(e);
        }
        Ok(())
    }
    fn runserver(&self, client_handle: Option<TaskHandle_t>) -> anyhow::Result<()> {
        let tls_cfg = match &self.cloud_cfg.robot_tls_config {
            Some(tls_cfg) => tls_cfg,
            None => return Err(anyhow::anyhow!("no tls configuration supplied")),
        };
        let tls = Box::new(Esp32Tls::new_server(tls_cfg));
        let address: SocketAddr = "0.0.0.0:80".parse().unwrap();
        let mut listener = Esp32Listener::new(address.into(), Some(tls))?;
        let exec = Esp32Executor::new();
        let srv = GrpcServer::new(self.robot.clone());
        if let Some(hnd) = client_handle {
            if unsafe { notify(hnd, 1) } {
                log::info!("successfully notified client task");
                unsafe {
                    vTaskDelay(1000);
                };
            } else {
                log::error!("failed to notity client task had handle {:?}", hnd);
            }
        } else {
            log::error!("no handle")
        }
        loop {
            let stream = listener.accept()?;
            block_on(exec.run(async {
                let err = Http::new()
                    .with_executor(exec.clone())
                    .http2_max_concurrent_streams(1)
                    .serve_connection(stream, srv.clone())
                    .await;
                if err.is_err() {
                    log::error!("server error {}", err.err().unwrap());
                }
            }));
        }
    }
}
