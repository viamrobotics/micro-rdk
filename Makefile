SHELL := /bin/bash
ESPFLASHVERSION_MAJ := $(shell expr `cargo espflash -V | grep ^cargo-espflash | sed 's/^.* //g' | cut -f1 -d. `)
ESPFLASHVERSION_MIN := $(shell expr `cargo espflash -V | grep ^cargo-espflash | sed 's/^.* //g' | cut -f2 -d. `)
ESPFLASHVERSION := $(shell [ $(ESPFLASHVERSION_MAJ) -gt 2 -a $(ESPFLASHVERSION_MIN) -ge 2 ] && echo true)
VIAM_API_VERSION := v0.1.336

DATE := $(shell date +%F)
IMAGE_BASE = ghcr.io/viamrobotics/micro-rdk-dev-env

default: build-esp32-bin

clean:
	cargo clean

all: clean build-esp32-bin build-native

buf-clean:
	find micro-rdk/src/gen -type f \( -iname "*.rs" \) -delete

buf: buf-clean
	buf generate buf.build/viamrobotics/goutils --template micro-rdk/buf.gen.yaml
	buf generate buf.build/googleapis/googleapis --template micro-rdk/buf.gen.yaml --path google/rpc --path google/api
	buf generate buf.build/viamrobotics/api:${VIAM_API_VERSION} --template micro-rdk/buf.gen.yaml
	printf "// AUTO-GENERATED CODE; DO NOT DELETE OR EDIT\npub const VIAM_API_VERSION: &str = \"${VIAM_API_VERSION}\";\n" > micro-rdk/src/gen/api_version.rs
	buf generate buf.build/protocolbuffers/wellknowntypes --template micro-rdk/buf.gen.yaml

license-finder:
	license_finder

cargo-ver:
ifneq ($(ESPFLASHVERSION),true)
		$(error Update espfash to version >=3.0. Update with cargo install cargo-espflash)
endif

build:
	cargo +esp build  -p micro-rdk-server --bin micro-rdk-server-esp32 --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort

build-native:
	cargo build -p micro-rdk-server --bin micro-rdk-server-native

build-installer:
	cargo build -p micro-rdk-installer --bin micro-rdk-installer --release

native:
	cargo run -p micro-rdk-server --bin micro-rdk-server-native

build-qemu:
	cargo +esp build -p micro-rdk-server --bin micro-rdk-server-esp32  --features qemu --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort && cargo +esp espflash save-image --package micro-rdk-server --features qemu --merge --chip esp32 target/xtensa-esp32-espidf/debug/debug.bin -T micro-rdk-server/esp32/partitions.csv -s 4mb  --bin micro-rdk-server-esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort


sim-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205,hostfwd=tcp::12346-:12346 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -m 4m

# debug-local is identical to sim-local, except the `-S` at the end means "wait until a debugger is
# attached before starting."
debug-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205,hostfwd=tcp::12346-:12346 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -S -m 4m

upload: cargo-ver
	cargo +esp espflash flash --package micro-rdk-server --monitor --partition-table micro-rdk-server/esp32/partitions.csv --baud 460800 -f 80mhz --bin micro-rdk-server-esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort

test:
	cargo test -p micro-rdk --lib --features native,ota

clippy-native:
	cargo clippy -p micro-rdk --no-deps --features native,ota  -- -Dwarnings

clippy-esp32:
	cargo +esp clippy -p micro-rdk  --features esp32,ota  --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort -- -Dwarnings

clippy-cli:
	cargo clippy -p micro-rdk-installer --no-default-features -- -Dwarnings

clippy-ffi-native:
	cargo clippy -p micro-rdk-ffi -- -Dwarnings

clippy-ffi-esp32:
	cargo +esp clippy -p micro-rdk-ffi  --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort -- -Dwarnings

clippy-ffi : clippy-ffi-native clippy-ffi-esp32

format:
	cargo fmt --all -- --check

doc:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --workspace --exclude micro-rdk-macros

doc-open:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --open

size:
	find . -name "esp-build.map" -exec ${IDF_PATH}/tools/idf_size.py {} \;

build-esp32-bin: build-esp32-ota
	cargo +esp espflash save-image \
		--skip-update-check \
		--package=micro-rdk-server \
		--features=ota \
		--chip=esp32 \
		--partition-table=micro-rdk-server/esp32/ota_8mb_partitions.csv \
		--flash-size=8mb \
		--bin=micro-rdk-server-esp32 \
		--target=xtensa-esp32-espidf \
		-Zbuild-std=std,panic_abort --release \
		target/xtensa-esp32-espidf/micro-rdk-server-esp32.bin \
		--merge

build-esp32-ota:
	cargo +esp espflash save-image \
		--skip-update-check \
		--package=micro-rdk-server \
		--features=ota \
		--chip=esp32 \
		--bin=micro-rdk-server-esp32 \
		--partition-table=micro-rdk-server/esp32/ota_8mb_partitions.csv \
		--target=xtensa-esp32-espidf \
		-Zbuild-std=std,panic_abort --release \
		./target/xtensa-esp32-espidf/micro-rdk-server-esp32-ota.bin

serve-ota: build-esp32-ota
	cargo r --package ota-server

flash-esp32-bin:
ifneq (,$(wildcard ./target/xtensa-esp32-espidf/micro-rdk-server-esp32.bin))
	espflash write-bin 0x0 ./target/xtensa-esp32-espidf/micro-rdk-server-esp32.bin --baud 460800  && sleep 2 && espflash monitor
else
	$(error micro-rdk-server-esp32.bin not found, run make build-esp32-bin first)
endif

canon-image: canon-image-amd64 canon-image-arm64

canon-image-amd64:
	cd etc/docker && docker buildx build . --load --no-cache --platform linux/amd64 -t $(IMAGE_BASE):amd64

canon-image-arm64:
	cd etc/docker && docker buildx build . --load --no-cache --platform linux/arm64 -t $(IMAGE_BASE):arm64

canon-upload: canon-upload-amd64 canon-upload-arm64

canon-upload-amd64:
	docker tag $(IMAGE_BASE):amd64 $(IMAGE_BASE):amd64_$(DATE)
	docker push $(IMAGE_BASE):amd64
	docker push $(IMAGE_BASE):amd64_$(DATE)

canon-upload-arm64:
	docker tag $(IMAGE_BASE):arm64 $(IMAGE_BASE):arm64_$(DATE)
	docker push $(IMAGE_BASE):arm64
	docker push $(IMAGE_BASE):arm64_$(DATE)
