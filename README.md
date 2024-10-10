Table of Contents
=================

- [Table of Contents](#table-of-contents)
- [Micro-RDK (Robot Development Kit for Microcontrollers)](#micro-rdk-robot-development-kit-for-microcontrollers)
  - [Contact](#contact)
  - [(In)stability Notice](#instability-notice)
  - [Getting Started](#getting-started)
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

If you have a bug or an idea, please open an issue [here](https://viam.atlassian.net/servicedesk/customer/portal/7).

## (In)stability Notice

> **Warning**
> This is a beta release of the Viam Micro-RDK. Stability is not guaranteed. Breaking changes are likely to occur, and occur often.

## Getting Started

For instructions on how to install the Micro-RDK, see the [Viam Installation Documentation](https://docs.viam.com/installation/microcontrollers/).

For instructions on how to get started with Micro-RDK development tasks,
such as module development or usage, please see the [Development Setup Documentation](https://docs.viam.com/installation/viam-micro-server-dev/).

For documentation on advanced development configuration, including
working across multiple projects, sharing local changes, credentials
management, build performance, and customizing ESP-IDF configuration,
please see [DEVELOPMENT.md](DEVELOPMENT.md) in this repository.

## Debugging

### Viewing server logs

The following instructions should be used for viewing server logs from an esp32 in terminal. These logs should be copied and included when contacting Viam support.

#### Using espflash

To see server logs for an esp32, use the `monitor` command on `espflash`:

```
espflash monitor
```

#### Without espflash

In the event that cargo and/or espflash is not installed, the [micro-rdk-installer](https://github.com/viamrobotics/micro-rdk/tree/main/micro-rdk-installer) also contains the monitor command and can be downloaded as an alternative.
Here is an example using the x86_64-Linux version:

```
./micro-rdk-installer-amd64-linux monitor
```

## License

Copyright 2022-2023 Viam Inc.

AGPLv3 - See [LICENSE](https://github.com/viamrobotics/micro-rdk/blob/main/LICENSE) file
