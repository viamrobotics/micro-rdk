use log::LevelFilter;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use clap::{arg, command, Args, Parser, Subcommand};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Password};
use espflash::cli::{
    config::Config, connect, monitor::monitor, serial_monitor, EspflashProgress, FlashArgs,
    MonitorArgs,
};
use micro_rdk_installer::{
    error::Error,
    nvs::{
        data::{ViamFlashStorageData, WifiCredentials},
        metadata::read_nvs_metadata,
        partition::{NVSPartition, NVSPartitionData},
        request::{download_micro_rdk_release, populate_nvs_storage_from_app},
    },
};
use secrecy::Secret;
use serde::Deserialize;
use tokio::runtime::Runtime;

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
    WriteFlash(Box<WriteFlashArgs>),
    WriteCredentials(WriteCredentialsArgs),
    CreateNvsPartition(Box<CreateNVSPartitionArgs>),
    Monitor(MonitorArgs),
}

/// Write Wi-Fi and robot credentials to the NVS storage portion of a pre-compiled
/// binary running a micro-RDK server
#[derive(Args)]
struct WriteCredentialsArgs {
    /// File path to the JSON config of the robot, downloaded from app.viam.com
    #[arg(long = "app-config")]
    config: String,
    /// File path to the compiled micro-RDK binary. The portion reserved for the NVS
    /// data partition will be edited with Wi-Fi and robot credentials
    #[arg(long = "binary-path")]
    binary_path: String,
    /// Wi-Fi SSID to write to NVS partition of binary. If not provided, user will be
    /// prompted for it
    #[arg(long = "wifi-ssid")]
    wifi_ssid: Option<String>,
    /// Wi-Fi password to write to NVS partition of binary. If not provided, user will be
    /// prompted for it
    #[arg(long = "wifi-password")]
    wifi_password: Option<Secret<String>>,
}

/// Flash a pre-compiled binary with the micro-RDK server directly to an ESP32
/// connected to your computer via data cable
#[derive(Args, Clone)]
struct WriteFlashArgs {
    /// from espflash: baud, port
    #[clap(flatten)]
    monitor_args: MonitorArgs,
    /// from espflash: bootloader, log_output, monitor
    #[clap(flatten)]
    flash_args: FlashArgs,

    /// File path to the JSON config of the robot, downloaded from app.viam.com
    #[arg(long = "app-config")]
    config: String,
    /// Version of the compiled micro-RDK server to download.
    /// See https://github.com/viamrobotics/micro-rdk/releases for the version options
    #[arg(long = "version")]
    version: Option<String>,
    /// Wi-Fi SSID to write to NVS partition of binary. If not provided, user will be
    /// prompted for it
    #[arg(long = "wifi-ssid")]
    wifi_ssid: Option<String>,
    /// Wi-Fi password to write to NVS partition of binary. If not provided, user will be
    /// prompted for it
    #[arg(long = "wifi-password")]
    wifi_password: Option<Secret<String>>,
}

/// Generate a binary of a complete NVS data partition that conatins Wi-Fi and security
/// credentials for a robot
#[derive(Args)]
struct CreateNVSPartitionArgs {
    // File path to the JSON config of the robot, downloaded from app.viam.com
    #[arg(long = "app-config")]
    config: String,
    #[arg(long = "output")]
    file_name: String,
    // Size of the NVS partition in bytes. The default here represents the size
    // declared in examples/esp32/partitions.csv (0x8000, or 32768)
    #[arg(long = "size", default_value = "32768")]
    size: usize,
    /// Wi-Fi SSID to write to NVS partition of binary. If not provided, user will be
    /// prompted for it
    #[arg(long = "wifi-ssid")]
    wifi_ssid: Option<String>,
    /// Wi-Fi password to write to NVS partition of binary. If not provided, user will be
    /// prompted for it
    #[arg(long = "wifi-password")]
    wifi_password: Option<Secret<String>>,
}

#[derive(Parser)]
#[command(
    about = "A CLI that can flash a compilation of micro-RDK directly to an ESP32 provided configuration information",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn request_wifi(
    wifi_ssid: Option<String>,
    wifi_password: Option<Secret<String>>,
) -> Result<WifiCredentials, Error> {
    let ssid: String = if let Some(ssid) = wifi_ssid {
        ssid
    } else {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Please enter WiFi SSID")
            .interact_text()
            .unwrap()
    };
    let password: Secret<String> = if let Some(password) = wifi_password {
        password
    } else {
        Secret::new(
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
        )
    };
    Ok(WifiCredentials { ssid, password })
}

fn create_nvs_partition_binary(
    config_path: String,
    size: usize,
    wifi_ssid: Option<String>,
    wifi_password: Option<Secret<String>>,
) -> Result<Vec<u8>, Error> {
    let mut storage_data = ViamFlashStorageData::default();
    let config_str = fs::read_to_string(config_path).map_err(Error::FileError)?;
    let app_config: AppConfig = serde_json::from_str(&config_str)?;
    storage_data.robot_credentials.robot_id = Some(app_config.cloud.r#id.to_string());
    storage_data.robot_credentials.app_address = Some(app_config.cloud.app_address.to_string());
    storage_data.robot_credentials.robot_secret = Some(app_config.cloud.secret);
    let wifi_cred = request_wifi(wifi_ssid, wifi_password)?;
    storage_data.wifi = Some(wifi_cred);
    log::info!(
        "Creating NVS partition with robot id: {:?}, wifi ssid: {:?}.",
        storage_data
            .robot_credentials
            .robot_id
            .clone()
            .unwrap_or(String::from("none")),
        storage_data
            .wifi
            .clone()
            .ok_or(Error::NVSDataProcessingError("no wifi".to_string()))?
            .ssid
    );
    populate_nvs_storage_from_app(&mut storage_data)?;
    let part = &mut NVSPartition::from_storage_data(storage_data, size)?;
    Ok(NVSPartitionData::try_from(part)?.to_bytes())
}

fn write_credentials_to_app_binary(
    binary_path: PathBuf,
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
    if (nvs_start_address + nvs_size) >= file_len {
        return Err(Error::BinaryEditError(file_len));
    }
    app_file
        .seek(SeekFrom::Start(nvs_start_address))
        .map_err(Error::FileError)?;
    log::info!("Writing credentials to binary.");
    app_file.write_all(nvs_data).map_err(Error::FileError)?;
    Ok(())
}

fn flash(args: WriteFlashArgs, config: &Config, app_path: PathBuf) -> Result<(), Error> {
    log::info!("Connecting...");
    let mut flasher = connect(
        &args.monitor_args.connect_args,
        config,
        args.flash_args.no_verify,
        args.flash_args.no_skip,
    )
    .map_err(|_| Error::FlashConnect)?;
    let mut f = File::open(app_path).map_err(Error::FileError)?;
    let size = f.metadata().map_err(Error::FileError)?.len();
    let mut buffer = Vec::with_capacity(
        size.try_into()
            .map_err(|_| Error::BinaryBufferError(size))?,
    );
    f.read_to_end(&mut buffer).map_err(Error::FileError)?;
    log::info!("Connected. Writing to flash...");
    flasher
        .write_bin_to_flash(0x00, &buffer, Some(&mut EspflashProgress::default()))
        .map_err(Error::EspFlashError)?;
    log::info!("Flashing completed.");
    if args.flash_args.monitor {
        log::info!("Starting monitor...");
        let pid = flasher.get_usb_pid().map_err(Error::EspFlashError)?;
        monitor(
            flasher.into_serial(),
            None,
            pid,
            115_200,
            args.flash_args.log_format,
            args.flash_args.log_output,
            !args.monitor_args.non_interactive,
        )
        .map_err(|err| Error::MonitorError(err.to_string()))?;
    }
    Ok(())
}

fn init_logger() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Off)
        .filter_module("micro_rdk_installer", LevelFilter::Info)
        .format(|buf, record| {
            let style = buf.style();

            // just the message, no timestamp or log level
            writeln!(buf, "{}", style.value(record.args()))
        })
        .init();
}

fn main() -> Result<(), Error> {
    init_logger();
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::WriteCredentials(args)) => {
            let app_path = PathBuf::from(args.binary_path.clone());
            let nvs_metadata = read_nvs_metadata(app_path.clone())?;
            let nvs_data = create_nvs_partition_binary(
                args.config.to_string(),
                nvs_metadata.size as usize,
                args.wifi_ssid.clone(),
                args.wifi_password.clone(),
            )?;
            write_credentials_to_app_binary(
                app_path,
                &nvs_data,
                nvs_metadata.size,
                nvs_metadata.start_address,
            )?;
        }
        Some(Commands::WriteFlash(args)) => {
            let config = Config::load().map_err(|err| Error::SerialConfigError(err.to_string()))?;
            let tmp_dir = tempfile::Builder::new()
                .prefix("micro-rdk-bin")
                .tempdir()
                .map_err(Error::FileError)?;
            let app_path = match &args.flash_args.bootloader {
                Some(path) => PathBuf::from(path),
                None => {
                    let rt = Runtime::new().map_err(Error::AsyncError)?;
                    rt.block_on(download_micro_rdk_release(&tmp_dir, args.version.clone()))?
                }
            };
            let nvs_metadata = read_nvs_metadata(app_path.clone())?;
            let nvs_data = create_nvs_partition_binary(
                args.config.to_string(),
                nvs_metadata.size as usize,
                args.wifi_ssid.clone(),
                args.wifi_password.clone(),
            )?;
            write_credentials_to_app_binary(
                app_path.clone(),
                &nvs_data,
                nvs_metadata.size,
                nvs_metadata.start_address,
            )?;
            flash(*args.clone(), &config, app_path)?;
        }
        Some(Commands::CreateNvsPartition(args)) => {
            let mut file = File::create(&args.file_name).map_err(Error::FileError)?;
            file.write_all(&create_nvs_partition_binary(
                args.config.to_string(),
                args.size,
                args.wifi_ssid.clone(),
                args.wifi_password.clone(),
            )?)
            .map_err(Error::FileError)?;
        }
        Some(Commands::Monitor(args)) => {
            let config = Config::load().map_err(|err| Error::SerialConfigError(err.to_string()))?;
            serial_monitor(args, &config).map_err(|err| Error::MonitorError(err.to_string()))?
        }
        None => return Err(Error::NoCommandError),
    };
    Ok(())
}
