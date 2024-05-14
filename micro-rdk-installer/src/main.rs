use log::LevelFilter;
use std::{
    fs::{self, read, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::FileExt,
    path::PathBuf,
};

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
        request::{download_micro_rdk_release, populate_nvs_storage_from_app},
    },
};
use secrecy::Secret;
use serde::Deserialize;
use tokio::runtime::Runtime;

const PARTITION_TABLE_ADDR: u16 = 0x8000;
const PARTITION_TABLE_SIZE: u16 = 0xc00;

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

/// Flash a pre-compiled binary with the micro-RDK server directly to an ESP32
/// connected to your computer via data cable
#[derive(Args, Clone)]
struct AppImageArgs {
    /// from espflash: bootloader, log_output, monitor
    #[clap(flatten)]
    flash_args: FlashArgs,
    #[clap(flatten)]
    connect_args: ConnectArgs,

    /// Version of the compiled micro-RDK server to download.
    /// See https://github.com/viamrobotics/micro-rdk/releases for the version options
    #[arg(long = "version")]
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
    monitor_args: Option<MonitorArgs>,
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
    monitor_args: Option<MonitorArgs>,
    config: &Config,
    app_path: PathBuf,
) -> Result<(), Error> {
    log::info!("Connecting...");
    let mut flasher = connect(
        &monitor_args.clone().unwrap().connect_args,
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
            115_200,
            flash_args.log_format,
            flash_args.log_output,
            !monitor_args.unwrap().non_interactive,
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
        Some(Commands::UpdateAppImage(args)) => {
            let config = Config::load().map_err(|err| Error::SerialConfigError(err.to_string()))?;

            // Create a directory inside of `std::env::temp_dir()`.
            let dir = tempfile::tempdir().unwrap();
            let tmp_old = dir.path().join("running-app.img");

            log::info!("Retrieving running partition table");
            let mut flasher = connect(&args.connect_args, &config, false, false).unwrap();
            flasher
                .read_flash(
                    PARTITION_TABLE_ADDR.into(),
                    PARTITION_TABLE_SIZE.into(),
                    0x1000,
                    64,
                    tmp_old.clone(),
                )
                .map_err(|_| Error::FlashConnect)?;

            log::info!("Parsing running partition table");
            let mut ptable_raw = fs::read(tmp_old).map_err(Error::FileError)?;
            log::info!("Extracting running partition table");

            let old_ptable_hash = md5::compute(ptable_raw.clone());

            log::info!("parsing ptable");
            let ptable = PartitionTable::try_from_bytes(ptable_raw.clone()).unwrap();
            log::info!(
                "hash: {:?} \n partitions: {:?}",
                old_ptable_hash,
                ptable.partitions()
            );

            log::info!("Retrieving new image");
            let tmp_new = tempfile::Builder::new()
                .prefix("micro-rdk-bin")
                .tempdir()
                .map_err(Error::FileError)?;
            let app_path_new = match &args.flash_args.bootloader {
                Some(path) => PathBuf::from(path),
                None => {
                    let rt = Runtime::new().map_err(Error::AsyncError)?;
                    rt.block_on(download_micro_rdk_release(&tmp_new, args.version.clone()))?
                }
            };

            log::info!("extracting new partition table.");
            let app_file_new = OpenOptions::new()
                .read(true)
                .write(true)
                .open(app_path_new.clone())
                .map_err(Error::FileError)?;
            let _num_read = app_file_new
                .read_at(&mut ptable_raw, PARTITION_TABLE_ADDR.into())
                .map_err(Error::FileError)?;
            let new_ptable_hash = md5::compute(ptable_raw.clone());

            // Compare partition tables
            if old_ptable_hash != new_ptable_hash {
                log::error!("Partition tables do not match!");
                log::error!("Rebuild and flash micro-rdk from scratch");
                return Err(Error::BinaryEditError(64));
            }

            log::info!("partition tables match!");

            log::info!("parsing ptable");
            let ptable = PartitionTable::try_from_bytes(ptable_raw)
                .map_err(|_| Error::PartitionTableError)?;
            let nvs_partition_info = ptable
                .find("factory")
                .expect("failed to retrieve nvs partition table entry");
            let app_offset = nvs_partition_info.offset();
            let app_size = nvs_partition_info.size();
            log::debug!("app offset: {}, app_size: {}", app_offset, app_size);
            let mut app_segment = Vec::with_capacity(app_size as usize);
            // write just this data
            app_file_new
                .read_at(&mut app_segment, app_offset.into())
                .expect("failed to extract app segment");
            log::info!("writing new app segment to flash");
            flasher
                .write_bin_to_flash(app_offset, &app_segment, None)
                .expect(&format!(
                    "failed to embed new app image at offset {:x}",
                    app_offset
                ));

            /*
            // get binary size
            let file_len = app_file_new.metadata().map_err(Error::FileError)?.len();
            if (nvs_offset as u64 + nvs_size as u64) >= file_len {
                return Err(Error::BinaryEditError(file_len));
            }

            // get nvs data from the old binary
            let dir = tempfile::tempdir().unwrap();
            let nvs_path = dir.path().join("nvs.data");
            log::info!("extracting nvs data from device");
            // let mut flasher = connect(&args.connect_args, &config, false, false).unwrap();
            flasher
                .read_flash(nvs_offset, nvs_size, 0x1000, 64, nvs_path.clone())
                .map_err(Error::EspFlashError)?;

            log::info!("Writing nvs data to new image");
            let mut nvs_file = OpenOptions::new()
                .read(true)
                .open(nvs_path.clone())
                .map_err(Error::FileError)?;
            let mut nvs_dat = Vec::new();
            let nbytes: u32 = Read::read(&mut nvs_file, &mut nvs_dat)
                .unwrap()
                .try_into()
                .unwrap();
            if nbytes != nvs_size {
                return Err(Error::BinaryEditError(nbytes.into()));
            }

            write_credentials_to_app_binary(
                app_path_new.clone(),
                &nvs_dat,
                nvs_size.into(),
                nvs_offset.into(),
            )?;

            log::info!("Flashing updated image to device");
            flash(args.flash_args.clone(), None, &config, app_path_new)?;
            */
        }
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
            flash(
                args.flash_args.clone(),
                args.monitor_args.clone(),
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
