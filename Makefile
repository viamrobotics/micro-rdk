SHELL := /bin/bash
ESPFLASHVERSION = $(shell expr `cargo espflash -V | grep ^cargo-espflash | sed 's/^.* //g' | cut -f1 -d. ` \< 2)

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
	cargo build  --example esp32  --features qemu,esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort

build-qemu:
	cargo build  --example esp32  --features qemu,esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort && cargo espflash save-image --features qemu,esp32 --merge --chip esp32 target/xtensa-esp32-espidf/debug/debug.bin -T examples/esp32/partitions.csv -s 4M  --example esp32


sim-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=tcp::7888-:80 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw

# debug-local is identical to sim-local, except the `-S` at the end means "wait until a debugger is
# attached before starting."
debug-local: cargo-ver build-qemu
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=tcp::7888-:80 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -S

upload: cargo-ver
	cargo espflash flash --monitor --partition-table examples/esp32/partitions.csv --baud 460800 -f 80M --use-stub --example esp32 --target=xtensa-esp32-espidf -Zbuild-std=std,panic_abort --features esp32
