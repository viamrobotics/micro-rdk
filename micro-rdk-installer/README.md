# The Viam Micro-RDK Installer

A CLI that allows a user to flash a build of Micro-RDK, along with
their robot's credentials and their wifi information, directly to their esp32 without requiring
installation of ESP-IDF, Rust, or Python.

## Build Instructions

1. `cargo build`
2. ELF will be in target/debug

## Usage

```text
A CLI that can flash a compilation of micro-RDK directly to an ESP32 provided configuration information

Usage: micro-rdk-installer [COMMAND]

Commands:
  write-flash           Flash a pre-compiled binary with the micro-RDK server directly to an ESP32 connected to
                            your computer via data cable
  write-credentials     Write Wi-Fi and robot credentials to the NVS storage portion of a pre-compiled binary
                            running a micro-RDK server
  create-nvs-partition  Generate a binary of a complete NVS data partition that conatins Wi-Fi and security
                            credentials for a robot
  help                  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### Quick use

1. Find your robot part at https://app.viam.com. Then navigate to the Setup tab
2. Follow Step 1 under the instructions for Linux (regardless of your operating system) to download the Viam app config for your robot
3. Run: `./micro-rdk-installer write-flash --app-config=<file path to the viam.json file downloaded in previous step>`
    1. To see the micro-RDK server logs through the serial connection, add `--monitor`

### Linux Port Permissions

If a "Permission Denied" or similar port error occurs, first check the connection of the ESP32 to the machine's USB port. If 
connected and the error persists, run `sudo usermod -a -G dialout $USER` to add the current user to the `dialout` group, then try again.

### MacOS Executable Permissions

When using a machine running a version of MacOS, the user will be blocked from running the executable. To fix this, **Control+Click** the binary in Finder and then, in the following two prompts select **Open**. Close whatever terminal window this opens
and then you should be able to run the installer.

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