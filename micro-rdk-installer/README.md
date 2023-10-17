# The Viam Micro-RDK Installer

A CLI that allows a user to flash a build of Micro-RDK, along with their robot's credentials and their wifi information, directly to their esp32 without requiring installation of ESP-IDF, Rust, or Python.

## Option 1: Download Pre-Built Binaries

These download links are for the latest release of the installer by architecture
- [x86_64-Linux](https://github.com/viamrobotics/micro-rdk/releases/latest/download/micro-rdk-installer-amd64-linux)
- [aarch64-Linux](https://github.com/viamrobotics/micro-rdk/releases/latest/download/micro-rdk-installer-arm64-linux)
- [MacOS](https://github.com/viamrobotics/micro-rdk/releases/latest/download/micro-rdk-installer-macos)
- [Windows](https://github.com/viamrobotics/micro-rdk/releases/latest/download/micro-rdk-installer-windows.exe)

## Option 2: Build From Source

Only necessary as an alternative to the previous Download step

Requirements: rust (1.67.0 or higher), Cargo (1.67.0 or higher)

1. `git clone https://github.com/viamrobotics/micro-rdk.git`
2. cd micro-rdk/micro-rdk-installer
2.`cargo build`
3. Executable (`micro-rdk-installer`) will be under target/debug

## Usage

```text
A CLI that can flash a compilation of micro-RDK directly to an ESP32 provided configuration information

Usage: micro-rdk-installer [COMMAND]

Commands:
  write-flash           Flash a pre-compiled binary with the micro-RDK server directly to an ESP32 connected to
                            your computer via data cable
  write-credentials     Write Wi-Fi and robot credentials to the NVS storage portion of a pre-compiled binary
                            running a micro-RDK server
  create-nvs-partition  Generate a binary of a complete NVS data partition that contains Wi-Fi and security
                            credentials for a robot
  help                  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### Quick use

1. Find your robot part at https://app.viam.com. Then navigate to the **Setup** tab
2. Regardless of your operating system, select **Mac** and press the button that appears in Step 2 to download the Viam app config for your robot
3. Run: `./micro-rdk-installer write-flash --app-config=<file path to the viam.json file downloaded in previous step>`
    1. To see the micro-RDK server logs through the serial connection, add `--monitor`
    2. If the program cannot auto-detect the serial port to which your ESP32 is connected, you may be prompted to select the correct one among a list

## Common Problems

### Linux Port Permissions

If a "Permission Denied" or similar port error occurs, first check the connection of the ESP32 to the machine's USB port. If 
connected and the error persists, run `sudo usermod -a -G dialout $USER` to add the current user to the `dialout` group, then try again.

### MacOS Executable Permissions

When using a machine running a version of MacOS, the user will be blocked from running the executable. To fix this, **Control+Click** the binary in Finder and then, in the following two prompts select **Open**. Close whatever terminal window this opens to be able to run the installer.

### Error: FlashConnect

This may occur because the serial port chosen if/when prompted is incorrect. However, if the correct port has been selected, try the following:

1. Run the installer as explained above
2. When prompted to select a serial port
    1. Hold down the "EN" or enable button on your ESP32
    2. With the above button held down, select the correct serial port
    3. Press and hold down the "EN" and "Boot" buttons at the same time. Then release both

## Testing NVS

To test this functionality, download a json
file representing the robot's app config from the Setup tab of the robot part's page on app.viam.com and then
run the following command from within this subdirectory:
```
cargo run -- create-nvs-partition --app-config=<path to config json> --output=<destination path for resulting binary>
```

Alternatively you can build the binary (with `cargo build`) and run it in a similar fashion:
```
./micro-rdk-installer create-nvs-partition --app-config=<path to config json> --output=<destination path for resulting binary>
```