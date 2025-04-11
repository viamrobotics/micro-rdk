FROM debian:bookworm

ARG RUST_VERSION=1.85.0

ENV DEBIAN_FRONTEND=noninteractive
ENV DEBCONF_NOWARNINGS="yes"

ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH=/opt/rust/cargo/bin:$PATH \
    ESP_ROOT=/opt/esp \
    IDF_TOOLS_PATH=/opt/esp/toolchain \
    RUST_VERSION=$RUST_VERSION

RUN apt update && apt dist-upgrade -y

RUN apt-get install -y --no-install-recommends make build-essential pkg-config git wget flex bison gperf python3 python3-pip python3-virtualenv cmake ninja-build ccache libffi-dev libssl-dev libudev-dev dfu-util libusb-1.0-0 openssh-client software-properties-common python3-venv sudo curl libpixman-1-0 libslirp0 libsdl2-2.0-0 xz-utils && \
    apt-get install -y --no-install-recommends cmake && \
    apt-get install -y --no-install-recommends jq && \
    update-alternatives --install /usr/bin/python python /usr/bin/python3 10

RUN set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) qemuSha='071d117c44a6e9a1bc8664ab63b592d3e17ceb779119dcb46c59571a4a7a88c9'; qemuUrl='https://github.com/espressif/qemu/releases/download/esp-develop-9.0.0-20240606/qemu-xtensa-softmmu-esp_develop_9.0.0_20240606-x86_64-linux-gnu.tar.xz' ;; \
        arm64) qemuSha='43552f32b303a6820d0d9551903e54fc221aca98ccbd04e5cbccbca881548008';qemuUrl='https://github.com/espressif/qemu/releases/download/esp-develop-9.0.0-20240606/qemu-xtensa-softmmu-esp_develop_9.0.0_20240606-aarch64-linux-gnu.tar.xz' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    wget "$qemuUrl" -O qemu; \
    echo "${qemuSha} *qemu" | sha256sum -c -; \
    tar -C /usr  --strip-components=1 -xf qemu ; \
    rm  qemu

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

RUN export TARGET=$(rustc -vV | sed -En 's/^host: //p') && \
    curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash && \
    cargo binstall --pkg-url='https://github.com/esp-rs/espflash/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }' --pkg-fmt='zip' cargo-espflash@3.2.0 --install-path /usr/bin/ -y --target $TARGET && \
    cargo binstall --pkg-url='https://github.com/esp-rs/espup/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }' --pkg-fmt='zip' espup@0.11 --install-path /usr/bin/ -y --target $TARGET && \
    cargo binstall --pkg-url='https://github.com/esp-rs/embuild/releases/download/{ name }-v{ version }/{ name }-{ target }{ archive-suffix }' --pkg-fmt='zip' ldproxy@0.3.2 --install-path /usr/bin/ -y --target $TARGET && \
    cargo binstall --pkg-url='https://github.com/mozilla/sccache/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }' --pkg-fmt='tgz' sccache@0.7.7 --install-path /usr/bin/ -y --target $TARGET

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

RUN git clone --depth 1 -b v5.4.1 --single-branch --recurse-submodules --shallow-submodules https://github.com/espressif/esp-idf/ $ESP_ROOT/esp-idf

ENV IDF_PATH=$ESP_ROOT/esp-idf

RUN $IDF_PATH/tools/idf_tools.py --non-interactive install required  && $IDF_PATH/tools/idf_tools.py --non-interactive install cmake   && $IDF_PATH/tools/idf_tools.py --non-interactive install-python-env  && rm -rf $IDF_TOOLS_PATH/dist

RUN espup install -s -f $ESP_ROOT/export-esp.sh -v ${RUST_VERSION}
RUN rustup component add clippy-preview
RUN rustup component add rustfmt

RUN rm -rf $CARGO_HOME/registry

USER root

RUN echo "PATH=$PATH" >> /etc/profile && \
    echo ". $ESP_ROOT/export-esp.sh" >> /etc/profile && \
    echo ". $IDF_PATH/export.sh"  >> /etc/profile && \
    echo "export ESP_IDF_TOOLS_INSTALL_DIR=fromenv"  >> /etc/profile && \
    echo "CARGO_HOME=/host/.micro-rdk-docker-caches/cargo-registry" >> /etc/profile && \
    echo "[[ -v SCCACHE ]] && export RUSTC_WRAPPER=/usr/bin/sccache" >> /etc/profile && \
    echo "[[ -v SCCACHE ]] && export SCCACHE_DIR=/host/.micro-rdk-docker-caches/sccache" >> /etc/profile

RUN apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

RUN git config --global --add safe.directory '*'
