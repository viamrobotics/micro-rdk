# Mini-RDK
The Mini-RDK is a lightweight version of Viam's RDK, it's goal is to be ran on resource limited embedded systems. The only embedded system currently supported is the ESP32.

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
``` shell
git clone https://github.com/espressif/qemu
cd qemu
./configure --target-list=xtensa-softmmu     --enable-gcrypt \
    --enable-debug --enable-sanitizers  --disable-strip --disable-user \
    --disable-capstone --disable-vnc --disable-sdl --disable-gtk
cd build && ninja
```

Add `export QEMU_ESP32_XTENSA=<path-to-clone-qemu>/build/` to your `.zshrc` or `.bashrc`

## Building for ESP32
Navigate to the root of the Mini-RDK repository, once here run  `. $HOME/esp/esp-idf/export.sh` if you haven't done so already.
You will need to uncomment two lines from the file `sdkconfig.defaults` if they are not already uncommented

``` editorconfig
#CONFIG_ESPTOOLPY_FLASHFREQ_80M=y
#CONFIG_ESPTOOLPY_FLASHMODE_QIO=y
```
You can then run (with your esp32 connected)
``` shell
make build
make upload
```

## Building for QEMU
Navigate to the root of the Mini-RDK repository, once here run  `. $HOME/esp/esp-idf/export.sh` if you haven't done so already.
You will need to comment two lines from the file `sdkconfig.defaults`

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
