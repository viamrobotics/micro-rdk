use super::app_client::{ AppClient, AppClientError, PeriodicAppClientTask};
use futures_lite::Future;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::time::Duration;
use crc32fast::Hasher;
use crate::proto::app::v1::ConfigResponse;
use crate::common::app_client::encode_request;

pub struct ConfigMonitor<'a> {
    restart_hook: Option<Box<dyn FnOnce() + 'a>>,
    curr_config: ConfigResponse, //config for robot gotten from last robot startup, aka inputted from entry
    ip: Option<Ipv4Addr>,
}

impl<'a> ConfigMonitor<'a> {
    pub fn new(restart_hook: impl FnOnce() + 'a, curr_config: ConfigResponse, ip: Option<Ipv4Addr>) -> Self {
        Self {
            restart_hook: Some(Box::new(restart_hook)),
            curr_config: curr_config,
            ip: ip,
        }
    }

    fn restart(&mut self) -> ! {
        log::warn!("Config change detected - restarting or terminating now...");
        (self.restart_hook.take().unwrap())();
        unreachable!();
    }
}

impl<'a> PeriodicAppClientTask for ConfigMonitor<'a> {
    fn name(&self) -> &str {
        "ConfigMonitor"
    }

    fn get_default_period(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn invoke<'c, 'b: 'c>(
        &'b mut self,
        app_client: &'c AppClient,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Duration>, AppClientError>> + 'c>> {
        Box::pin(async move {
            let app_config = get_app_config(app_client).await.unwrap();
            match self.curr_config == app_config{
                true => self.restart(),
                false => Ok(Some(self.get_default_period())),
            }
        })
    }

}

async fn get_app_config(client : &AppClient) -> Result<ConfigResponse, AppClientError>{
    
    let (new_config, _cfg_received_datetime) = 
            client.get_config(None).await.unwrap();

    return Ok(*new_config);
    
}

