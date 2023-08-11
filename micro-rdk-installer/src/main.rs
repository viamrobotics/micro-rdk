use std::fs::{self, File};
use std::io::{stdin, stdout, Write};

use clap::{arg, command, Args, Parser, Subcommand};
use micro_rdk_installer::nvs::data::{ViamFlashStorageData, WifiCredentials};
use micro_rdk_installer::nvs::partition::{NVSPartition, NVSPartitionData};
use micro_rdk_installer::nvs::request::populate_nvs_storage_from_app;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct AppCloudConfig {
    r#id: String,
    app_address: String,
    secret: Secret<String>,
}

#[derive(Deserialize, Debug)]
struct AppConfig {
    cloud: AppCloudConfig,
}

#[derive(Subcommand)]
enum Commands {
    WriteBinary(WriteBinary),
    WriteFlash(WriteFlash),
    CreateNvsPartition(CreateNVSPartition),
}

#[derive(Args)]
struct WriteBinary {}

#[derive(Args)]
struct WriteFlash {}

#[derive(Args)]
struct CreateNVSPartition {
    #[arg(long = "app-config")]
    config: String,
    #[arg(long = "output")]
    file_name: String,
}

#[derive(Parser)]
#[command(about = "desc goes here")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn request_wifi() -> anyhow::Result<WifiCredentials> {
    let mut ssid = String::new();
    println!("Please enter WiFi SSID: ");
    let _ = stdout().flush();
    stdin().read_line(&mut ssid)?;
    ssid.pop();
    let mut password = String::new();
    println!("Please enter WiFi Password: ");
    let _ = stdout().flush();
    stdin().read_line(&mut password)?;
    password.pop();
    Ok(WifiCredentials { ssid, password })
}

fn create_nvs_partition_binary(config_path: String) -> anyhow::Result<Vec<u8>> {
    let mut storage_data = ViamFlashStorageData::default();
    let config_str = fs::read_to_string(config_path)?;
    let app_config: AppConfig = serde_json::from_str(&config_str)?;
    storage_data.robot_credentials.robot_id = Some(app_config.cloud.r#id.to_string());
    storage_data.robot_credentials.app_address = Some(app_config.cloud.app_address.to_string());
    storage_data.robot_credentials.robot_secret =
        Some(app_config.cloud.secret.expose_secret().to_string());
    let wifi_cred = request_wifi()?;
    storage_data.wifi = Some(wifi_cred);
    populate_nvs_storage_from_app(&mut storage_data)?;
    let part = &mut NVSPartition::try_from(storage_data)?;
    Ok(NVSPartitionData::try_from(part)?.to_bytes())
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::WriteBinary(_)) => {
            anyhow::bail!("binary write not yet supported")
        }
        Some(Commands::WriteFlash(_)) => {
            anyhow::bail!("writing to flash not yet supported")
        }
        Some(Commands::CreateNvsPartition(args)) => {
            let mut file = File::create(args.file_name.to_string())?;
            file.write_all(&create_nvs_partition_binary(args.config.to_string())?)?;
        }
        None => {
            anyhow::bail!("command required")
        }
    };
    Ok(())
}
