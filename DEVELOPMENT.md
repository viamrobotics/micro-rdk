# Viam Micro-RDK Development

## (In)stability Notice

> **Warning** The Viam Micro-RDK is currently in beta.

## Overview

This document provides advanced development configuration notes to aid
in the development of Micro-RDK modules and projects, or for working
on the Micro-RDK itself.

- If you are interested in just installing and using the Micro-RDK,
  please see the [Viam Installation Documentation](https://docs.viam.com/installation/microcontrollers/).

- These instructions and notes assume a working Micro-RDK development
  environment. Please see the [Development Setup Documentation](https://docs.viam.com/installation/viam-micro-server-dev/)
  before proceeding.

## Micro-RDK Development Tips

### Tree-level `cargo` configuration

The micro-rdk monorepo is only part of the development story: when
developing for or with Micro-RDK, there will be other directories in
play (e.g. projects generated from the template, etc.), and
coordinating the state of these many directories and repositories can
prove troublesome. A carefully placed `.cargo/config.toml` file can
inject settings for the entire tree of trees, including the micro-rdk
monorepo, projects, and modules. Simply create a `.cargo` directory as
a peer of your micro-rdk monorepo and any project directories, and
then create a `config.toml` file within it. Settings applied in that
file will now apply globally to all crates inside the micro-rdk as
well as to peer project directories. Here is an example layout:

```
.
├── .cargo
│   └── config.toml
├── micro-rdk
│   ├── ...
│   ├── examples
│   ├── micro-rdk
│   ├── micro-rdk-ffi
│   ├── micro-rdk-installer
│   ├── micro-rdk-macros
│   ├── micro-rdk-server
│   └── templates
└── projects
    ├── module-under-development
    ├── another-module
    └── module-consuming-project
```

Several of the remaining development tips in this document make use of
this functionality.

### Using `sccache` to improve build times

Use [sccache](https://github.com/mozilla/sccache) to cache the result
of rust compiles for an overall faster experience as you move back and
forth between branches. Install it with `cargo install sccache` and
then you can opt into it by setting the `RUSTC_WRAPPER` environment
variable to point to `sccache`, e.g. `RUSTC_WRAPPER=/path/to/sccache
cargo build ...`. But it can be easy to forget to configure
that. Instead, you can add the following to a `.cargo/config.toml`
(either in `$HOME/.cargo/config.toml` to apply it everywhere, or in a
tree-level `.cargo/config.toml` per above):


```
[build]
rustc-wrapper = "/path/to/sccache"
[profile.dev.package."*"]
incremental = false
```

Note that this disables incremental compilation for dev builds because
`sccache` is incompatible with incremental compilation.

### Fixing `+esp` Builds on MacOS

#### Use `CRATE_CC_NO_DEFAULTS`

If you are building `Micro-RDK` for ESP32 on a macOS machine and you
receive an error like the following:

```
xtensa-esp32-elf-gcc: error: unrecognized command line option '--target=xtensa-esp32-espidf'
```

Then [work around](https://github.com/esp-rs/esp-idf-template/issues/174) this
issue by setting `CRATE_CC_NO_DEFAULTS=1` in your environment
(e.g. `make build-esp32-bin CRATE_CC_NO_DEFAULTS=1` or
`CRATE_CC_NO_DEFAULTS=1 cargo +esp build ...`. Or, you can use the
tree-level Cargo configuration (e.g.  `.cargo/config.toml` as above)
to just take care of this for you:


```
[env]
CRATE_CC_NO_DEFAULTS = { value = "1" }
```

It might be better to scope that to only apply for `+esp` builds: if
you know the syntax for that, please submit a PR to this
repository. It is possible that a resolution of [this upstream `cargo `issue](https://github.com/rust-lang/cargo/issues/10273)
 may be a prerequisite.

#### Homebrew Python is not usable by default

The Python packaging made available with Homebrew [is not intended for
end-user
consumption](https://justinmayer.com/posts/homebrew-python-is-not-for-you/). Attempts
to install packages outside of a virtual environment of some sort will
result in messages like the following:

```
$ python3 -m pip install foo
error: externally-managed-environment
× This environment is externally managed
╰─> To install Python packages system-wide, try brew install
    xyz, where xyz is the package you are trying to
    install.

    If you wish to install a non-brew-packaged Python package,
    create a virtual environment using python3 -m venv path/to/venv.
    Then use path/to/venv/bin/python and path/to/venv/bin/pip.

    If you wish to install a non-brew packaged Python application,
    it may be easiest to use pipx install xyz, which will manage a
    virtual environment for you. Make sure you have pipx installed.

note: If you believe this is a mistake, please contact your Python installation or OS distribution provider. You can override this, at the risk of breaking your Python installation or OS, by passing --break-system-packages.
hint: See PEP 668 for the detailed specification.
```

Unfortunately, the ESP-IDF ecosystem explicitly depends on being able
to install python packages for its own ends, specifically, the
`virtualenv` package.

This conflict is unfortunate, and there are only two somewhat unsavory
paths forward:

- Use a different Python installation which is not externally
  managed. There are many ways to obtain a version of Python
  independent from the one in Homebrew which will not prevent ESP-IDF
  from installing the packages it wants to install (e.g. with
  [`asdf`](https://asdf-vm.com/)).

- Forcibly pre-install the `virtualenv` package by running `python3
  -m pip install --user --break-system-packages virtualenv`. This will
  permit the ESP-IDF build scripts to skip the attempt to install
  `virtualenv` because it is already installed. However, per the scary
  name of the flag, it risks breaking system packages.

### Patching in the local `micro-rdk` Monorepo at Tree Level

Again, if you are working with Micro-RDK, you probably have both the
`Micro-RDK` monorepo and various project directories associated with
it. Those projects quite often have their own `Cargo.toml` files that
have dependencies on micro-rdk by way of github:


```
[dependencies]
...
micro-rdk = {version = "0.2.2", git = "https://github.com/viamrobotics/micro-rdk.git", features = ["esp32", "binstart","provisioning"], rev = "a1863c9" }
micro-rdk-modular-driver-example = { version = "0.2.2", git = "https://github.com/viamrobotics/micro-rdk", rev = "a1863c9" }
```

But just as often, what you actually want to do is to have these
projects use your __local_ micro-rdk state, and you end up hand
editing the `Cargo.toml` file to point to a path instead:


```
[dependencies]
...
micro-rdk = { path = "../micro-rdk/micro-rdk, features = ["esp32", "binstart","provisioning"] }
micro-rdk-modular-driver-example = { path = "../micro-rdk/examples/modular-drivers" }
```

It is a nuisance to maintain these edits: avoiding checking them in,
keeping them in stashes, replicating them across many `Cargo.toml`
files, etc. Instead, use the tree-level `.cargo/config.toml` to point
to your local tree using patch:

```
[patch.'https://github.com/viamrobotics/micro-rdk.git']
micro-rdk = { path = "./micro-rdk/micro-rdk" }
micro-rdk-ffi = { path = "./micro-rdk/micro-rdk-ffi" }
micro-rdk-macros = { path = "./micro-rdk/micro-rdk-macros" }
micro-rdk-modular-driver-example = { path = "./micro-rdk/examples/modular-drivers" }
```

Note that you need to declare each library crate that a dependency
might want to link against individually. Also be careful with pathing,
as it appears that the paths are interpreted relative to the siting of
the `.cargo` directory, not the `.cargo/config.toml` file (i.e. you do
not need a leading `../` to pop up a directory level. Now all projects
under the affected tree will always use the local Micro-RDK state
without needing local `Cargo.toml` edits.

### Customizing ESP-IDF configuration

The documented build procedures for Micro-RDK and Micro-RDK adjacent
projects leverage the [`embuild` package](https://github.com/esp-rs/embuild)
to automate the installation of the ESP-IDF framework. The default
settings are usually what you want, but there are times when it may
make sense to customize them.

Please review the [`esp-idf-sys` build options documentation](https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md#esp-idf-configuration) for details on what things are available to customize.

Some values are defaulted in the Micro-RDK top-level `Cargo.toml`, but
these values may be overridden with environment variables on the
command line or as an argument to `make`. Another path to
customization is to add settings to the `[env]` section of a
tree-level `.cargo/config.toml` file.

Values of particular interest include:

- [`esp_idf_tools_install_dir` / `$ESP_IDF_TOOLS_INSTALL_DIR`](https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md#esp_idf_tools_install_dir-esp_idf_tools_install_dir),
  to customize the location where ESP-IDF trees will be installed, if
  you do not like the default of installing them under the `target`
  directory. Note in particular the `fromenv` value, which should be
  used if you have a dedicated ESP-IDF installation already installed
  and activated that you wish to use.
- [`esp_idf_version` / `$ESP_IDF_VERSION`](https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md#esp_idf_version-esp_idf_version-native-builder-only),
  to override the version of ESP-IDF that will be used to build your
  project or the Micro-RDK.
- [`mcu` / `$MCU`](https://github.com/esp-rs/esp-idf-sys/blob/master/BUILD-OPTIONS.md#mcu-mcu),
  to select a different target for the build, rather than the default
  `esp32` MCU target.

### Automatically Opting-in to `ESP_IDF_TOOLS_INSTALL_DIR=fromenv`

If you prefer to always use a manually configured ESP-IDF environment
rather than relying on `embuild`s automated ESP-IDF installation, you
can do this permanently for all rust development by adding the
following to your treel-level `.cargo/config.toml`:

```
[env]
ESP_IDF_TOOLS_INSTALL_DIR = { value = "fromenv" }
```

Once this setting is in play, there may be configuration scripts that
need to be run to tell the build system where to find the standalone
ESP-IDF installation.

## Explicitly Configuring Wifi and Robot Credentials

Both the Micro-RDK and projects generated from the project template
honor environment variables to statically configure WiFi credentials
via `MICRO_RDK_WIFI_SSID` and `MICRO_RDK_WIFI_PASSWORD`. These values
are examined at build time, so if you wish to change them you will need to
rebuild and reflash your device for them to have effect.

Similarly, the `micro-rdk-server` project and projects generated from
the project template will expect to find robot identity and credential
information in a file called `viam.json`. This file is, like the WiFi
credentials, used at build time, so if you add or remove it or change
the contents, you will need to rebuild and reflash in order for the
changes to have effect.

## Development with Viam's `canon` Infrastructure and Docker

Viam provides a Docker image with a pre-configured Micro-RDK
development environment, and tooling to automate use of that image,
called `canon`. The primary use case for the `canon`-based workflow is
CI and release builds of the Micro-RDK. However, it is also possible
to use these images for development, if the other ways are proving
difficult.

1. Install `canon`, per the [installation instructions](https://github.com/viamrobotics/canon?tab=readme-ov-file#installation)

2. Run rust development tasks inside the container by launching them
   as a `bash` shell command:

```
$ canon bash -lc "make build-esp32-bin"
```

3. Run flashing and monitoring commands _outside_ cannon, so they have
   access to things like local USB ports:

```
$ make flash-esp32-bin
```
