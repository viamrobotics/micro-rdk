#![allow(unused)]
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::common::{
    app_client::AppClientConfig,
    grpc::{GrpcBody, GrpcServer},
};

use super::super::common::robot::LocalRobot;
use futures_lite::future::block_on;
use hyper::server::conn::Http;
use local_ip_address::local_ip;
use log::logger;
use mdns_sd::{ServiceDaemon, ServiceInfo};

use super::{
    exec::NativeExecutor,
    tcp::NativeListener,
    tls::{NativeTls, NativeTlsServerConfig},
};

pub struct CloudConfig<'a> {
    robot_name: &'a str,
    robot_local_fqdn: &'a str,
    robot_fqdn: &'a str,
    robot_id: &'a str,
    robot_secret: &'a str,
    robot_tls_config: Option<NativeTlsServerConfig>,
}

impl<'a> CloudConfig<'a> {
    pub fn new(
        robot_name: &'a str,
        robot_local_fqdn: &'a str,
        robot_fqdn: &'a str,
        robot_id: &'a str,
        robot_secret: &'a str,
    ) -> Self {
        CloudConfig {
            robot_name,
            robot_local_fqdn,
            robot_fqdn,
            robot_id,
            robot_secret,
            robot_tls_config: None,
        }
    }
    pub fn set_tls_config(&mut self, tls_cfg: NativeTlsServerConfig) {
        self.robot_tls_config = Some(tls_cfg)
    }
}

pub struct NativeServer<'a> {
    robot: Arc<Mutex<LocalRobot>>,
    cloud_cfg: CloudConfig<'a>,
}

impl<'a> NativeServer<'a> {
    pub fn new(robot: LocalRobot, cloud_cfg: CloudConfig<'a>) -> Self {
        NativeServer {
            robot: Arc::new(Mutex::new(robot)),
            cloud_cfg,
        }
    }
    pub fn start(&self, ip: Ipv4Addr) -> anyhow::Result<()> {
        let mut client_cfg = {
            AppClientConfig::new(
                self.cloud_cfg.robot_secret.to_owned(),
                self.cloud_cfg.robot_id.to_owned(),
                ip,
            )
        };
        let hnd = match super::entry::start(client_cfg) {
            Err(e) => {
                log::error!("couldn't start robot client {:?} will start the server", e);
                None
            }
            Ok(hnd) => Some(hnd),
        };
        let _ = hnd.unwrap().join();
        //let _mdns = md
        let mdns = ServiceDaemon::new()?;
        let mut prop = HashMap::new();
        prop.insert("grpc".to_string(), "".to_string());
        let srv = ServiceInfo::new(
            "_rpc._tcp.local.",
            self.cloud_cfg.robot_fqdn,
            self.cloud_cfg.robot_name,
            local_ip().unwrap().to_string(),
            12346,
            Some(prop.clone()),
        )?;
        mdns.register(srv)?;
        let srv = ServiceInfo::new(
            "_rpc._tcp.local.",
            self.cloud_cfg.robot_local_fqdn,
            self.cloud_cfg.robot_name,
            local_ip().unwrap().to_string(),
            12346,
            Some(prop.clone()),
        )?;
        mdns.register(srv)?;
        if let Err(e) = self.runserver() {
            log::error!("robot server failed with error {:?}", e);
            return Err(e);
        }
        Ok(())
    }
    fn runserver(&self) -> anyhow::Result<()> {
        let tls_cfg = match &self.cloud_cfg.robot_tls_config {
            Some(tls_cfg) => tls_cfg,
            None => return Err(anyhow::anyhow!("no tls configuration supplied")),
        };
        let tls = Box::new(NativeTls::new_server(tls_cfg.clone()));
        let address: SocketAddr = "0.0.0.0:12346".parse().unwrap();
        let mut listener = NativeListener::new(address.into(), Some(tls))?;
        let exec = NativeExecutor::new();
        let srv = GrpcServer::new(self.robot.clone(), GrpcBody::new());
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
