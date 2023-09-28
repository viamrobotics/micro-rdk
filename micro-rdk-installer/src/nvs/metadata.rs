use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use super::super::error::Error;

pub struct NVSMetadata {
    pub size: u64,
    pub start_address: u64
}

// These constraints are germane to Esp32's Partition Table. See Espressif's 
// docs: https://docs.espressif.com/projects/esp-idf/en/release-v4.4/esp32/api-guides/partition-tables.html?highlight=partition%20table
const PARTITION_TABLE_MAX_ENTRIES: usize = 95;
const PARTITION_TABLE_ENTRY_MAGIC_BYTES: [u8; 2] = [0xAA, 0x50];

pub fn read_nvs_metadata(binary_path: PathBuf) -> Result<NVSMetadata, Error> {
    let mut app_file = OpenOptions::new()
        .read(true)
        .open(binary_path)
        .map_err(Error::FileError)?;
    app_file
        .seek(SeekFrom::Start(0x8000))
        .map_err(Error::FileError)?;
    let mut entries_read = 0;
    loop {
        if entries_read == PARTITION_TABLE_MAX_ENTRIES {
            break
        }
        let mut magic_bytes: [u8; 2] = [0xFF, 0xFF];
        app_file.read(&mut magic_bytes[..]).map_err(Error::FileError)?;
        if magic_bytes != PARTITION_TABLE_ENTRY_MAGIC_BYTES {
            break;
        }
        let mut table_entry: [u8; 30] = [0xFF; 30];
        app_file.read(&mut table_entry[..]).map_err(Error::FileError)?;
        if let Some(nvs_metadata) = get_nvs_metadata_from_entry(&table_entry)? { return Ok(nvs_metadata) }
        entries_read += 1;
    }
    Err(Error::NVSMissingError)
}

fn get_nvs_metadata_from_entry(line: &[u8; 30]) -> Result<Option<NVSMetadata>, Error> {
    let type_is_data = line[0] == 0x01;
    let subtype_is_nvs = line[1] == 0x02;
    if type_is_data && subtype_is_nvs {
        if line[2..6] == [0xFF, 0xFF, 0xFF, 0xFF] {
            return Err(Error::NVSOffsetMissingError);
        }
        let offset = u32::from_le_bytes(match line[2..6].try_into() {
            Ok(val) => val,
            Err(_) => {
                unreachable!()
            }
        });
        let size = u32::from_le_bytes(match line[6..10].try_into(){
            Ok(val) => val,
            Err(_) => {
                unreachable!()
            }
        });
        return Ok(Some(NVSMetadata{
            size: size as u64,
            start_address: offset as u64
        }));
    }
    Ok(None)
}