.PHONY: build

# Older ESP32 devices might only have 4mb of flash and slower
FLASHSIZE:=8mb
BAUDRATE:=460800

clean:
	cargo clean

build:
	cargo build --release

upload:
	cargo espflash flash \
		--erase-parts nvs \
		--monitor \
		--partition-table partitions.csv \
		--baud ${BAUDRATE} \
		--flash-freq 80mhz \
		--release $(ESPFLASH_FLASH_ARGS)

monitor:
	espflash monitor --baud ${BAUDRATE}

info:
	cargo espflash board-info --baud ${BAUDRATE}

erase:
	cargo espflash erase-flash --baud ${BAUDRATE}

list-partitions:
	cargo espflash partition-table partitions.csv

build-esp32-bin:
	cargo espflash save-image \
		--merge \
		--chip esp32 \
		--partition-table partitions.csv \
		--target xtensa-esp32-espidf \
		--flash-size ${FLASHSIZE} \
		--release \
		target/xtensa-esp32-espidf/release/{{project-name}}.bin

build-esp32-ota:
	cargo +esp espflash save-image \
		--skip-update-check \
		--chip=esp32 \
		--partition-table=partitions.csv \
		--target=xtensa-esp32-espidf \
		-Zbuild-std=std,panic_abort \
		--release \
		target/xtensa-esp32-espidf/project-ota.bin

flash-esp32-bin:
ifneq (,$(wildcard target/xtensa-esp32-espidf/release/{{project-name}}))
	espflash flash \
		--erase-parts nvs \
		--partition-table partitions.csv \
		--flash-size ${FLASHSIZE} \
		--baud ${BAUDRATE} \
		target/xtensa-esp32-espidf/release/{{project-name}} && sleep 2 && espflash monitor --baud ${BAUDRATE}
else
	$(error target/xtensa-esp32-espidf/release/{{project-name}} not found, run build-esp32-bin first)
endif
