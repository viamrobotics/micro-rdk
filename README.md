Table of Contents
=================

- [Table of Contents](#table-of-contents)
- [Micro-RDK (Robot Development Kit for Microcontrollers)](#micro-rdk-robot-development-kit-for-microcontrollers)
  - [Contact](#contact)
  - [(In)stability Notice](#instability-notice)
  - [Getting Started](#getting-started)
    - [Install ESP-IDF](#install-esp-idf)
    - [Install Rust](#install-rust)
      - [MacOS \& Linux](#macos--linux)
    - [Install the Rust ESP Toolchain](#install-the-rust-esp-toolchain)
      - [Activate the ESP-RS Virtual Environment](#activate-the-esp-rs-virtual-environment)
    - [Install `cargo-generate` with `cargo`](#install-cargo-generate-with-cargo)
    - [Update `cargo-espflash`](#update-cargo-espflash)
    - [(Optional) Install the QEMU ESP32 Emulator](#optional-install-the-qemu-esp32-emulator)
      - [MacOS](#macos)
      - [Linux](#linux)
  - [Your first ESP32 robot](#your-first-esp32-robot)
    - [Create a new robot](#create-a-new-robot)
    - [Generate a new micro-rdk project](#generate-a-new-micro-rdk-project)
    - [Upload](#upload)
  - [Next Steps](#next-steps)
    - [Configure the ESP32 as a remote](#configure-the-esp32-as-a-remote)
    - [Modifying the generated template](#modifying-the-generated-template)
      - [Exposing other gpio pins](#exposing-other-gpio-pins)
      - [Adding a new analog reader](#adding-a-new-analog-reader)
  - [Building with the QEMU ESP32 Emulator](#building-with-the-qemu-esp32-emulator)
    - [Troubleshooting](#troubleshooting)
  - [License](#license)

# Micro-RDK (Robot Development Kit for Microcontrollers)

Viam provides an open source robot architecture that provides robotics functionality via simple APIs.

The Micro-RDK is a lightweight version of Viam's [RDK](https://github.com/viamrobotics/rdk). Its goal
is to be run on resource-limited embedded systems. The only microcontroller currently supported is
the ESP32.

**Website**: [viam.com](https://www.viam.com)

**Documentation**: [docs.viam.com](https://docs.viam.com)

**Cloud App**: [app.viam.com](https://app.viam.com)

## Contact

- Community Slack: [join](https://join.slack.com/t/viamrobotics/shared_invite/zt-1f5xf1qk5-TECJc1MIY1MW0d6ZCg~Wnw)
- Support: <https://support.viam.com>

## (In)stability Notice

**Warning** This is an alpha release of the Viam Micro-RDK. Stability is not guaranteed. Breaking
changes are likely to occur, and occur often.

## Getting Started

ESP-IDF is the development framework for Espressif SoCs, supported on Windows, Linux and macOS.
Viam recommends using [our fork](https://github.com/npmenard/esp-idf) of the ESP-IDF framework to support camera configuration.

### Install ESP-IDF

Start by completing Step 1 of [these instructions](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/linux-macos-setup.html), following the appropriate steps for your development machine's architecture, and then return here.

Clone Viam's fork of the ESP-IDF:

``` shell
mkdir -p ~/esp
cd ~/esp
git clone https://github.com/npmenard/esp-idf
cd esp-idf
git checkout v4.4.1
git submodule update --init --recursive
```

Then, install the required tools for ESP-IDF:

``` shell
cd ~/esp/esp-idf
./install.sh esp32
```

Finally, to activate ESP-IDF, source the activation script `export.sh`:

``` shell
. $HOME/esp/esp-idf/export.sh
```

To avoid conflicts with other toolchains, adding this command to your `.bashrc` or `.zshrc` is not recommended.
Save this command to run in any future terminal session where you need to activate the ESP-IDF development framework.

### Install Rust

#### MacOS & Linux

If you don't already have the Rust programming language installed on your development machine, run the following command to download Rustup and install Rust:

``` shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

See <https://www.rust-lang.org/tools/install> for more information and other installation methods.

### Install the Rust ESP Toolchain

To install the Rust ESP toolchain, run:

``` shell
curl -LO https://github.com/esp-rs/rust-build/releases/download/v1.64.0.0/install-rust-toolchain.sh
chmod a+x install-rust-toolchain.sh
./install-rust-toolchain.sh
```

#### Activate the ESP-RS Virtual Environment

Running this script will prompt you to add two variables to your `.zshrc` or `.bashrc` if you want to activate the ESP-RS environment automatically in every terminal session:

``` shell
IMPORTANT!
 The following environment variables need to be updated:
export LIBCLANG_PATH= ...
```

Doing so is not recommended, as this may cause conflicts with other toolchains.
As an alternative, the script prompts you to save the export file `export-esp.sh`.
Viam recommends following this method.

Run the following command to save the `./export-esp.sh` file at `$HOME/esp/esp-idf/export-esp-rs.sh`:

``` shell
mv ./export-esp.sh $HOME/esp/esp-idf/export-esp-rs.sh
```

After doing so, run the following command to source (`.`) this file, activating the ESP-RS Virtual Environment:

``` shell
. $HOME/esp/esp-idf/export-esp-rs.sh
```

Save this command to run in any future terminal session where you need to activate the Virtual Environment.

### Install `cargo-generate` with `cargo`

**NOTE:** `cargo` is installed when downloading Rust with Rustup.

- If you need to install `cargo`, run the following command, or see <https://doc.rust-lang.org/cargo/getting-started/installation.html> for other installation methods:

  ``` shell
  curl https://sh.rustup.rs -sSf | sh
  ```

Run the following command to install `cargo-generate`:

``` shell
cargo install cargo-generate
```

### Update `cargo-espflash`

As of 2/16/2022 the default version of `cargo-espflash` has a bug affecting performance.
Therefore, Viam recommends updating your version to a beta release.

Run the following command to update `cargo-espflash` to our recommended version:

``` shell
cargo install cargo-espflash@2.0.0-rc.1
```

### (Optional) Install the QEMU ESP32 Emulator

Espressif maintains a pretty good QEMU emulator supporting the ESP32, we recommend using it during
development. See [here](https://github.com/espressif/qemu) for more information.

#### MacOS

Run the following command to install the QEMU ESP32 Emulator:

``` shell
git clone https://github.com/espressif/qemu
cd qemu
./configure --target-list=xtensa-softmmu \
    --enable-gcrypt \
    --enable-debug --enable-sanitizers \
    --disable-strip --disable-user \
    --disable-capstone --disable-vnc \
    --disable-sdl --disable-gtk --extra-cflags="-I/opt/homebrew/Cellar/libgcrypt/1.10.1/include -I/opt/homebrew//include/"
cd build && ninja
```

#### Linux

On Ubuntu or Debian, first make sure you have the `libgcrypt` library and headers installed by running the following command:

``` shell
sudo apt-get install libgcrypt20 libgcrypt20-dev
```

Then, run the following command to install QEMU:

``` shell
git clone https://github.com/espressif/qemu
cd qemu
./configure --target-list=xtensa-softmmu     --enable-gcrypt \
    --enable-debug --enable-sanitizers  --disable-strip --disable-user \
    --disable-capstone --disable-vnc --disable-sdl --disable-gtk
cd build && ninja
```

Add `export QEMU_ESP32_XTENSA=<path-to-clone-qemu>/build/` to your `.zshrc` or `.bashrc`, or save this command to run in your terminal every session you wish to use the QEMU emulator.

## Your first ESP32 robot

### Create a new robot

Navigate to [the Viam App](app.viam.com) and create a new robot in your desired location.
Leave your `Mode` and `Architecture` selections at default.
Skip any setup steps about downloading, installing, or starting `viam-server`, since it is not used on the ESP32 board.

When completing the next step to generate a new micro-rdk project, you will be asked to paste a viam robot configuration into the terminal.
Use the `Copy viam-server config` button on the `Setup` tab for your new robot to obtain the correct value.

### Generate a new micro-rdk project

Using [this template](https://github.com/viamrobotics/micro-rdk-template.git), we are going to create a new micro-rdk project that can be uploaded to an ESP32 microcontroller board.

Run the following command to generate a new project with `cargo`:

``` shell
cargo generate --git https://github.com/viamrobotics/micro-rdk-template.git
```

If you would like, you can use `mkdir` to initialize a new repository in the directory you created by running that command to track any changes you need to make to the generated project.
All of the generated files should be safe to commit with the exception of `viam.json`, since it contains a secret key.

### Upload

Modify the contents of the file src/main.rs to your liking and run:

``` shell
make upload
```

While running `make upload`, you may be presented with an interactive menu of different serial port options to use to connect to the ESP32 board.
Once you have identified the correct choice for your environment, you may bypass the menu by providing the correct port as an argument to future invocations of `make upload`:

``` shell
make ESPFLASH_FLASH_ARGS="-p /dev/cu.usbserial-130" upload
```

If successful, `make upload` will retain a serial connection to the board until `Ctrl-C` is pressed, so consider running it within a dedicated terminal session (or under `tmux` or `screen`).
While the serial connection is live, you can also restart the currently flashed image with `Ctrl-R`.

If everything went well, your ESP32 will be programmed so that you will be able to see your robot live on <app.viam.com>.

NOTE: If you encounter a crash due to stack overflow, you may need to increase the stack available to the main task.
Edit the generated `sdkconfig.defaults` file as follows and re-flash the board:

``` diff
diff --git a/sdkconfig.defaults b/sdkconfig.defaults
index f75b465..2b0ba9c 100644
--- a/sdkconfig.defaults
+++ b/sdkconfig.defaults
@@ -1,5 +1,5 @@
 # Rust often needs a bit of an extra main task stack size compared to C (the default is 3K)
-CONFIG_ESP_MAIN_TASK_STACK_SIZE=24576
+CONFIG_ESP_MAIN_TASK_STACK_SIZE=32768
 CONFIG_ESP_MAIN_TASK_AFFINITY_CPU1=y
 # Use this to set FreeRTOS kernel tick frequency to 1000 Hz (100 Hz by default).
```

## Next Steps

### Configure the ESP32 as a remote

In order to control the robot now running on the ESP32, you will need another robot to host your
application, since the esp32 cannot do so.

- Navigate to <app.viam.com>. Create and configure a new robot, *or* select an existing robot
  to which you would like to add the ESP32-backed robot.
- Add the ESP32-backed robot as a "remote" of your new or existing robot:
  - Navigate to the `Control` tab of the ESP32-backed robot and copy its `Remote Address`.
  - Navigate to the `Config` tab of the robot from which you want to control the esp32-based robot,
    select the `Remotes` tab, and create a new remote.
  - Set the `Address` field of the new remote to be the `Remote Address` you copied above.
  - Set `TLS` for the remote to `Enabled`.
- Ensure that the controlling robot is live in <app.viam.com>.
  - The ESP32-backed robot should now be programmatically available in the application controlling the
  robot to which the ESP-backed robot was added as a remote.
  
### Modifying the generated template

You can find the declaration of the robot in the generated file `src/main.rs`.
In this example, we expose one gpio pin (pin 18), and one analog reader attached to gpio pin 34.

#### Exposing other gpio pins

Once you have selected an appropriate gpio pin (according to the pinout diagram with your ESP32), you can add to the collection of exposed pins.
For example, if you want to expose gpio pin 21, simply change the line:

``` rust
let pins = vec![PinDriver::output(periph.pins.gpio18.downgrade_output())?];
```

to

``` rust
let pins = vec![PinDriver::output(periph.pins.gpio18.downgrade_output())?,
    PinDriver::output(periph.pins.gpio21.downgrade_output())?,];
```

You will now be able to change & read the state of pin 21 from <app.viam.com>.

#### Adding a new analog reader

Adding a new analog reader requires a couple more steps.
First, you will want to identify a pin capable of analog reading.

In the pinout diagram of the ESP32, the pins are labeled like this:
- `ADCn_y`: where `n` is the adc number (1 or 2, note that 2 cannot be used with WiFi enabled), and `y` is the channel number.

Once you have identified an appropriate pin, follow these steps to add it.
In this example, we want to add gpio pin 35, which is labeled `ADC1_7` in the pinout diagram:

- Create a new ADC channel:

``` rust
let my_analog_channel = adc_chan: AdcChannelDriver<_, Atten11dB<adc::ADC1>> =
            AdcChannelDriver::new(periph.pins.gpio35)?;
```

- Create the actual Analog reader (note that `adc1` is declared above):

``` rust
let my_analog_reader = Esp32AnalogReader::new("A2".to_string(), my_analog_channel, adc1.clone());
```

- Finally, add the collection of analog readers:

``` rust
let analog_readers = vec![
            Rc::new(RefCell::new(analog1)),
            Rc::new(RefCell::new(my_analog_reader)),
        ];
```

## Building with the QEMU ESP32 Emulator

Navigate to the root of the Micro-RDK repository.
Once you've `cd`'d to the correct repository, run `. $HOME/esp/esp-idf/export.sh` if you haven't done so already in this terminal session.

You will need to comment out two lines from the file `sdkconfig.defaults`:

``` editorconfig
CONFIG_ESPTOOLPY_FLASHFREQ_80M=y
CONFIG_ESPTOOLPY_FLASHMODE_QIO=y
```

You can then run:

``` shell
make sim-local
```

Or, if you want to connect a debugger:

``` shell
make debug-local
```

### Troubleshooting

- If you are unable to connect to the esp32-backed robot as a remote, try adding `:4545` to the end
  of the value set in the remotes `Address` field above.

## License

Copyright 2022-2023 Viam Inc.

AGPLv3 - See [LICENSE](https://github.com/viamrobotics/micro-rdk/blob/main/LICENSE) file
