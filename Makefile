.PHONY: build

SHELL := /bin/bash


build:
	cargo build
debug-local:
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	cargo build --features qemu && cargo espflash save-image --features qemu --merge ESP32 target/xtensa-esp32-espidf/debug/debug.bin -T partitions.csv -s 4MB
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=tcp::7888-:80 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw -S
sim-local:
ifndef QEMU_ESP32_XTENSA
	$(error QEMU_ESP32_XTENSA is not set)
endif
	pkill qemu || true
	cargo build --features qemu && cargo espflash save-image --features qemu --merge ESP32 target/xtensa-esp32-espidf/debug/debug.bin -T partitions.csv -s 4MB
	$(QEMU_ESP32_XTENSA)/qemu-system-xtensa -nographic -machine esp32 -gdb tcp::3334 -nic user,model=open_eth,hostfwd=tcp::7888-:80 -drive file=target/xtensa-esp32-espidf/debug/debug.bin,if=mtd,format=raw

buf-clean:
	find src/gen -type f \( -iname "*.rs" \) -delete

buf:	buf-clean
	buf generate buf.build/viamrobotics/goutils --template buf.gen.yaml
	buf generate buf.build/googleapis/googleapis --template buf.gen.yaml --path google/rpc --path google/api
	buf generate buf.build/viamrobotics/api --template buf.gen.yaml

upload:
	cargo build && cargo espflash save-image -f 80M -s 4MB -T partitions.csv ESP32 target/xtensa-esp32-espidf/debug/mini-rdk.bin
	cargo espflash partition-table --to-binary partitions.csv -o target/xtensa-esp32-espidf/debug/part.bin
	esptool.py  --baud 460800 write_flash --flash_mode qio --flash_freq 80m 0x8000 target/xtensa-esp32-espidf/debug/part.bin 0x10000 target/xtensa-esp32-espidf/debug/mini-rdk.bin && sleep 2 && cargo espflash serial-monitor
