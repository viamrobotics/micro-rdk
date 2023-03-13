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

- Discord: <https://discord.gg/viam>
- Support: <https://support.viam.com>

## (In)stability Notice

> **Warning**
> This is an alpha release of the Viam Micro-RDK. Stability is not guaranteed. Breaking changes are likely to occur, and occur often.

For documentation of the Micro-RDK, see the [Viam Documentation](https://docs.viam.com/installation/microcontrollers/).



## License

Copyright 2022-2023 Viam Inc.

AGPLv3 - See [LICENSE](https://github.com/viamrobotics/micro-rdk/blob/main/LICENSE) file
