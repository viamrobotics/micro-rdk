# `free_heap_sensor`

## Configure

The `free_heap_sensor` is a wrapper around `esp_get_free_heap_size` and does not require additional attributes.

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
| bytes | int | Number of bytes available on the heap |



# `wifi_rssi_sensor`

The `wifi_rssi_sensor` is a wrapper around esp-idf `esp_wifi_sta_get_ap_info` api

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
