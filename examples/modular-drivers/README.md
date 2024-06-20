# Viam Micro-RDK Modular Driver Examples

## (In)stability Notice

> **Warning** The Viam Micro-RDK is currently in beta.

## Overview

This project in this repository was generated from the [Micro-RDK
Module
Template](https://github.com/viamrobotics/micro-rdk/tree/main/templates/module),
and demonstrates how to produce modular resources for the Micro-RDK.

## Usage

### Setup

If you don't yet have a Micro-RDK robot project, please create one by
following the [Micro-RDK Development
Setup](https://docs.viam.com/installation/prepare/microcontrollers/development-setup/)
instructions.

### Installation

To add this module to a robot project, just add this repository to the
`[dependencies]` section of the `Cargo.toml` file in the robot
project:


``` diff
diff --git a/Cargo.toml b/Cargo.toml
index 79fbc5c..949b5ab 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -32,6 +32,8 @@ async-channel = "1.8.0"
 smol = "1.2"
 futures-lite = "1.12.0"
 micro-rdk = {version = "0.0.3", git = "https://github.com/viamrobotics/micro-rdk.git", features = ["esp32"]}
+micro-rdk-modular-driver-example = { git = "https://github.com/viamrobotics/micro-rdk/tree/main/examples/modular-drivers" }
```

Rebuild the project per the above Micro-RDK Development Setup
instructions and reflash the board. We will be using the Wifi
RSSI Sensor and free heap sensor for this example, which used
to be in a seperate folder called 'micro-rdk-esp32-sensor-examples'.

### Instantiating the Sensors

You can now instantiate the sensors by adding them as new `components`
in your robot configuration on app.viam.com.

To instantiate the Wifi RSSI Sensor, add the following to the
`components` section of your configuration (edit using the `Raw JSON`
mode):

``` json
    {
      "name": "my-wifi-sensor",
      "type": "sensor",
      "model": "wifi-rssi",
      "attributes": {},
      "depends_on": []
    }
```

To instantiate the free heap sensor, add the following:

``` json
    {
      "attributes": {},
      "depends_on": [],
      "type": "sensor",
      "model": "free-heap",
      "name": "my-free-heap-sensor"
    }
```

Reboot the ESP32 board (by, say, pressing the physical "boot" button,
or hitting Ctrl-R if the monitor is active) so that it can pull the
new configuration from app.viam.com, and these sensors should now be
available to query in your language of choice with the Viam SDK (you
can find this code on the `Code Sample` page for your robot):

``` python
    # wifi-sensor
    wifi_sensor = Sensor.from_robot(robot, "my-wifi-sensor")
    wifi_sensor_return_value = await wifi_sensor.get_readings()
    print(f"wifi-sensor get_readings return value: {wifi_sensor_return_value}")

    # free-heap-sensor
    free_heap_sensor = Sensor.from_robot(robot, "my-free-heap-sensor")
    free_heap_sensor_return_value = await free_heap_sensor.get_readings()
    print(f"free-heap-sensor get_readings return value: {free_heap_sensor_return_value}")
```

## Implementation Walkthrough

This project was created by using the [Micro-RDK Module
Template](https://github.com/viamrobotics/micro-rdk/tree/main/templates/module)
and `cargo generate`:

``` shell
$ cargo install cargo-generate
$ cargo generate --git https://github.com/viamrobotics/micro-rdk templates/module
```

When prompted by the template, the project was named
`micro-rdk-esp32-sensor-examples` and `esp32` selected for the target
platform. The generated project has the form of a library crate, where
`src/lib.rs` defines an initially empty implementation of the
well-known Micro-RDK module entry point `register_models`:

``` rust
use micro_rdk::common::registry::{ComponentRegistry, RegistryError};

pub fn register_models(_registry: &mut ComponentRegistry) -> anyhow::Result<(), RegistryError> {
    Ok(())
}
```

The generated project also includes a `package.metadata` section in
its `Cargo.toml` which identifies the library crate as being a
Micro-RDK module:

https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/Cargo.toml#L18-19

A subsequent commit adds definitions of the
[FreeHeapSensor](https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/free_heap_sensor.rs)
and
[WifiRSSISensor](https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/wifi_rssi_sensor.rs)

That commit also introduces a crate-local `register_models` function
for each sensor:

- `FreeHeapSensor`: https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/free_heap_sensor.rs#L23-L27
- `WifiRSSISensor`: https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/wifi_rssi_sensor.rs#L23-L27

Finally, the top level `register_models` entry point is updated to delegate to the `register_model` function for both sensors:

https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/lib.rs#L10-L18

The Micro-RDK module is now ready to be used in a Micro-RDK project,
just by adding it as an ordinary dependency in the `dependencies`
section of the project's `Cargo.toml` file, as noted in the
`Installation` section above.

## Related Reading

Please see the [moisture sensor](https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/moisture_sensor.rs) and [water pump](https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/src/water_pump.rs) for
more examples of modular Micro-RDK drivers.
