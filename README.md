Table of Contents
=================

* [Mini-RDK](#mini-rdk)
   * [Getting Started](#getting-started)
      * [Installing ESP-IDF](#installing-esp-idf)
      * [Installing the Rust ESP Toolchain](#installing-the-rust-esp-toolchain)
      * [Installing cargo generate](#installing-cargo-generate)
      * [Updating cargo espflash](#updating-cargo-espflash)
      * [(Optional) Installing QEMU for esp32](#optional-installing-qemu-for-esp32)
         * [MacOS](#macos)
         * [Linux](#linux)
   * [Your first esp32 robot](#your-first-esp32-robot)
      * [Create a new robot](#create-a-new-robot)
      * [Generate a new mini-rdk project](#generate-a-new-mini-rdk-project)
      * [Upload!!!](#upload)
     * [Building for QEMU](#building-for-qemu)
   * [Next Steps](#nex-steps)

# Mini-RDK

Viam provides an open source robot architecture that provides robotics functionality via simple APIs

The Mini-RDK is a lightweight version of Viam's [RDK](https://github.com/viamrobotics/rdk). Its goal is to be run on resource-limited embedded systems. The only embedded system currently supported is the ESP32.

**Website**: [viam.com](https://www.viam.com)

**Documentation**: [docs.viam.com](https://docs.viam.com)

**Cloud App**: [app.viam.com](https://app.viam.com)

## Contact

* Community Slack: [join](https://join.slack.com/t/viamrobotics/shared_invite/zt-1f5xf1qk5-TECJc1MIY1MW0d6ZCg~Wnw)
* Support: https://support.viam.com

## (In)stability Notice

**Warning**
This is an alpha release of the Viam Mini RDK. Stability is not guaranteed. Breaking changes are likely to occur, and occur often.

## Getting Started

### Installing ESP-IDF
ESP-IDF is the development framework for Espressif SoCs supported on Windows, Linux and macOS.
To properly support cameras we use our own fork on the ESP-IDF. Start by following these [instructions](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/linux-macos-setup.html) up to step 2.

``` shell
mkdir -p ~/esp
cd ~/esp
git clone https://github.com/npmenard/esp-idf
cd esp-idf
git checkout v4.4.1
git submodule update --init --recursive
```
You will then need to install the tools for ESP-IDF

``` shell
cd ~/esp/esp-idf
./install.sh esp32
```

Finally to activate ESP-IDF use `. $HOME/esp/esp-idf/export.sh`. Note that you shouldn't add this to your `.bashrc` or `.zshrc` to avoid conflicts with other toolchains.

### Installing the Rust ESP Toolchain
To install the rust toolchain run:

``` shell
curl -LO https://github.com/esp-rs/rust-build/releases/download/v1.64.0.0/install-rust-toolchain.sh
chmod a+x install-rust-toolchain.sh
./install-rust-toolchain.sh
```
The script will give you two variables to add to your `.zshrc` or `.bashrc`. You may do so, but this may cause conflicts with other toolchains. The installer will
also produce a file called `export-esp.sh` which you may retain under a name and location of your choice, and then source when needed:

``` shell
mv ./export-esp.sh $HOME/esp/esp-idf/export-esp-rs.sh
. $HOME/esp/esp-idf/export-esp-rs.sh

```

### Installing cargo generate

``` shell
cargo install cargo-generate
```

### Updating cargo espflash
The default version of espflash has a bug therefore we need to update it to a beta version.

``` shell
cargo install cargo-espflash@2.0.0-rc.1
```

### (Optional) Installing QEMU for esp32
Espressif maintains a pretty good QEMU emulator supporting the ESP32, we recommend using it during development. See [here](https://github.com/espressif/qemu) for more information

#### MacOS
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
First, make sure you have the libgcrypt library and headers installed. On Ubuntu or Debian, this is
``` shell
sudo apt-get install libgcrypt20 libgcrypt20-dev
```

Then, install QEMU itself:
``` shell
git clone https://github.com/espressif/qemu
cd qemu
./configure --target-list=xtensa-softmmu     --enable-gcrypt \
    --enable-debug --enable-sanitizers  --disable-strip --disable-user \
    --disable-capstone --disable-vnc --disable-sdl --disable-gtk
cd build && ninja
```

Add `export QEMU_ESP32_XTENSA=<path-to-clone-qemu>/build/` to your `.zshrc` or `.bashrc`

## Your first esp32 robot
Congratulation for making it this far, just a few more steps and you will be running your first esp32 robot!

### Create a new robot
You will want to navigate to app.viam.com and create a new robot with an empty config in your favorite location. The `Mode` and `Architecture` selections can be
ignored and left at the default. You may also skip any setup steps about downloading, installing, or starting `viam-server`, since it is not used on the ESP32 board
The only required step is to download the Viam app config for the new robot.

### Generate a new mini-rdk project
Using a template we are going to create a new mini-rdk project that can be uploaded to an esp32.
You will be asked several questions needed to setup Wifi among other things, at one point you will be asked to input a viam robot configuration: be sure to use the one you just downloaded from app.viam.com
``` shell
cargo generate --git git@github.com:viamrobotics/mini-rdk-template.git
```

If you like, you can initialize a new revision control repository in the newly created directory to track any changes you need to make to the generated project. All of the generated files should be
safe to commit with the exception of `viam.json`, since it contains a secret key.

### Upload!!!
Modify src/main.rs to you liking and run :

``` shell
make upload
```

While running `make upload` you may be presented with an interactive menu of different serial port options to use to connect to the board. Once you have identified
the correct choice for your environment, you may bypass the menu by providing the correct port as an argument to future invocations of `make upload`:

``` shell
make ESPFLASH_FLASH_ARGS="-p /dev/cu.usbserial-130" upload
```

If successful, `make upload` will retain a serial connection to the board until `Ctrl-C` is pressed, so consider running it within a dedicated terminal session
(or under `tmux` or `screen`). While the serial connection is live, you can also restart the currently flashed image with `Ctrl-R`.

If everything went well the esp32 connected will be programmed and you will be able to see your robot live on app.viam.com.

NOTE: If you encounter a crash due to stack overflow, you may need to increase the stack available to the main task. Edit the generated `sdkconfig.defaults` file as follows and re-flash the board:

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

In order to control the robot now running on the esp32, you will need another robot to host your application, since the esp32 cannot do so.

- Navigate to app.viam.com, and either create and configure a new robot, or select an existing robot to which you would like to add the esp32-backed robot.
- Add the esp32-backed robot as a "remote" of your new or existing robot:
  - Navigate to the `Control` tab of the esp32-backed robot and copy its `Remote Address`.
  - Navigate to the `Config` tab of the robot from which you want to control the esp32-based robot, select the `Remotes` tab, and create a new remote.
  - Set the `Address` field of the new remote to be the `Remote Address` you copied above, and add `:4545` to the end
  - Set `TLS` for the remote to `Enabled`.
- Ensure that the controlling robot is live.
- The esp32-backed robot should now be programmatically available in the application controlling the robot to which the esp-backed robot was added as a remote.


## Building for QEMU
Navigate to the root of the Mini-RDK repository, once here run `. $HOME/esp/esp-idf/export.sh` if you haven't done so already.
You will need to comment out two lines from the file `sdkconfig.defaults`

``` editorconfig
CONFIG_ESPTOOLPY_FLASHFREQ_80M=y
CONFIG_ESPTOOLPY_FLASHMODE_QIO=y
```

You can then run
``` shell
make sim-local
```
Or if you want to connect a debugger
``` shell
make debug-local
```

## License
Copyright 2022-2023 Viam Inc.

AGPLv3 - See [LICENSE](https://github.com/viamrobotics/mini-rdk/blob/main/LICENSE) file
