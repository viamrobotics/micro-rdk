# The Viam Micro-RDK Installer (IN PROGRESS)

When completed, this will be a CLI that allows a user to flash a build of Micro-RDK, along with
their robot's credentials and their wifi information, directly to their esp32 without requiring
installation of ESP-IDF, Rust, or Python.

For now, this only has the option of generating a binary of data representing the esp32's Non-Volatile 
Storage (NVS) Partition and containing wifi and robot credentials. One could then potentially flash just
the NVS partition to their esp32 using tools provided by ESP-IDF. 

## Testing NVS

To test this functionality, download a json
file representing the robot's app config from the Setup tab of the robot part's page on app.viam.com and then
run the following command from within this subdirectory:
```
cargo run -- create-nvs-partition --app-config=<path to config json> --output=<destination path for resulting binary>
```

Alternatively you can build the binary (with `cargo build`) and run it in a similar fashion:
```
./micro-rdk-installer create-nvs-partition --app-config=<path to config json> --output=<destination path for resulting binary>
```