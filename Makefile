SHELL := /bin/bash
VIAM_API_VERSION := v0.1.394

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


sim-local: build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205,hostfwd=tcp::12346-:12346 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -m 4m

# debug-local is identical to sim-local, except the `-S` at the end means "wait until a debugger is
# attached before starting."
debug-local: build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205,hostfwd=tcp::12346-:12346 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -S -m 4m

test:
	cargo test --workspace --tests --no-fail-fast --features native

clippy-native:
	cargo clippy --workspace --no-deps --locked --all-targets --features native  -- -Dwarnings

clippy-esp32:
	cargo +esp clippy -p micro-rdk-server --bin micro-rdk-server-esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --release -- -Dwarnings
	cargo +esp clippy -p micro-rdk-ffi --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --release -- -Dwarnings

format:
	cargo fmt --all -- --check

doc:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --workspace --exclude micro-rdk-macros

doc-open:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --open

build-esp32-bin:
	cargo +esp espflash save-image \
		--skip-update-check \
		--package=micro-rdk-server \
		--chip=esp32 \
		--bin=micro-rdk-server-esp32 \
		--partition-table=micro-rdk-server/esp32/partitions.csv \
		--target=xtensa-esp32-espidf	 \
		-Zbuild-std=std,panic_abort --release \
		--flash-size=8mb \
		--merge \
		target/xtensa-esp32-espidf/micro-rdk-server-esp32.bin

build-esp32-ota:
	cargo +esp espflash save-image \
		--skip-update-check \
		--package=micro-rdk-server \
		--chip=esp32 \
		--bin=micro-rdk-server-esp32 \
		--partition-table=micro-rdk-server/esp32/partitions.csv \
		--target=xtensa-esp32-espidf \
		-Zbuild-std=std,panic_abort --release \
		target/xtensa-esp32-espidf/micro-rdk-server-esp32-ota.bin

serve-ota:
	cargo r --package ota-dev-server

flash-esp32-bin:
ifneq (,$(wildcard ./target/xtensa-esp32-espidf/micro-rdk-server-esp32.bin))
	espflash write-bin 0x0 ./target/xtensa-esp32-espidf/micro-rdk-server-esp32.bin --baud 460800  && sleep 2 && espflash monitor
else
	$(error micro-rdk-server-esp32.bin not found, run make build-esp32-bin first)
endif

