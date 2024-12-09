# Over-The-Air (OTA) Updates


> OTA is experimental and in active development. Breaking changes should be expected often. Check this document frequently for updates


## Workflow

In app.viam, add the following to the `services` array; you can alternatively add a `generic` service then edit it to match the following

```json
    {
      "name": "OTA",
      "namespace": "rdk",
      "type": "generic",
      "model": "ota_service",
      "attributes": {
        "url": "",
        "version": ""
		}
	}
```


In the `url` field, enter the url where a firmware will be downloaded from
   - if using a local endpoint on the same network, remember to use the proper private ip address
   - for cloud-hosted resources, embedded auth in the url is easiest, we don't support jwt yet


The `version` field is equivalent to a `tag` and can be any arbitrary string of up to 128 characters. After successfully applying the new firmware, this `version` will be stored in NVS. This values is compared to that of the latest machine config from app.viam and will trigger the update process.


## Requirements

- An esp32 wrover-e with 8MB of flash memory.

## Build Process

## Primer

```
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,  0x4000,
otadata,  data, ota,     0xd000,  0x2000,
phy_init, data, phy,     0xf000,  0x1000,
factory,  app,  factory, 0x10000,  1M,
ota_0,    app,  ota_0,   0x110000, 1M,
ota_1,    app,  ota_1,   0x210000, 1M,

```

The terms 'firmware' or 'binary' can be a bit generic.
In this section, we will refer to two types of binaries that can be built.
1. a Merged Binary
2. an App Image

## Full Build

If a device is built with the above partition table, the `make build-esp32-bin` command creates a Merged Binary that includes
- the bootloader
- the partition table mapping (`partitions.csv`)
- populated partitions (with partition headers) according to the mapping

The command `make flash-esp32-bin` writes this entire merged binary to the device's flash memory.

This is the build workflow which must be used if you want to update a device's partition table; for example, to make a device capable of OTA.

**This is not the build that should be hosted at the `url` in the service config.**
You can confirm this by using `ls -l` in your build directory to compare the size of the binary to your partition table.

### OTA Build
The `ota` build consists of *only*:
- the type-specific partition header, `esp_app_desc_t`
- the application image that contains the program instructions

This build must be within the size limits for the `ota0` and `ota1` partitions specified by a device's *current* partition table.

To update a device's partition table, use the method in the Full Build workflow.


## Firmware Upload and Hosting Options
### Local
- use `make serve-ota` to point to create a local endpoint serving the ota build. Ensure the link includes the host's private address in the url.
### Cloud
- if using a cloud hosting solution, generate a `url` for downloading your firmware that includes auth embedded in the url if possible.

## Internals
