FROM rust:1.70.0-bullseye AS builder
RUN apt update && apt-get install -y --no-install-recommends git libudev-dev make bzip2 gcc libgcrypt20 libgcrypt20-dev  ninja-build libudev-dev sudo wget git flex bison gperf ccache libffi-dev libssl-dev dfu-util libusb-1.0-0 cmake pkg-config libglib2.0-dev libpixman-1-dev software-properties-common openssh-client libslirp-dev

RUN cargo install cargo-binstall
RUN cargo binstall cargo-espflash@2.0.0-rc.1 --install-path /usr/ -y
RUN cargo binstall espup --install-path /usr/ -y
RUN cargo install ldproxy --root /usr/ 


WORKDIR /qemu
RUN git clone --depth 1 --branch esp-develop https://github.com/espressif/qemu
RUN cd qemu && mkdir build && cd build && ../configure --target-list=xtensa-softmmu  --enable-gcrypt --enable-slirp \
    --disable-sanitizers  --disable-strip --disable-user \
    --disable-capstone --disable-vnc --disable-sdl --disable-gtk && ninja qemu-system-xtensa

WORKDIR /esp

RUN git clone --depth 1 -b v4.4.4 --single-branch --recurse-submodules --shallow-submodules https://github.com/npmenard/esp-idf


FROM debian:bullseye

ARG RUST_VERSION=1.67.0

ENV DEBIAN_FRONTEND noninteractive
ENV DEBCONF_NOWARNINGS="yes"

ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH=/opt/rust/cargo/bin:$PATH \
    ESP_ROOT=/opt/esp \
    IDF_TOOLS_PATH=/opt/esp/toolchain \
    RUST_VERSION=$RUST_VERSION

RUN apt update && apt dist-upgrade -y

RUN echo "deb http://deb.debian.org/debian bullseye-backports main" >> /etc/apt/sources.list.d/backports.list

RUN apt update 

RUN apt-get install -y --no-install-recommends git wget flex bison gperf python3 python3-pip python3-venv cmake ninja-build ccache libffi-dev libssl-dev dfu-util libusb-1.0-0 openssh-client software-properties-common sudo && \
    apt-add-repository 'deb http://deb.debian.org/debian bullseye-backports main' && \
    apt update && \
    apt-get install -y --no-install-recommends cmake/bullseye-backports && \
    update-alternatives --install /usr/bin/python python /usr/bin/python3 10 && \
    python -m pip install --upgrade pip virtualenv


RUN RUST_VERSION=$RUST_VERSION \
    set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) rustArch='x86_64-unknown-linux-gnu'; rustupSha256='bb31eaf643926b2ee9f4d8d6fc0e2835e03c0a60f34d324048aa194f0b29a71c' ;; \
        armhf) rustArch='armv7-unknown-linux-gnueabihf'; rustupSha256='6626b90205d7fe7058754c8e993b7efd91dedc6833a11a225b296b7c2941194f' ;; \
        arm64) rustArch='aarch64-unknown-linux-gnu'; rustupSha256='4ccaa7de6b8be1569f6b764acc28e84f5eca342f5162cd5c810891bff7ed7f74' ;; \
        i386) rustArch='i686-unknown-linux-gnu'; rustupSha256='34392b53a25c56435b411d3e575b63aab962034dd1409ba405e708610c829607' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    url="https://static.rust-lang.org/rustup/archive/1.25.2/${rustArch}/rustup-init"; \
    wget "$url"; \
    echo "${rustupSha256} *rustup-init" | sha256sum -c -; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain ${RUST_VERSION} --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME

COPY --from=builder /usr/cargo-espflash /usr/bin
COPY --from=builder /usr/bin/ldproxy /usr/bin
COPY --from=builder /usr/espup /usr/bin
   
WORKDIR /host/


RUN useradd -s /bin/bash -m testbot && \
    echo 'testbot ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers && \
    echo "Defaults   secure_path=\"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${CARGO_HOME}/bin\"" >> /etc/sudoers.d/environment && \
    echo 'Defaults   env_keep+="CARGO_HOME RUSTUP_HOME IDF_PATH IDF_TOOLS_PATH ESP_ROOT"' >> /etc/sudoers.d/environment

RUN sudo chown -R testbot:testbot $CARGO_HOME &&\
    sudo chown -R testbot:testbot $RUSTUP_HOME

USER testbot

RUN sudo mkdir -p $ESP_ROOT &&\
    sudo chown -R testbot:testbot $ESP_ROOT

COPY --from=builder /esp $ESP_ROOT/

RUN sudo chmod -R a+rwx $ESP_ROOT

RUN git config --global --add safe.directory $ESP_ROOT/esp-idf

ENV IDF_PATH=$ESP_ROOT/esp-idf

RUN $IDF_PATH/tools/idf_tools.py --non-interactive install required  && $IDF_PATH/tools/idf_tools.py --non-interactive install cmake   && $IDF_PATH/tools/idf_tools.py --non-interactive install-python-env  && rm -rf $IDF_TOOLS_PATH/dist

RUN espup install -s -f $ESP_ROOT/export-esp.sh -v ${RUST_VERSION}
RUN rustup component add clippy-preview
RUN rustup component add rustfmt

RUN rm -rf $CARGO_HOME/registry

RUN echo "PATH=$PATH" >> ~/.bash_profile && \
    echo ". $ESP_ROOT/export-esp.sh" >> ~/.bash_profile && \
    echo ". $IDF_PATH/export.sh"  >> ~/.bash_profile && \
    echo "CARGO_HOME=/host/.cargo-docker-registry" >> ~/.bash_profile

USER root

RUN apt-get install make build-essential -y && apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /qemu/qemu/build/qemu-system-xtensa /usr/bin

