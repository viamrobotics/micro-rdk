# Over-The-Air (OTA) Updates

## Workflow

In [app.viam.com](app.viam.com), add the following to the `services` array; you can alternatively add a `generic` service then edit it to match the following

### OTA Service Config

```json
{
  "name": "OTA",
  "namespace": "rdk",
  "type": "generic",
  "model": "ota_service",
  "attributes": {
    "url": <firmware-download-url>,
    "version": <some-tag>
  }
}
```


In the `url` field, enter the url where a firmware will be downloaded from
   - if using a local endpoint on the same network, remember to use the proper private ip address
   - for cloud-hosted resources, embedded auth in the url is easiest, we don't support tokens yet


The `version` field is equivalent to a `tag` and can be any arbitrary string of up to 128 characters.
After successfully applying the new firmware, this `version` will be stored in NVS.
This value is compared to that of the latest machine config from app.viam.com and will trigger the update process.


## Requirements

- an esp32 WROVER-E with 8MB or more of flash memory
- a partition table (ex `partitions.csv`) with `otadata`, `ota_0`, and `ota_1` partitions


## Primer

Consider firmware built with the following partition table:

```
# ESP-IDF Partition Table

# Name,   Type, SubType, Offset,  Size, Flags
# Note: if you have increased the bootloader size, make sure to update the offsets to avoid overlap
nvs,	       data,	nvs,	  0x9000,	0x6000,
otadata,       data,	ota,	  0xF000,	0x2000,
phy_init,      data,	phy, 	  0x11000, 	0x1000,
ota_0,	       app,	    ota_0,	  ,		    0x377000,
ota_1,	       app,	    ota_1,	  ,		    0x377000,
```

The `otadata` partition contains information on which OTA partition to boot from and the states of the `ota_*` partitions.

The terms 'firmware' or 'binary' can be a bit generic.
In this section, we will refer to two types of binaries that can be built.
1. a Merged Image
2. an [Application (App) Image](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/app_image_format.html)

> Note: in the absence of a `factory` partition `ota_0` fills the same initial role.

## Full Build

If a device is built with the above partition table, the `make build-esp32-bin` command creates a Merged Image that includes
- the bootloader
- the partition table mapping (`partitions.csv`)
- populated partitions (with partition headers) according to the mapping

The command `make flash-esp32-bin` writes this entire Merged Image to the device's flash memory.

This is the build workflow which must be used to:
- flash a new device for the first time
- update a device's partition table
  - for example, make a device capable of OTA.

**This is not the build that should be hosted at the `url` in the service config.**
You can confirm this by using `ls -l` in your build directory to compare the size of the binary to your partition table; the Merged Image will about the size of the full partition table, `8MB` in this example.

## OTA Build

The `make build-esp32-ota` command produces an App Image (described above), which internally consists of:
- the type-specific partition header, `esp_app_desc_t**
- the application image that contains the program instructions

**This app image is what you must host, see [Firmware Hosting Options](#firmware-hosting-options).**

This build must be within the size limits of the smallest `ota_*` partition in a device's *current* partition table.
This document assumes the user is using our included partition tables; should the final image be larger than the capacity of the ota partitions, the build will fail indicating so.

To update a device's partition table, use the method in the [Full Build](#full-build) workflow.


## Firmware Hosting Options
### Local
  - use `make serve-dev-ota` to create a local endpoint for serving the ota app image
  - the command will build the ota firmware first before serving the url

### Cloud

The OTA Service in the Micro-RDK currently supports **only HTTP/2**, this means that the hosting platform must support HTTP/2 connections.

While not all blob storage platform support HTTP/2, many offer Content Delivery Network (CDN) solutions that do.

We don't currently support authentication tokens in the [OTA Service Config](#ota-service-config), so if permissions are required to access the endpoint they must be embedded in the URL as query params.

## Related Links

> Links may point to latest branches of documentation to reduce chances of dead links; reference the appropriate version if available.

- [Over The Air Updates](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/ota.html) - Espressif
- [Partition Tables](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html) - Espressif
- [App Image Format](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/app_image_format.html) - Espressif
