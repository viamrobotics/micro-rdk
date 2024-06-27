use super::app_client::{ AppClient, AppClientError, PeriodicAppClientTask};
use futures_lite::Future;
use std::pin::Pin;
use std::time::Duration;
use crc32fast::Hasher;
use crate::proto::app::v1::ConfigResponse;
use crate::common::app_client::encode_request;

pub struct ConfigMonitor<'a> {
    restart_hook: Option<Box<dyn FnOnce() + 'a>>,
    curr_config: ConfigResponse, //config for robot gotten from last robot startup, aka inputted from entry
}

impl<'a> ConfigMonitor<'a> {
    pub fn new(restart_hook: impl FnOnce() + 'a, curr_config: ConfigResponse) -> Self {
        Self {
            restart_hook: Some(Box::new(restart_hook)),
            curr_config: curr_config
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
            match check_configs( self.curr_config.clone() , app_client ).await {
                Ok(None) => self.restart(),
                other => other,
            }
        })
    }

}

async fn check_configs(curr_config: ConfigResponse,client : &AppClient) -> Result<Option<Duration>, AppClientError>{
    let (new_config, _cfg_received_datetime) = 
            client.get_config(None).await.unwrap();    
    let curr_config = encode_request(curr_config.clone())?;
    let mut hasher = Hasher::new_with_initial(0xffff_ffff); //diff types?
    hasher.update(curr_config.as_ref());
    let hashed_curr = hasher.finalize();


    let mut hasher = Hasher::new_with_initial(0xffff_ffff); //diff types?
    let new_config = encode_request(*new_config)?;
    hasher.update(new_config.as_ref());
    let hashed_new = hasher.finalize();
    match hashed_curr == hashed_new {
        true => Ok(Some(Duration::from_secs(10))),
        false => Ok(None)
    }
    
}

