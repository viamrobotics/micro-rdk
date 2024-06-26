#[cfg(feature = "esp32")]
use crate::esp32::exec::Esp32Executor;
#[cfg(feature = "native")]
use crate::native::exec::NativeExecutor;

use super::app_client::{AppClient, AppClientBuilder, AppClientConfig};
use super::conn::server::TlsClientConnector;
use super::grpc_client::GrpcClient;
use super::provisioning::storage::RobotCredentials;
use super::registry::ComponentRegistry;
use super::robot::LocalRobot;

pub enum RobotRepresentation {
    WithRobot(LocalRobot),
    WithRegistry(Box<ComponentRegistry>),
}

#[cfg(feature = "native")]
type Executor = NativeExecutor;
#[cfg(feature = "esp32")]
type Executor = Esp32Executor;
pub async fn validate_robot_credentials(
    exec: Executor,
    robot_creds: &RobotCredentials,
    client_connector: &mut impl TlsClientConnector,
) -> Result<AppClient, Box<dyn std::error::Error>> {
    let app_config = AppClientConfig::new(
        robot_creds.robot_secret().to_owned(),
        robot_creds.robot_id().to_owned(),
        "".to_owned(),
    );
    let client = GrpcClient::new(client_connector.connect().await?, exec.clone(), "https://app.viam.com:443").await?;
    let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

    builder.build().await.map_err(|e| e.into())
}
