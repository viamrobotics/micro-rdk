use std::collections::VecDeque;

use crc32fast::Hasher;

use super::data::ViamFlashStorageData;

const MAX_BLOB_SIZE: usize = 4000;
const BLOB_DATA_FORMAT: u8 = 0x42;
const STRING_VALUE_FORMAT: u8 = 0x21;
const BLOB_IDX_FORMAT: u8 = 0x48;
const PAGE_VERSION: u8 = 0xFE; // Version 2

const DEFAULT_BLOB_CHUNK_IDX: u8 = 0xFF;

/*
This module represents code for generating an Non-Volatile Storage Partition binary tailored
to storing wifi credentials and security credentials for a robot configured through Viam.
The code was heavily inspired by the ESP-IDF's partition generator utility (nvs_partition_gen.py)

An NVS Partition is sectioned into Pages and each Page has 32-byte entries. Every key-value pair
stored in NVS consists of a 32-byte header entry (which contains the key and other metadata) followed
by data split into however many entries are required based on length. Since the above credentials data
consists only of string or binary values, only those two entry formats have been implemented

More information on the structure of NVS and its API can be found in Espressif's online documentation
(https://docs.espressif.com/projects/esp-idf/en/release-v4.4/esp32/api-reference/storage/nvs_flash.html)
*/

// computes the checksum of the contents of the header and stores it at index 4
// as a 32-bit integer (see the link above for more information)
fn set_header_crc(header: &mut Vec<u8>) {
    let mut crc_data = std::iter::repeat(0).take(28).collect::<Vec<u8>>();
    crc_data[0..4].clone_from_slice(&header[0..4]);
    crc_data[4..28].clone_from_slice(&header[8..32]);
    let mut hasher = Hasher::new_with_initial(0xFFFFFFFF);
    hasher.update(&crc_data);
    let checksum = hasher.finalize() & 0xFFFFFFFF;
    let _ = header.splice(4..8, checksum.to_le_bytes());
}

// pad the final entry for piece of data to meet the 32 byte length requirement
fn pad_data(data: &mut Vec<u8>, data_entry_count: usize) {
    let padding_len = (data_entry_count * 32) - data.len();
    let mut padding = std::iter::repeat(0xFF)
        .take(padding_len)
        .collect::<Vec<u8>>();
    data.append(&mut padding);
}

pub struct NVSPartition {
    pub entries: Vec<NVSEntry>,
    pub size: usize,
}

impl NVSPartition {
    pub fn from_storage_data(data: ViamFlashStorageData, size: usize) -> anyhow::Result<Self> {
        Ok(Self {
            entries: data.to_entries(0)?,
            size,
        })
    }
}

impl TryFrom<ViamFlashStorageData> for NVSPartition {
    type Error = anyhow::Error;
    fn try_from(value: ViamFlashStorageData) -> Result<Self, Self::Error> {
        Ok(Self {
            entries: value.to_entries(0)?,
            size: 32768,
        })
    }
}

pub struct NVSEntry {
    pub header: Vec<u8>,
    pub data: Vec<u8>,
    pub data_entry_count: u8,
}

impl NVSEntry {
    pub fn get_blob_index_entry(&self, num_chunks: u8, data_len: u32) -> Vec<u8> {
        let mut header = std::iter::repeat(0xFF).take(32).collect::<Vec<u8>>();
        header.copy_from_slice(&self.header);
        header[24..32].copy_from_slice(&[0xFF; 8]);
        header[1] = BLOB_IDX_FORMAT;
        header[2] = 1;
        header[3] = DEFAULT_BLOB_CHUNK_IDX;
        let data_len_bytes = (data_len as u32).to_le_bytes();
        header[24..28].copy_from_slice(&data_len_bytes);
        header[28] = num_chunks as u8;
        header[29] = 0;
        set_header_crc(&mut header);
        header
    }
}

pub enum NVSValue {
    String(String),
    Bytes(Vec<u8>),
}

pub struct NVSKeyValuePair {
    pub key: String,
    pub value: NVSValue,
    pub namespace_idx: u8,
}

impl TryFrom<&NVSKeyValuePair> for NVSEntry {
    type Error = anyhow::Error;
    fn try_from(pair: &NVSKeyValuePair) -> Result<Self, Self::Error> {
        let format: u8;
        let data = match &pair.value {
            NVSValue::String(value_str) => {
                format = STRING_VALUE_FORMAT;
                let mut res = value_str.to_string().into_bytes();
                res.push(0x00);
                res
            }
            NVSValue::Bytes(val) => {
                format = BLOB_DATA_FORMAT;
                val.to_vec()
            }
        };
        let value_len = data.len();
        if value_len > MAX_BLOB_SIZE {
            anyhow::bail!("value too big to pack")
        }

        let mut header = std::iter::repeat(0xFF).take(32).collect::<Vec<u8>>();
        // write namespace
        header[0] = pair.namespace_idx + 1;
        // write entry count
        let rounded_size: usize = (value_len + 31) & !31;
        let data_entry_count = u8::try_from(rounded_size / 32)?;
        header[2] = data_entry_count + 1;
        header[3] = DEFAULT_BLOB_CHUNK_IDX;
        // write key
        let key_bytes = pair.key.to_string().into_bytes();
        let key_arr = std::iter::repeat(0x00).take(16).collect::<Vec<u8>>();
        let _ = header.splice(8..24, key_arr);
        let key_end = key_bytes.len() + 8;
        let _ = header.splice(8..key_end, key_bytes);
        // write crc
        header[1] = format;
        let value_len_bytes = u16::try_from(value_len)?.to_le_bytes();
        let _ = header.splice(24..26, value_len_bytes);
        Ok(NVSEntry {
            header,
            data,
            data_entry_count,
        })
    }
}

struct NVSPage {
    data: [u8; 4096],
    current_position: usize,
    bitmap_array: [u8; 32],
    entry_num: usize,
}

impl NVSPage {
    pub fn new(section_number: u32) -> Self {
        let mut data = [0xFF; 4096];
        let mut header = [0xFF; 32];
        let active_state = (0xFFFFFFFE as u32).to_le_bytes();
        header[0..4].clone_from_slice(&active_state);
        let section_number_bytes = section_number.to_le_bytes();
        header[4..8].clone_from_slice(&section_number_bytes);
        header[8] = PAGE_VERSION;
        let mut hasher = Hasher::new_with_initial(0xFFFFFFFF);
        hasher.update(&header[4..28]);
        let checksum = hasher.finalize();
        let crc = (checksum & 0xFFFFFFFF).to_le_bytes();
        header[28..32].clone_from_slice(&crc);
        data[0..32].clone_from_slice(&header);
        let bitmap_array = [255; 32];
        Self {
            data,
            current_position: 64,
            bitmap_array,
            entry_num: 0,
        }
    }

    pub fn get_remaining_space(&self) -> usize {
        self.data.len() - self.current_position
    }

    pub fn write_namespace(&mut self) -> anyhow::Result<()> {
        let mut header = std::iter::repeat(0xFF).take(32).collect::<Vec<u8>>();
        header[0] = 0;
        header[2] = 0x01;
        header[3] = 0xFF;
        let key_bytes = "VIAM_NS".to_string().into_bytes();
        let key_arr = std::iter::repeat(0x00).take(16).collect::<Vec<u8>>();
        let _ = header.splice(8..24, key_arr);
        let key_end = key_bytes.len() + 8;
        let _ = header.splice(8..key_end, key_bytes);
        header[1] = 0x01;
        header[24] = 0x01;
        set_header_crc(&mut header);
        self.write_misc_data(&header, 1)
    }

    pub fn write_chunk_to_data(
        &mut self,
        header: &Vec<u8>,
        entry_data: &mut Vec<u8>,
        chunk_num: u8,
        data_entry_count: u8,
        pad: bool,
    ) -> anyhow::Result<()> {
        let write_len = header.len() + entry_data.len();
        let remaining_space = self.get_remaining_space();
        if write_len > (remaining_space) {
            log::error!("tried to write {:?} bytes", write_len);
            log::error!("actual space: {:?}", remaining_space);
            anyhow::bail!("not enough space left in current section, make new one")
        }
        let entry_data_pos = self.current_position + header.len();
        let mut edited_header = std::iter::repeat(0xFF).take(32).collect::<Vec<u8>>();
        edited_header.copy_from_slice(&header);
        edited_header[3] = chunk_num as u8;
        let data_len_bytes = (entry_data.len() as u16).to_le_bytes();
        edited_header[24..26].copy_from_slice(&data_len_bytes);
        let mut hasher = Hasher::new_with_initial(0xFFFFFFFF);
        hasher.update(&entry_data);
        let crc_checksum = (hasher.finalize() & 0xFFFFFFFF).to_le_bytes();
        let _ = edited_header.splice(28..32, crc_checksum);
        set_header_crc(&mut edited_header);
        self.data[self.current_position..entry_data_pos].clone_from_slice(&edited_header);
        self.write_bitmaparray(1);
        if pad {
            pad_data(entry_data, data_entry_count as usize);
        }
        let entry_data_end = entry_data_pos + entry_data.len();
        self.data[entry_data_pos..entry_data_end].clone_from_slice(entry_data);
        self.write_bitmaparray(data_entry_count);
        self.current_position = entry_data_end;
        Ok(())
    }

    fn write_bitmaparray(&mut self, data_entry_count: u8) {
        for _ in 0..data_entry_count {
            let bitnum = self.entry_num * 2;
            let byte_idx = bitnum / 8;
            let bit_offset = bitnum & 7;
            let mask: u8 = !(1 << bit_offset);
            self.bitmap_array[byte_idx] &= mask;
            self.data[32..64].clone_from_slice(&self.bitmap_array);
            self.entry_num += 1;
        }
    }

    pub fn write_misc_data(&mut self, data: &Vec<u8>, data_entry_count: u8) -> anyhow::Result<()> {
        let write_len = data.len();
        if write_len > 4096 - self.current_position {
            anyhow::bail!("not enough space left in current section, make new one")
        }
        let end_idx = self.current_position + write_len;
        self.data[self.current_position..end_idx].clone_from_slice(data);
        self.current_position += write_len;
        self.write_bitmaparray(data_entry_count);
        Ok(())
    }

    pub fn close(&mut self) {
        let closed_state = (0xFFFFFFFC as u32).to_le_bytes();
        self.data[0..4].clone_from_slice(&closed_state);
    }
}

pub struct NVSPartitionData {
    sections: VecDeque<NVSPage>,
    current_section: usize,
    size: usize,
}

impl NVSPartitionData {
    fn new(size: usize) -> Self {
        Self {
            sections: vec![NVSPage::new(0)].into(),
            current_section: 0,
            size,
        }
    }

    pub fn start_new_section(&mut self) -> anyhow::Result<()> {
        let max_sections = self.size / 4096 - 1;
        if self.current_section == max_sections - 1 {
            anyhow::bail!("data overflow, increase size for NVS partition and try again")
        }
        self.sections[self.current_section].close();
        self.current_section += 1;
        self.sections
            .push_back(NVSPage::new(self.current_section as u32));
        Ok(())
    }

    pub fn write_string_entry(&mut self, entry: &mut NVSEntry) -> anyhow::Result<()> {
        let mut current_section = &mut self.sections[self.current_section];
        let curr_size = current_section.get_remaining_space();
        let header = &mut entry.header;
        let data_len = entry.data.len();
        let projected_total_entries =
            current_section.entry_num + entry.data_entry_count as usize + 1;
        if ((header.len() + data_len) > curr_size) || (projected_total_entries > 126) {
            self.start_new_section()?;
            current_section = &mut self.sections[self.current_section];
        }
        let mut hasher = Hasher::new_with_initial(0xFFFFFFFF);
        hasher.update(&entry.data);
        let checksum = hasher.finalize();
        let crc = (checksum & 0xFFFFFFFF).to_le_bytes();
        header[28..32].clone_from_slice(&crc);
        set_header_crc(header);
        current_section.write_misc_data(header, 1)?;

        let data = &mut entry.data;
        let padding_len = (entry.data_entry_count as usize * 32) - data.len();
        let mut padding = std::iter::repeat(0xFF)
            .take(padding_len)
            .collect::<Vec<u8>>();
        data.append(&mut padding);
        current_section.write_misc_data(data, entry.data_entry_count)?;
        Ok(())
    }

    pub fn write_binary_entry(&mut self, entry: &mut NVSEntry) -> anyhow::Result<()> {
        let mut current_section = &mut self.sections[self.current_section];
        let mut curr_size = current_section.get_remaining_space();
        let header = &mut std::iter::repeat(0xFF).take(32).collect::<Vec<u8>>();
        header.copy_from_slice(&entry.header);
        if header.len() > curr_size {
            self.start_new_section()?;
            current_section = &mut self.sections[self.current_section];
            curr_size = current_section.get_remaining_space();
        }
        curr_size -= header.len();
        let data = &mut entry.data;
        let mut data_len_u32 = 0;
        let mut chunk_num: u8 = 0;
        while data.len() > 0 {
            let split_idx = match data.len() < curr_size {
                true => data.len(),
                false => curr_size,
            };
            let mut to_write: Vec<u8> = data.drain(0..split_idx).collect();
            let chunk_size = (to_write.len() + 31) & !31;
            let data_entry_count = (chunk_size / 32) as u8;
            header[2] = data_entry_count + 1;
            header[3] = chunk_num;
            data_len_u32 += to_write.len() as u32;
            current_section.write_chunk_to_data(
                header,
                &mut to_write,
                chunk_num,
                data_entry_count,
                data.len() == 0,
            )?;
            if data.len() != 0 {
                self.start_new_section()?;
                current_section = &mut self.sections[self.current_section];
                curr_size = current_section.get_remaining_space() - header.len();
            }
            chunk_num += 1;
        }
        let last_header = entry.get_blob_index_entry(chunk_num, data_len_u32);
        if last_header.len() > current_section.get_remaining_space() {
            self.start_new_section()?;
            current_section = &mut self.sections[self.current_section];
        }
        current_section.write_misc_data(&last_header, 1)?;
        Ok(())
    }

    fn write_namespace(&mut self) -> anyhow::Result<()> {
        let current_section = &mut self.sections[self.current_section];
        match current_section.write_namespace() {
            Ok(()) => Ok(()),
            Err(_) => {
                self.start_new_section()?;
                let current_section = &mut self.sections[self.current_section];
                current_section.write_namespace()
            }
        }
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut res = vec![];
        let total_sections = self.size / 4096 - 1;
        let empty_sections = total_sections - self.sections.len();
        for _ in 0..empty_sections {
            match self.start_new_section() {
                Ok(_) => {}
                Err(_) => unreachable!(),
            };
        }
        self.sections[self.current_section].close();
        let num_sections = self.sections.len();
        log::info!("Writing {:?} NVS pages", num_sections);
        for _ in 0..num_sections {
            let section = self.sections.pop_front().unwrap();
            res.append(&mut section.data.to_vec());
        }
        let mut reserved_section = std::iter::repeat(0xFF).take(4096).collect::<Vec<u8>>();
        res.append(&mut reserved_section);
        res
    }
}

impl TryFrom<&mut NVSPartition> for NVSPartitionData {
    type Error = anyhow::Error;
    fn try_from(value: &mut NVSPartition) -> Result<Self, Self::Error> {
        let mut nvs_inst = Self::new(value.size);
        nvs_inst.write_namespace()?;
        let total_entries = value.entries.len();
        for (i, entry) in value.entries.iter_mut().enumerate() {
            log::info!("writing entry {:?} of {:?}...", i + 1, total_entries);
            if entry.header[1] == STRING_VALUE_FORMAT {
                nvs_inst.write_string_entry(entry)?
            } else {
                nvs_inst.write_binary_entry(entry)?
            }
        }
        Ok(nvs_inst)
    }
}
