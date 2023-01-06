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
This is an beta release of the Viam Mini RDK. Stability is not guaranteed. Breaking changes are likely to occur, and occur often.

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

Finally to activate ESP-IDF use `. $HOME/esp/esp-idf/export.sh` note that you shouldn't add this to your `.bashrc` or `.zshrc` to avoid conflicts with other toolchains

### Installing the Rust ESP Toolchain
To install the rust toolchain run :

``` shell
curl -LO https://github.com/esp-rs/rust-build/releases/download/v1.64.0.0/install-rust-toolchain.sh
chmod a+x install-rust-toolchain.sh
./install-rust-toolchain.sh
```
The script will give you two variables to add to you `.zshrc` or `.bashrc` do so and refresh the shell

### Installing cargo generate

``` shell
cargo install cargo-generate
```

### Updating cargo espflash
The default version of espflash has a bug therefore we need to update it to a beta version 

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
You will want to navigate to app.viam.com and create a new robot with an empty config in your favorite location

### Generate a new mini-rdk project
Using a template we are going to create a new mini-rdk project that can be uploaded to an esp32. 
You will be asked several questions needed to setup Wifi among other things, at one point you will be asked to input a viam robot configuration be sure to use the one you just created from app.viam.com
``` shell
cargo generate --git git@github.com:viamrobotics/mini-rdk-template.git
```

### Upload!!!
Modify src/main.rs to you liking and run :

``` shell
make upload
```
If everything went well the esp32 connected will be programmed and you will be able to see your robot live on app.viam.com

## Building for QEMU
Navigate to the root of the Mini-RDK repository, once here run  `. $HOME/esp/esp-idf/export.sh` if you haven't done so already.
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
