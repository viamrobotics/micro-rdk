FROM rust:1.75-bookworm AS builder
RUN apt update && apt-get install -y --no-install-recommends git libudev-dev make bzip2 gcc libgcrypt20 libgcrypt20-dev  ninja-build libudev-dev sudo wget git flex bison gperf ccache libffi-dev libssl-dev dfu-util libusb-1.0-0 cmake pkg-config libglib2.0-dev libpixman-1-dev software-properties-common openssh-client libslirp-dev python3-venv

RUN export TARGET=$(rustc -vV | sed -En 's/^host: //p') && \
    curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash && \ 
    cargo binstall --pkg-url='https://github.com/esp-rs/espflash/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }' --pkg-fmt='zip' cargo-espflash@3.0.0 --install-path /usr -y --target $TARGET && \
    cargo binstall --pkg-url='https://github.com/esp-rs/espup/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }' --pkg-fmt='zip' espup@0.11 --install-path /usr -y --target $TARGET && \
    cargo binstall --pkg-url='https://github.com/esp-rs/embuild/releases/download/{ name }-v{ version }/{ name }-{ target }{ archive-suffix }' --pkg-fmt='zip' ldproxy@0.3.2 --install-path /usr -y --target $TARGET && \
    cargo binstall --pkg-url='https://github.com/mozilla/sccache/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }' --pkg-fmt='tgz' sccache@0.7.7 --install-path /usr -y --target $TARGET

WORKDIR /qemu
RUN git clone --depth 1 --branch esp-develop https://github.com/espressif/qemu
RUN cd qemu && mkdir build && cd build && ../configure --target-list=xtensa-softmmu  --enable-gcrypt --enable-slirp \
    --disable-sanitizers  --disable-strip --disable-user \
    --disable-capstone --disable-vnc --disable-sdl --disable-gtk && ninja qemu-system-xtensa

WORKDIR /esp

RUN git clone --depth 1 -b v4.4.8 --single-branch --recurse-submodules --shallow-submodules https://github.com/espressif/esp-idf/


FROM debian:bookworm

ARG RUST_VERSION=1.75.0

ENV DEBIAN_FRONTEND=noninteractive
ENV DEBCONF_NOWARNINGS="yes"

ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH=/opt/rust/cargo/bin:$PATH \
    ESP_ROOT=/opt/esp \
    IDF_TOOLS_PATH=/opt/esp/toolchain \
    RUST_VERSION=$RUST_VERSION

RUN apt update && apt dist-upgrade -y

RUN apt update

RUN apt-get install -y --no-install-recommends git wget flex bison gperf python3 python3-pip python3-virtualenv cmake ninja-build ccache libffi-dev libssl-dev libudev-dev dfu-util libusb-1.0-0 openssh-client software-properties-common python3-venv sudo curl && \
    apt-get install -y --no-install-recommends cmake && \
    update-alternatives --install /usr/bin/python python /usr/bin/python3 10


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
COPY --from=builder /usr/ldproxy /usr/bin
COPY --from=builder /usr/espup /usr/bin
COPY --from=builder /usr/sccache /usr/bin

WORKDIR /host/

RUN useradd -s /bin/bash -m testbot && \
    echo 'testbot ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers && \
    echo "Defaults   secure_path=\"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${CARGO_HOME}/bin\"" >> /etc/sudoers.d/environment && \
    echo 'Defaults   env_keep+="CARGO_HOME RUSTUP_HOME IDF_PATH IDF_TOOLS_PATH ESP_ROOT"' >> /etc/sudoers.d/environment

RUN chown -R testbot:testbot $CARGO_HOME &&\
    chown -R testbot:testbot $RUSTUP_HOME

RUN mkdir -p $ESP_ROOT && \
    chown -R testbot:testbot $ESP_ROOT

RUN apt-get install -y ruby && gem install license_finder

USER testbot

COPY --from=builder /esp $ESP_ROOT/

ENV IDF_PATH=$ESP_ROOT/esp-idf

RUN $IDF_PATH/tools/idf_tools.py --non-interactive install required  && $IDF_PATH/tools/idf_tools.py --non-interactive install cmake   && $IDF_PATH/tools/idf_tools.py --non-interactive install-python-env  && rm -rf $IDF_TOOLS_PATH/dist

RUN espup install -s -f $ESP_ROOT/export-esp.sh -v ${RUST_VERSION}
RUN rustup component add clippy-preview
RUN rustup component add rustfmt

RUN git config --global --add safe.directory '*'

RUN rm -rf $CARGO_HOME/registry

RUN echo "PATH=$PATH" >> ~/.bash_profile && \
    echo ". $ESP_ROOT/export-esp.sh" >> ~/.bash_profile && \
    echo ". $IDF_PATH/export.sh"  >> ~/.bash_profile && \
    echo "CARGO_HOME=/host/.micro-rdk-docker-caches/cargo-registry" >> ~/.bash_profile && \
    echo "[[ -v SCCACHE ]] && export RUSTC_WRAPPER=/usr/bin/sccache" >> ~/.bash_profile && \
    echo "[[ -v SCCACHE ]] && export SCCACHE_DIR=/host/.micro-rdk-docker-caches/sccache" >> ~/.bash_profile

USER root

RUN apt-get install make build-essential pkg-config -y && apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /qemu/qemu/build/qemu-system-xtensa /usr/bin

RUN chmod -R a+rwx $ESP_ROOT && \
    chown -R testbot:testbot $ESP_ROOT

RUN git config --global --add safe.directory '*'
