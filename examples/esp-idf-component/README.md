# Using Micro-RDK as a library Example

This example shows how to use the Micro-RDK in a new or existing ESP project.

## How to configure the example

Open the project configuration menu (`idf.py menuconfig`).

In the `Micro-RDK lib example configuration` menu:

* Set the Wi-Fi configuration.
    * Set `WiFi SSID`.
    * Set `WiFi Password`.

## Build and Flash

Build the project and flash it to the board, then run the monitor tool to view the serial output:

Run `idf.py -p PORT flash monitor` to build, flash and monitor the project.

(To exit the serial monitor, type ``Ctrl-]``.)

See the Getting Started Guide for all the steps to configure and use the ESP-IDF to build projects.

* [ESP-IDF Getting Started Guide on ESP32](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/index.html)

See the Viam documentation for more information about the Micro-RDK:

* [Micro-RDK](https://docs.viam.com/operate/get-started/other-hardware/micro-module/)
