.PHONY: build

build:
	cargo build --release

upload:
	cargo espflash flash --monitor --partition-table partitions.csv --baud 460800 -f 80mhz --release $(ESPFLASH_FLASH_ARGS)


build-esp32-bin:
	cargo espflash save-image --merge --chip esp32 target/esp32-server.bin --partition-table partitions.csv -s 4mb  --release

flash-esp32-bin:
ifneq (,$(wildcard target/esp32-server.bin))
	espflash write-bin 0x0 target/esp32-server.bin -b 460800  && sleep 2 && espflash monitor
else
	$(error esp32-server.bin not found, run build-esp32-bin first)
endif
