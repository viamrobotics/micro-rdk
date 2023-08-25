use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};

use clap::{arg, command, Args, Parser, Subcommand};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password};
use micro_rdk_installer::error::Error;
use micro_rdk_installer::nvs::data::{ViamFlashStorageData, WifiCredentials};
use micro_rdk_installer::nvs::partition::{NVSPartition, NVSPartitionData};
use micro_rdk_installer::nvs::request::populate_nvs_storage_from_app;
use secrecy::Secret;
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
    WriteCredentials(WriteCredentials),
    WriteFlash(WriteFlash),
    CreateNvsPartition(CreateNVSPartition),
}

#[derive(Args)]
struct WriteCredentials {
    #[arg(long = "app-config")]
    config: String,
    #[arg(long = "bin")]
    binary_path: String,
    #[arg(long = "nvs-size", default_value = "32768")]
    size: usize,
    // the default here represents the offset as declared in
    // examples/esp32/partitions.csv (0x9000, here written as 36864),
    // as that is the partition table that will be used to compile
    // the default application binary
    #[arg(long = "nvs-offset-address", default_value = "36864")]
    nvs_offset: u64,
}

#[derive(Args)]
struct WriteFlash {}

#[derive(Args)]
struct CreateNVSPartition {
    #[arg(long = "app-config")]
    config: String,
    #[arg(long = "output")]
    file_name: String,
    #[arg(long = "size", default_value = "32768")]
    size: usize,
}

#[derive(Parser)]
#[command(
    about = "A CLI that can compile a micro-RDK binary or flash a compilation of micro-RDK directly to an ESP32 provided configuration information"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn request_wifi() -> Result<WifiCredentials, Error> {
    let ssid: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Please enter WiFi SSID")
        .interact_text()
        .map_err(Error::WifiCredentialsError)?;
    let password: Secret<String> = Secret::new(
        Password::with_theme(&ColorfulTheme::default())
            .with_prompt("Please enter WiFi Password")
            .validate_with(|input: &String| -> Result<(), Error> {
                if input.len() > 64 {
                    return Err(Error::WifiPasswordTooLongError(
                        "password length limited to 64 characters or less".to_string(),
                    ));
                }
                Ok(())
            })
            .interact()
            .map_err(Error::WifiCredentialsError)?,
    );

    Ok(WifiCredentials { ssid, password })
}

fn create_nvs_partition_binary(config_path: String, size: usize) -> Result<Vec<u8>, Error> {
    let mut storage_data = ViamFlashStorageData::default();
    let config_str = fs::read_to_string(config_path).map_err(Error::FileError)?;
    let app_config: AppConfig = serde_json::from_str(&config_str)?;
    storage_data.robot_credentials.robot_id = Some(app_config.cloud.r#id.to_string());
    storage_data.robot_credentials.app_address = Some(app_config.cloud.app_address.to_string());
    storage_data.robot_credentials.robot_secret = Some(app_config.cloud.secret);
    let wifi_cred = request_wifi()?;
    storage_data.wifi = Some(wifi_cred);
    populate_nvs_storage_from_app(&mut storage_data)?;
    let part = &mut NVSPartition::from_storage_data(storage_data, size)?;
    Ok(NVSPartitionData::try_from(part)?.to_bytes())
}

fn write_credentials_to_app_binary(
    binary_path: &str,
    nvs_data: &[u8],
    nvs_size: u64,
    nvs_start_address: u64,
) -> Result<(), Error> {
    let mut app_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(binary_path)
        .map_err(Error::FileError)?;
    let file_len = app_file.metadata().map_err(Error::FileError)?.len();
    if (file_len - nvs_start_address) <= nvs_size {
        return Err(Error::BinaryEditError(file_len));
    }
    app_file
        .seek(SeekFrom::Start(nvs_start_address))
        .map_err(Error::FileError)?;
    app_file.write_all(nvs_data).map_err(Error::FileError)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::WriteCredentials(args)) => {
            let nvs_data = create_nvs_partition_binary(args.config.to_string(), args.size)?;
            write_credentials_to_app_binary(
                &args.binary_path,
                &nvs_data,
                args.size as u64,
                args.nvs_offset,
            )?;
        }
        Some(Commands::WriteFlash(_)) => {
            return Err(Error::UnimplementedError(
                "writing to flash not yet supported".to_string(),
            ))
        }
        Some(Commands::CreateNvsPartition(args)) => {
            let mut file = File::create(&args.file_name).map_err(Error::FileError)?;
            file.write_all(&create_nvs_partition_binary(
                args.config.to_string(),
                args.size,
            )?)
            .map_err(Error::FileError)?;
        }
        None => return Err(Error::NoCommandError),
    };
    Ok(())
}
