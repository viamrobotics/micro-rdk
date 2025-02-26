.PHONY: build

build:
	cargo build --release

upload:
	cargo espflash flash --erase-parts nvs --monitor --partition-table partitions.csv --baud 460800 -f 80mhz --release $(ESPFLASH_FLASH_ARGS)


build-esp32-bin:
	cargo espflash save-image --merge --chip esp32 target/xtensa-esp32-espidf/release/{{project-name}}.bin --partition-table partitions.csv -s 8mb  --release

build-esp32-ota:
	cargo +esp espflash save-image \
		--skip-update-check \
		--chip=esp32 \
		--partition-table=partitions.csv \
		--target=xtensa-esp32-espidf \
		-Zbuild-std=std,panic_abort --release \
		target/xtensa-esp32-espidf/project-ota.bin

flash-esp32-bin:
ifneq (,$(wildcard target/xtensa-esp32-espidf/release/{{project-name}}))
	espflash flash --erase-parts nvs --partition-table partitions.csv  target/xtensa-esp32-espidf/release/{{project-name}} --baud 460800 -s 8mb && sleep 2 && espflash monitor
else
	$(error target/xtensa-esp32-espidf/release/{{project-name}} not found, run build-esp32-bin first)
endif
