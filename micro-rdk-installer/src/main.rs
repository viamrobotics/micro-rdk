use log::LevelFilter;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

#[cfg(target_family = "unix")]
use std::os::unix::fs::FileExt;
#[cfg(target_family = "windows")]
use std::os::windows::fs::FileExt;

use clap::{arg, command, Args, Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Input, Password};
use esp_idf_part::PartitionTable;
use espflash::cli::{
    config::Config, connect, monitor::monitor, serial_monitor, ConnectArgs, EspflashProgress,
    FlashArgs, MonitorArgs,
};
use micro_rdk_installer::{
    error::Error,
    nvs::{
        data::{ViamFlashStorageData, WifiCredentials},
        metadata::read_nvs_metadata,
        partition::{NVSPartition, NVSPartitionData},
        request::download_micro_rdk_release,
    },
};
use secrecy::Secret;
use serde::Deserialize;
use tokio::runtime::Runtime;

const PARTITION_TABLE_ADDR: u32 = 0x8000;
const PARTITION_TABLE_SIZE: u32 = 0xc00;
const EMPTY_BYTE: u8 = 0xFF;
const APP_IMAGE_PARTITION_NAME: &str = "factory";
// taken from `espflash::cli::ReadFlashArgs` default values
const DEFAULT_BLOCK_SIZE: u32 = 0x1000;
const DEFAULT_MAX_IN_FLIGHT: u32 = 64;
const DEFAULT_BAUD: u32 = 115_200;

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
    UpdateAppImage(Box<AppImageArgs>),
    WriteFlash(Box<WriteFlashArgs>),
    WriteCredentials(WriteCredentialsArgs),
    CreateNvsPartition(Box<CreateNVSPartitionArgs>),
    Monitor(MonitorArgs),
}

/// Flash a new micro-RDK app image directly to an ESP32's `factory` partition
#[derive(Args, Clone)]
struct AppImageArgs {
    #[clap(flatten)]
    flash_args: FlashArgs,
    #[clap(flatten)]
    connect_args: ConnectArgs,

    /// File path to the compiled micro-RDK binary. The portion reserved for the NVS
    /// data partition will be edited with Wi-Fi and robot credentials
    #[arg(long = "binary-path")]
    #[clap(conflicts_with = "version")]
    binary_path: Option<PathBuf>,

    /// Version of the compiled micro-RDK server to download.
    /// See https://github.com/viamrobotics/micro-rdk/releases for the version options
    #[arg(long = "version", value_parser = validate_version)]
    version: Option<String>,
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

/// Flash a pre-compiled binary with the micro-RDK, the robot config, and wifi info
/// directly to an ESP32 connected to your computer via data cable
#[derive(Args, Clone)]
struct WriteFlashArgs {
    /// from espflash: baud, port
    #[clap(flatten)]
    connect_args: ConnectArgs,
    #[clap(flatten)]
    flash_args: FlashArgs,

    /// File path to the compiled micro-RDK binary. The portion reserved for the NVS
    /// data partition will be edited with Wi-Fi and robot credentials
    #[arg(long = "binary-path")]
    #[clap(conflicts_with = "version")]
    binary_path: Option<PathBuf>,

    /// File path to the JSON config of the robot, downloaded from app.viam.com
    #[arg(long = "app-config")]
    config: Option<String>,
    /// Version of the compiled micro-RDK server to download.
    /// See https://github.com/viamrobotics/micro-rdk/releases for the version options
    #[arg(long = "version", value_parser = validate_version)]
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
    // declared in micro-rdk-server/esp32/partitions.csv (0x8000, or 32768)
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

fn validate_version(version: &str) -> Result<String, String> {
    // With 0.1.9+ release the installer will not be backward compatible
    // with prior version, therefore we return an error letting the user know they should
    // use an older installer
    let version_019 = version_compare::Version::from("0.1.9").unwrap();

    let requested_version = version_compare::Version::from(version)
        .ok_or(format!("{} is not a valid version string", version))?;

    if requested_version < version_019 {
        return Err(format!("this version of the installer does not support version of micro-rdk < 0.1.9. If you want to install micro-rdk {} please downgrade the installer to v0.1.8 first",version));
    }
    Ok(version.to_owned())
}

fn request_wifi(
    wifi_ssid: Option<String>,
    wifi_password: Option<Secret<String>>,
) -> Result<WifiCredentials, Error> {
    let ssid: String = if let Some(ssid) = wifi_ssid {
        ssid
    } else {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Please enter the WiFi SSID (i.e. WiFi network name) for the network that Micro-RDK will connect to")
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
    let part = &mut NVSPartition::from_storage_data(storage_data, size)?;
    Ok(NVSPartitionData::try_from(part)?.to_bytes())
}

fn write_credentials_to_app_binary(
    binary_path: PathBuf,
    nvs_data: &[u8],
    nvs_size: u64,
    nvs_start_address: u64,
) -> Result<(), Error> {
    // open binary
    let mut app_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(binary_path)
        .map_err(Error::FileError)?;
    // get binary size
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

fn flash(
    flash_args: FlashArgs,
    connect_args: ConnectArgs,
    config: &Config,
    app_path: PathBuf,
) -> Result<(), Error> {
    log::info!("Connecting...");
    let mut flasher = connect(
        &connect_args,
        config,
        flash_args.no_verify,
        flash_args.no_skip,
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
    if flash_args.monitor {
        log::info!("Starting monitor...");
        let pid = flasher.get_usb_pid().map_err(Error::EspFlashError)?;
        monitor(
            flasher.into_serial(),
            None,
            pid,
            DEFAULT_BAUD,
            flash_args.log_format,
            flash_args.log_output,
            true,
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
        Some(Commands::UpdateAppImage(args)) => update_app_image(args)?,
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
            let tmp_path = tempfile::NamedTempFile::new()
                .map_err(Error::FileError)?
                .path()
                .to_path_buf();
            let app_path = match &args.binary_path {
                Some(path) => PathBuf::from(path),
                None => {
                    let rt = Runtime::new().map_err(Error::AsyncError)?;
                    rt.block_on(download_micro_rdk_release(&tmp_path, args.version.clone()))?
                }
            };
            if let Some(app_config) = args.config.as_ref() {
                let nvs_metadata = read_nvs_metadata(app_path.clone())?;

                let nvs_data = create_nvs_partition_binary(
                    app_config.to_string(),
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
            }
            flash(
                args.flash_args.clone(),
                args.connect_args.clone(),
                &config,
                app_path,
            )?;
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

fn update_app_image(args: &AppImageArgs) -> Result<(), Error> {
    let config = Config::load().map_err(|err| Error::SerialConfigError(err.to_string()))?;

    let tmp_old = tempfile::NamedTempFile::new().map_err(Error::FileError)?;

    log::info!("Retrieving running partition table");
    let mut flasher =
        connect(&args.connect_args, &config, false, false).map_err(|_| Error::FlashConnect)?;
    flasher
        .read_flash(
            PARTITION_TABLE_ADDR,
            PARTITION_TABLE_SIZE,
            DEFAULT_BLOCK_SIZE,
            DEFAULT_MAX_IN_FLIGHT,
            tmp_old.path().to_path_buf(),
        )
        .map_err(|_| Error::FlashConnect)?;

    let old_ptable_buf = fs::read(tmp_old).map_err(Error::FileError)?;
    let old_ptable = PartitionTable::try_from_bytes(old_ptable_buf.clone())
        .map_err(|e| Error::PartitionTableError(e.to_string()))?;

    log::info!("Retrieving new image");
    let tmp_new = tempfile::NamedTempFile::new()
        .map_err(Error::FileError)?
        .path()
        .to_path_buf();
    let app_path_new = match &args.binary_path {
        Some(path) => PathBuf::from(path),
        None => {
            let rt = Runtime::new().map_err(Error::AsyncError)?;
            rt.block_on(download_micro_rdk_release(&tmp_new, args.version.clone()))?
        }
    };

    let mut new_ptable_buf = vec![0; PARTITION_TABLE_SIZE as usize];
    log::info!("Extracting new partition table");
    let mut app_file_new = OpenOptions::new()
        .read(true)
        .open(app_path_new.clone())
        .map_err(Error::FileError)?;

    let file_len = app_file_new.metadata().map_err(Error::FileError)?.len();
    if file_len < (PARTITION_TABLE_ADDR as u64 + PARTITION_TABLE_SIZE as u64) {
        return Err(Error::PartitionTableError(
            "file length is less than partition size".to_string(),
        ));
    }

    let _ = app_file_new
        .seek(SeekFrom::Start(PARTITION_TABLE_ADDR.into()))
        .map_err(Error::FileError)?;
    app_file_new
        .read_exact(&mut new_ptable_buf)
        .map_err(Error::FileError)?;
    let new_ptable = PartitionTable::try_from_bytes(new_ptable_buf.clone())
        .map_err(|e| Error::PartitionTableError(e.to_string()))?;

    // Compare partition tables
    if old_ptable != new_ptable {
        log::error!(
            "old and new partition tables do not match - rebuild and flash micro-rdk from scratch"
        );
        return Err(Error::PartitionTableError(
            "incompatible partition tables".to_string(),
        ));
    }

    let app_partition_info = new_ptable.find(APP_IMAGE_PARTITION_NAME).ok_or_else(|| {
        Error::PartitionTableError(format!(
            "failed to find `{}` partition",
            APP_IMAGE_PARTITION_NAME
        ))
    })?;
    let app_offset = app_partition_info.offset();
    let app_size = app_partition_info.size();
    log::debug!(
        "{} offset: {:x}, {} size: {:x}",
        APP_IMAGE_PARTITION_NAME,
        app_offset,
        APP_IMAGE_PARTITION_NAME,
        app_size
    );
    #[allow(unused_mut)]
    let mut app_segment = vec![EMPTY_BYTE; app_size as usize];
    // write just this data
    #[cfg(target_family = "unix")]
    app_file_new
        .read_at(&mut app_segment, app_offset.into())
        .map_err(Error::FileError)?;

    #[cfg(target_family = "windows")]
    app_file_new
        .seek_read(&mut app_segment, app_offset.into())
        .map_err(Error::FileError)?;
    log::info!("Writing new app segment to flash");
    flasher
        .write_bin_to_flash(
            app_offset,
            &app_segment,
            Some(&mut EspflashProgress::default()),
        )
        .map_err(Error::EspFlashError)?;
    log::info!("Running image has been updated");
    if args.flash_args.monitor {
        log::info!("Starting monitor...");
        let pid = flasher.get_usb_pid().map_err(Error::EspFlashError)?;
        monitor(
            flasher.into_serial(),
            None,
            pid,
            DEFAULT_BAUD,
            args.flash_args.log_format,
            args.flash_args.log_output.clone(),
            true,
        )
        .map_err(|err| Error::MonitorError(err.to_string()))?;
    }
    Ok(())
}
