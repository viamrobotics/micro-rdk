//! Contains the DataStore trait and a usable StaticMemoryDataStore.
//! Implementers of the trait are meant to be written to by DataCollectors (RSDK-6992, RSDK-6994)
//! and read from by a task that uploads the data to app (RSDK-6995)

use crate::proto::app::data_sync::v1::SensorData;
use bytes::{Buf, BufMut, BytesMut};
use prost::{encoding::decode_varint, length_delimiter_len, DecodeError, EncodeError, Message};
use ringbuf::{ring_buffer::RbBase, Consumer, LocalRb, Producer};
use std::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

use super::data_collector::ResourceMethodKey;

#[derive(Debug, Clone, Copy)]
pub enum WriteMode {
    PreserveOrFail,
    OverwriteOldest,
}

impl Default for WriteMode {
    fn default() -> Self {
        Self::PreserveOrFail
    }
}

static mut DATA_STORE: [MaybeUninit<u8>; 1024000] = [MaybeUninit::uninit(); 1024000];

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error(transparent)]
    EncodingError(#[from] EncodeError),
    #[error("SensorDataTooLarge")]
    DataTooLarge,
    #[error("Store already initialized")]
    DataStoreInitialized,
    #[error("Data write failure")]
    DataWriteFailure,
    #[error("Buffer full")]
    DataBufferFull(ResourceMethodKey, SensorData),
    #[error("Current message is malformed")]
    DataIntegrityError,
    #[error("Unknown collector key: {0}")]
    UnknownCollectorKey(ResourceMethodKey),
    #[error(transparent)]
    DecodeError(#[from] DecodeError),
    #[error("unimplemented")]
    Unimplemented,
}

lazy_static::lazy_static! {
    static ref DATA_STORE_INITIALIZED: AtomicBool = AtomicBool::new(false);
}

pub trait DataStore {
    /// Store the sensor data message in a region specified by the ResourceMethodKey. To overwrite
    /// the oldest messages if necessary, pass true for `overwrite_old_data`
    fn write_message(
        &mut self,
        collector_key: &ResourceMethodKey,
        message: SensorData,
        write_mode: WriteMode,
    ) -> Result<(), DataStoreError>;

    /// Reads the next available message in the store for the given ResourceMethodKey. It should return
    /// an empty BytesMut with 0 capacity when there are no available messages left.
    fn read_next_message(
        &mut self,
        collector_key: &ResourceMethodKey,
    ) -> Result<BytesMut, DataStoreError>;
}

/// StaticMemoryDataStore is an entity that governs the static bytes memory
/// reserved in DATA_STORE. The memory is segmented based according to the DataCollectors expected
/// (identified by collector keys) and each segment view is treated as a separate ring buffer of SensorData
/// messages. Currently, an equal amount of space is alloted to each collector, which will affect
/// the maximum allowed size of a single message (computed as the length of DATA_STORE divided by
/// the number of collector keys). It should be treated as a global struct that should only be initialized once
/// and is not thread-safe (all interactions should be blocking).
pub struct StaticMemoryDataStore {
    buffers: Vec<LocalRb<u8, &'static mut [MaybeUninit<u8>]>>,
    collector_keys: Vec<ResourceMethodKey>,
}

impl StaticMemoryDataStore {
    pub fn new(collector_keys: Vec<ResourceMethodKey>) -> Result<Self, DataStoreError> {
        if !DATA_STORE_INITIALIZED.fetch_or(true, Ordering::SeqCst) {
            let len_per_buffer = unsafe { DATA_STORE.len() } / collector_keys.len();
            let mut buffers = Vec::new();
            for i in 0..(collector_keys.len()) {
                let start_idx = i * len_per_buffer;
                let end_idx = (i + 1) * len_per_buffer;
                unsafe {
                    buffers.push(LocalRb::from_raw_parts(
                        &mut DATA_STORE[start_idx..end_idx],
                        0,
                        0,
                    ));
                }
            }
            return Ok(Self {
                buffers,
                collector_keys,
            });
        }
        Err(DataStoreError::DataStoreInitialized)
    }

    fn get_index_for_collector(
        &self,
        collector_key: &ResourceMethodKey,
    ) -> Result<usize, DataStoreError> {
        self.collector_keys
            .iter()
            .position(|key| key == collector_key)
            .ok_or(DataStoreError::UnknownCollectorKey(collector_key.clone()))
    }
}

impl DataStore for StaticMemoryDataStore {
    fn write_message(
        &mut self,
        collector_key: &ResourceMethodKey,
        message: SensorData,
        write_mode: WriteMode,
    ) -> Result<(), DataStoreError> {
        let buffer_index = self.get_index_for_collector(collector_key)?;
        let buffer = &self.buffers[buffer_index];
        let encode_len = message.encoded_len();
        let total_encode_len = length_delimiter_len(encode_len) + encode_len;

        while total_encode_len > buffer.vacant_len() {
            if !matches!(write_mode, WriteMode::OverwriteOldest) {
                return Err(DataStoreError::DataBufferFull(
                    collector_key.clone(),
                    message,
                ));
            }
            let mut cons = unsafe { Consumer::new(buffer) };
            let (left, right) = cons.as_slices();
            let mut chained = Buf::chain(left, right);
            let encoded_len = decode_varint(&mut chained)? as usize;

            let advance = length_delimiter_len(encoded_len);
            unsafe { cons.advance(advance) };
            cons.skip(encoded_len);
        }
        unsafe {
            let mut prod = Producer::new(buffer);
            let (left, right) = prod.free_space_as_slices();
            let mut chained = BufMut::chain_mut(left, right);
            message.encode_length_delimited(&mut chained)?;
            prod.advance(total_encode_len);
        }

        Ok(())
    }
    fn read_next_message(
        &mut self,
        collector_key: &ResourceMethodKey,
    ) -> Result<BytesMut, DataStoreError> {
        let buffer_index = self.get_index_for_collector(collector_key)?;
        let buffer = &self.buffers[buffer_index];
        if buffer.is_empty() {
            return Ok(BytesMut::with_capacity(0));
        }

        let mut cons = unsafe { Consumer::new(buffer) };
        let (left, right) = cons.as_slices();
        let mut chained = Buf::chain(left, right);
        let encoded_len = decode_varint(&mut chained)? as usize;
        let advance = length_delimiter_len(encoded_len);
        unsafe { cons.advance(advance) };

        let mut msg_bytes = BytesMut::with_capacity(encoded_len);
        unsafe {
            msg_bytes.set_len(encoded_len);
        }
        cons.pop_slice(&mut msg_bytes);
        Ok(msg_bytes)
    }
}

impl Drop for StaticMemoryDataStore {
    fn drop(&mut self) {
        DATA_STORE_INITIALIZED.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::data_collector::{CollectionMethod, ResourceMethodKey};
    use crate::common::data_store::DataStore;
    use crate::common::data_store::DataStoreError;
    use crate::common::data_store::WriteMode;
    use crate::common::data_store::DATA_STORE;
    use crate::google::protobuf::Timestamp;
    use crate::google::protobuf::{value::Kind, Struct, Value};
    use crate::proto::app::data_sync::v1::sensor_data::Data;
    use crate::proto::app::data_sync::v1::{SensorData, SensorMetadata};
    use prost::{length_delimiter_len, Message};

    #[test_log::test]
    fn test_data_store() {
        let reading_requested_dt = chrono::offset::Local::now().fixed_offset();

        let empty_message = SensorData {
            metadata: None,
            data: None,
        };
        let thing_key = ResourceMethodKey {
            r_name: "thing".to_string(),
            component_type: "rdk::component::sensor".to_string(),
            method: CollectionMethod::Readings,
        };
        let empty_message_2 = SensorData {
            metadata: None,
            data: Some(Data::Struct(Struct {
                fields: HashMap::new(),
            })),
        };
        let thing_2_key = ResourceMethodKey {
            r_name: "thing".to_string(),
            component_type: "rdk::component::movement_sensor".to_string(),
            method: CollectionMethod::Readings,
        };
        let data_message_no_metadata = SensorData {
            metadata: None,
            data: Some(Data::Struct(Struct {
                fields: HashMap::from([
                    (
                        "thing_1".to_string(),
                        Value {
                            kind: Some(Kind::NumberValue(245.01)),
                        },
                    ),
                    (
                        "thing_2".to_string(),
                        Value {
                            kind: Some(Kind::BoolValue(true)),
                        },
                    ),
                ]),
            })),
        };
        let reading_received_dt = chrono::offset::Local::now().fixed_offset();
        let data_message = SensorData {
            metadata: Some(SensorMetadata {
                time_requested: Some(Timestamp {
                    seconds: reading_requested_dt.timestamp(),
                    nanos: reading_requested_dt.timestamp_subsec_nanos() as i32,
                }),
                time_received: Some(Timestamp {
                    seconds: reading_received_dt.timestamp(),
                    nanos: reading_received_dt.timestamp_subsec_nanos() as i32,
                }),
            }),
            data: Some(Data::Struct(Struct {
                fields: HashMap::from([
                    (
                        "thing_1".to_string(),
                        Value {
                            kind: Some(Kind::NumberValue(245.01)),
                        },
                    ),
                    (
                        "thing_2".to_string(),
                        Value {
                            kind: Some(Kind::BoolValue(true)),
                        },
                    ),
                ]),
            })),
        };
        let collector_keys = vec![thing_key.clone(), thing_2_key.clone()];
        let store = super::StaticMemoryDataStore::new(collector_keys);
        assert!(store.is_ok());
        let mut store = store.unwrap();

        let res = store.write_message(&thing_key, empty_message, Default::default());
        assert!(res.is_ok());
        let res = store.write_message(&thing_key, data_message, Default::default());
        assert!(res.is_ok());

        let res = store.write_message(&thing_2_key, empty_message_2, Default::default());
        assert!(res.is_ok());
        let res = store.write_message(&thing_2_key, data_message_no_metadata, Default::default());
        assert!(res.is_ok());

        let read_message = store.read_next_message(&thing_key);
        assert!(read_message.is_ok());
        let mut read_message = read_message.unwrap();
        let read_message = SensorData::decode(&mut read_message);
        assert!(read_message.is_ok());
        let read_message = read_message.unwrap();
        let expected_msg = SensorData {
            metadata: None,
            data: None,
        };
        assert_eq!(read_message, expected_msg);

        let read_message = store.read_next_message(&thing_key);
        assert!(read_message.is_ok());
        let mut read_message = read_message.unwrap();
        let read_message = SensorData::decode(&mut read_message);
        assert!(read_message.is_ok());
        let read_message = read_message.unwrap();
        let expected_msg = SensorData {
            metadata: Some(SensorMetadata {
                time_requested: Some(Timestamp {
                    seconds: reading_requested_dt.timestamp(),
                    nanos: reading_requested_dt.timestamp_subsec_nanos() as i32,
                }),
                time_received: Some(Timestamp {
                    seconds: reading_received_dt.timestamp(),
                    nanos: reading_received_dt.timestamp_subsec_nanos() as i32,
                }),
            }),
            data: Some(Data::Struct(Struct {
                fields: HashMap::from([
                    (
                        "thing_1".to_string(),
                        Value {
                            kind: Some(Kind::NumberValue(245.01)),
                        },
                    ),
                    (
                        "thing_2".to_string(),
                        Value {
                            kind: Some(Kind::BoolValue(true)),
                        },
                    ),
                ]),
            })),
        };
        assert_eq!(read_message, expected_msg);

        let read_message = store.read_next_message(&thing_2_key);
        assert!(read_message.is_ok());
        let mut read_message = read_message.unwrap();
        let read_message = SensorData::decode(&mut read_message);
        assert!(read_message.is_ok());
        let read_message = read_message.unwrap();
        let expected_msg = SensorData {
            metadata: None,
            data: Some(Data::Struct(Struct {
                fields: HashMap::new(),
            })),
        };
        assert_eq!(read_message, expected_msg);

        let read_message = store.read_next_message(&thing_2_key);
        assert!(read_message.is_ok());
        let mut read_message = read_message.unwrap();
        let read_message = SensorData::decode(&mut read_message);
        assert!(read_message.is_ok());
        let read_message = read_message.unwrap();
        let expected_msg = SensorData {
            metadata: None,
            data: Some(Data::Struct(Struct {
                fields: HashMap::from([
                    (
                        "thing_1".to_string(),
                        Value {
                            kind: Some(Kind::NumberValue(245.01)),
                        },
                    ),
                    (
                        "thing_2".to_string(),
                        Value {
                            kind: Some(Kind::BoolValue(true)),
                        },
                    ),
                ]),
            })),
        };
        assert_eq!(read_message, expected_msg);

        let read_message = store.read_next_message(&thing_2_key);
        assert!(read_message.is_ok());
        let read_message = read_message.unwrap();
        assert_eq!(read_message.len(), 0);

        let thing_key = ResourceMethodKey {
            r_name: "thing".to_string(),
            component_type: "rdk::component::sensor".to_string(),
            method: CollectionMethod::Readings,
        };
        let thing_2_key = ResourceMethodKey {
            r_name: "thing".to_string(),
            component_type: "rdk::component::movement_sensor".to_string(),
            method: CollectionMethod::Readings,
        };
        let collector_keys = vec![thing_key.clone(), thing_2_key.clone()];
        std::mem::drop(store);
        let store = super::StaticMemoryDataStore::new(collector_keys);
        assert!(store.is_ok());
        let mut store = store.unwrap();

        // test ring buffer wrap

        let data = SensorData {
            metadata: None,
            data: Some(Data::Struct(Struct {
                fields: HashMap::from([
                    (
                        "thing_1".to_string(),
                        Value {
                            kind: Some(Kind::NumberValue(245.01)),
                        },
                    ),
                    (
                        "thing_2".to_string(),
                        Value {
                            kind: Some(Kind::BoolValue(true)),
                        },
                    ),
                ]),
            })),
        };
        // size of message that we are about to repeatedly write
        let message_byte_size = data.encoded_len();
        let message_byte_size_total = length_delimiter_len(message_byte_size) + message_byte_size;

        // store was initialized with two keys, so the byte capacity is half the length of DATA_STORE
        let message_capacity_for_buffer: usize =
            unsafe { DATA_STORE.len() } / 2 / message_byte_size_total;

        // we want to prove that an additional two messages can only be written once the read pointer
        // has progressed
        let num_write_attempts = message_capacity_for_buffer + 2;

        let collector_key = ResourceMethodKey {
            r_name: "thing".to_string(),
            component_type: "rdk::component::sensor".to_string(),
            method: CollectionMethod::Readings,
        };
        for i in 0..num_write_attempts {
            let res = store.write_message(
                &collector_key,
                SensorData {
                    metadata: None,
                    data: Some(Data::Struct(Struct {
                        fields: HashMap::from([
                            (
                                "thing_1".to_string(),
                                Value {
                                    kind: Some(Kind::NumberValue(245.01)),
                                },
                            ),
                            (
                                "thing_2".to_string(),
                                Value {
                                    kind: Some(Kind::BoolValue(true)),
                                },
                            ),
                        ]),
                    })),
                },
                Default::default(),
            );
            if i < num_write_attempts - 2 {
                assert!(res.is_ok());
            } else {
                let expected_msg = SensorData {
                    metadata: None,
                    data: Some(Data::Struct(Struct {
                        fields: HashMap::from([
                            (
                                "thing_1".to_string(),
                                Value {
                                    kind: Some(Kind::NumberValue(245.01)),
                                },
                            ),
                            (
                                "thing_2".to_string(),
                                Value {
                                    kind: Some(Kind::BoolValue(true)),
                                },
                            ),
                        ]),
                    })),
                };
                match res {
                    Ok(()) => unreachable!(),
                    Err(DataStoreError::DataBufferFull(key, msg)) => {
                        assert_eq!(key, collector_key.clone());
                        assert_eq!(msg, expected_msg);
                    }
                    _ => unreachable!(),
                }
            }
        }

        for _ in 0..2 {
            let res = store.write_message(
                &collector_key,
                SensorData {
                    metadata: None,
                    data: Some(Data::Struct(Struct {
                        fields: HashMap::from([
                            (
                                "thing_1".to_string(),
                                Value {
                                    kind: Some(Kind::NumberValue(245.01)),
                                },
                            ),
                            (
                                "thing_2".to_string(),
                                Value {
                                    kind: Some(Kind::BoolValue(true)),
                                },
                            ),
                        ]),
                    })),
                },
                WriteMode::OverwriteOldest,
            );
            assert!(res.is_ok());
        }
    }
}
