SHELL := /bin/bash
ESPFLASHVERSION_MAJ := $(shell expr `cargo espflash -V | grep ^cargo-espflash | sed 's/^.* //g' | cut -f1 -d. `)
ESPFLASHVERSION_MIN := $(shell expr `cargo espflash -V | grep ^cargo-espflash | sed 's/^.* //g' | cut -f2 -d. `)
ESPFLASHVERSION := $(shell [ $(ESPFLASHVERSION_MAJ) -gt 1 -a $(ESPFLASHVERSION_MIN) -ge 1 ] && echo true)

DATE := $(shell date +%F)
IMAGE_BASE = ghcr.io/viamrobotics/micro-rdk-dev-env

default: build-esp32-bin

clean:
	cargo clean

all: clean build-esp32-bin build-native build-esp32-with-cred-bin

buf-clean:
	find micro-rdk/src/gen -type f \( -iname "*.rs" \) -delete

buf: buf-clean
	buf generate buf.build/viamrobotics/goutils --template micro-rdk/buf.gen.yaml
	buf generate buf.build/googleapis/googleapis --template micro-rdk/buf.gen.yaml --path google/rpc --path google/api
	buf generate buf.build/viamrobotics/api --template micro-rdk/buf.gen.yaml
	buf generate buf.build/protocolbuffers/wellknowntypes --template micro-rdk/buf.gen.yaml 

license-finder:
	license_finder

cargo-ver:
ifneq ($(ESPFLASHVERSION),true)
		$(error Update espfash to version >2.0. Update with cargo install cargo-espflash)
endif

build:
	cargo +esp build  -p examples --bin esp32-server --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort

build-native:
	cargo build -p examples  --bin native-server

native:
	cargo run -p examples  --bin native-server

build-qemu:
	cargo +esp build -p examples  --bin esp32-server  --features qemu binstart --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort && cargo espflash save-image --package examples --features qemu --merge --chip esp32 target/xtensa-esp32-espidf/debug/debug.bin -T examples/esp32/partitions.csv -s 4mb  --bin esp32-server --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort


sim-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205,hostfwd=tcp::12346-:12346 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw

# debug-local is identical to sim-local, except the `-S` at the end means "wait until a debugger is
# attached before starting."
debug-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -S

upload: cargo-ver
	cargo +esp espflash flash --package examples --monitor --partition-table examples/esp32/partitions.csv --baud 460800 -f 80mhz --bin esp32-server --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort

test:
	cargo test -p micro-rdk --lib --features native

clippy-native:
	cargo clippy -p micro-rdk --no-deps --features native --no-default-features -- -Dwarnings

clippy-esp32:
	cargo +esp clippy -p micro-rdk  --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort -- -Dwarnings

clippy-cli:
	cargo clippy -p micro-rdk-installer --no-default-features -- -Dwarnings

format:
	cargo fmt --all -- --check

doc:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --workspace --exclude micro-rdk-macros

doc-open:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --open

size:
	find . -name "esp-build.map" -exec ${IDF_PATH}/tools/idf_size.py {} \;

build-esp32-bin:
	cargo +esp espflash save-image --package examples --merge --chip esp32 target/xtensa-esp32-espidf/esp32-server.bin -T examples/esp32/partitions.csv -s 4mb  --bin esp32-server --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort --release

build-esp32-with-cred-bin:
	cargo +esp espflash save-image --package examples --merge --chip esp32 target/xtensa-esp32-espidf/esp32-server-with-cred.bin -T examples/esp32/partitions.csv -s 4mb  --bin esp32-server-with-cred --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort --release

flash-esp32-bin:
ifneq (,$(wildcard ./target/xtensa-esp32-espidf/esp32-server.bin))
	espflash write-bin 0x0 ./target/xtensa-esp32-espidf/esp32-server.bin -b 460800  && sleep 2 && espflash monitor
else
	$(error esp32-server.bin not found, run make build-esp32-bin first)
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
