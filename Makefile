SHELL := /bin/bash
ESPFLASHVERSION = $(shell expr `cargo espflash -V | grep ^cargo-espflash | sed 's/^.* //g' | cut -f1 -d. ` \< 2)

DATE := $(shell date +%F)
IMAGE_BASE = ghcr.io/viamrobotics/micro-rdk-dev-env

buf-clean:
	find src/gen -type f \( -iname "*.rs" \) -delete

buf: buf-clean
	buf generate buf.build/viamrobotics/goutils --template buf.gen.yaml
	buf generate buf.build/googleapis/googleapis --template buf.gen.yaml --path google/rpc --path google/api
	buf generate buf.build/viamrobotics/api --template buf.gen.yaml

license-finder:
	license_finder

cargo-ver:
ifeq "$(ESPFLASHVERSION)" "1"
		$(error Update espfash to version >2.0. Update with cargo install cargo-espflash@2.0.0-rc.1)
endif

build:
	cd examples && cargo build  --bin esp32-server --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort

build-native:
	cd examples && cargo build  --bin native-server

native:
	cd examples && cargo run  --bin native-server

build-qemu:
	cd examples && cargo build  --bin esp32-server  --features qemu --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort && cargo espflash save-image --features qemu --merge --chip esp32 target/xtensa-esp32-espidf/debug/debug.bin -T esp32/partitions.csv -s 4M  --bin esp32-server --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort


sim-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	cd examples && $(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205,hostfwd=tcp::12346-:12346 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw

# debug-local is identical to sim-local, except the `-S` at the end means "wait until a debugger is
# attached before starting."
debug-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	cd examples && $(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=udp::-:61205 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -S

upload: cargo-ver
	cd examples && cargo espflash flash --monitor --partition-table esp32/partitions.csv --baud 460800 -f 80M --use-stub --bin esp32-server --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort

test:
	cargo test --lib --features native

clippy-native:
	cargo clippy --no-deps --features native --no-default-features -- -Dwarnings

clippy-esp32:
	cargo +esp clippy  --features esp32 --no-default-features --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort -- -Dwarnings

clippy-cli:
	cd micro-rdk-installer && cargo clippy --no-default-features -- -Dwarnings

format:
	cargo fmt --all -- --check
	cd examples && cargo fmt --all -- --check

doc:
	cargo doc --no-default-features --features esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort

size:
	find . -name "esp-build.map" -exec ${IDF_PATH}/tools/idf_size.py {} \;

build-esp32-bin:
	cd examples && cargo espflash save-image --merge --chip esp32 target/esp32-server.bin -T esp32/partitions.csv -s 4M  --bin esp32-server --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort --release

build-esp32-with-cred-bin:
	cd examples && cargo espflash save-image --merge --chip esp32 target/esp32-server-with-cred.bin -T esp32/partitions.csv -s 4M  --bin esp32-server-with-cred --target=xtensa-esp32-espidf  -Zbuild-std=std,panic_abort --release

flash-esp32-bin:
ifneq (,$(wildcard ./examples/target/esp32-server.bin))
	espflash write-bin 0x0 ./examples/target/esp32-server.bin -b 460800  && sleep 2 && espflash monitor
else
	$(error esp32-server.bin not found, run build-esp32-bin first)
endif


build-fake:
	touch esp32-server-with-cred.bin
	dd if=/dev/urandom of=esp32-server-with-cred.bin bs=4M count=1

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
