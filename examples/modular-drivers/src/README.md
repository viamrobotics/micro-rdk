# `free_heap_sensor`

## Configure

The `free_heap_sensor` is a wrapper around [`esp_get_free_heap_size`](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/misc_system_api.html#_CPPv422esp_get_free_heap_sizev) and does not require additional attributes.

``` json
    {
      "attributes": {},
      "depends_on": [],
      "type": "sensor",
      "model": "free-heap",
      "name": "my-free-heap-sensor"
    }
```

## Returned Values
| Key | Type | Description |
|-----|------|-------------|
| bytes | int | Available heap size, in bytes. |



# `wifi_rssi_sensor`

The `wifi_rssi_sensor` is a wrapper around [`esp_wifi_sta_get_ap_info`](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/network/esp_wifi.html#_CPPv424esp_wifi_sta_get_ap_infoP16wifi_ap_record_t) and does not require additional attributes.

## Configure
```json
    {
      "name": "my-wifi-sensor",
      "namespace": "rdk",
      "type": "sensor",
      "model": "wifi-rssi",
      "attributes": {}
    }
```

## Returned Values
| Key        | Type  | Description                                |
|------------|-------|--------------------------------------------|
| rssi | int | Signal strength of AP. Note that in some rare cases where signal strength is very strong, RSSI values can be slightly positive |



# `moisture_sensor`

## Configure

The `moisture_sensor` module is a wrapper around the `board`'s analogue reader.
It requires both a `board` configured with an `analog` attribute and the `moisture_sensor` itself.

```json
    {
      "name": "my-board",
      "api": "rdk:component:board",
      "model": "rdk:builtin:esp32",
      "attributes": {
        "pins": [],
        "analogs": [
          {
            "pin": 34,
            "name": "moisture"
          }
        ]
      }
    },
	{
	  "name": "moisture",
	  "api": "rdk:component:sensor",
      "model": "moisture_sensor",
      "attributes": {},
      "depends_on": [
	    "board-1"
      ]
	}
```

## Returned Values
| Key        | Type  | Description                                |
|------------|-------|--------------------------------------------|
| millivolts | float | Dryness as a raw value between 0 and 3,300 |

# `water_pump`

## Configure

The `water_pump` is a `motor` that is driven by a single `pin`. It optionally takes an `led` attribute which is another GPIO pin that controls an LED.

```json
    {
      "name": "my-board",
      "api": "rdk:component:board",
      "model": "rdk:builtin:esp32",
      "attributes": {
        "pins": [
		  15, 
		  16
	    ],
        "analogs": [
          {}
        ]
      }
    },
	{
	  "name": "moisture",
	  "api": "rdk:component:motor",
      "model": "water_pump",
      "attributes": {},
      "depends_on": [
	    "my-board"
      ]
	}
```

