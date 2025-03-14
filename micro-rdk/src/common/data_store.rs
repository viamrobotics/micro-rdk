//! Contains the DataStore trait and a usable DefaultDataStore.
//! Implementers of the trait are meant to be written to by DataCollectors (RSDK-6992, RSDK-6994)
//! and read from by a task that uploads the data to app (RSDK-6995)

use crate::proto::app::data_sync::v1::SensorData;
use bytes::{Buf, BufMut, BytesMut};
use prost::{encoding::decode_varint, length_delimiter_len, DecodeError, EncodeError, Message};
use ringbuf::{ring_buffer::RbBase, Consumer, LocalRb, Producer, Rb};
use scopeguard::defer;
use std::{
    mem::MaybeUninit,
    rc::Rc,
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

#[derive(Clone, Error, Debug)]
pub enum DataStoreError {
    #[error("No collector keys supplied")]
    NoCollectors,
    #[error(transparent)]
    EncodingError(#[from] EncodeError),
    #[error("Maximum allowed capacity (64 KB) across data collectors exceeded")]
    MaxAllowedCapacity,
    #[error("Message for collector {0} was {1} bytes, exceeding allowed capacity of {2} bytes")]
    DataTooLarge(ResourceMethodKey, usize, usize),
    #[error("Data write failure")]
    DataWriteFailure,
    #[error("Buffer full")]
    DataBufferFull(ResourceMethodKey),
    #[error("Current message is malformed")]
    DataIntegrityError,
    #[error("Unknown collector key: {0}")]
    UnknownCollectorKey(ResourceMethodKey),
    #[error(transparent)]
    DecodeError(#[from] DecodeError),
    #[error("buffer for {0} in use")]
    BufferInUse(ResourceMethodKey),
    #[error("unimplemented")]
    Unimplemented,
}

/// A trait for an entity that is capable of reading from a store region without consuming
/// the messages until a command to flush the read messages is sent.
pub trait DataStoreReader {
    /// Reads the next available message in the store for the given ResourceMethodKey. It should return
    /// an empty BytesMut with 0 capacity when there are no available messages left.
    fn read_next_message(&mut self) -> Result<BytesMut, DataStoreError>;
    /// Returns the number of messages currently in the store region.
    fn messages_remaining(&self) -> Result<usize, DataStoreError>;
    fn flush(self);
}

pub trait DataStore {
    type Reader: DataStoreReader;

    /// Store the sensor data message in a region specified by the ResourceMethodKey. To overwrite
    /// the oldest messages if necessary, pass true for `overwrite_old_data`
    fn write_message(
        &mut self,
        collector_key: &ResourceMethodKey,
        message: SensorData,
        write_mode: WriteMode,
    ) -> Result<(), DataStoreError>;

    /// Initializes from resource-method keys.
    fn from_resource_method_settings(
        settings: Vec<(ResourceMethodKey, usize)>,
    ) -> Result<Self, DataStoreError>
    where
        Self: std::marker::Sized;

    // Gets a reader that should implement `DataStoreReader`
    fn get_reader(&self, collector_key: &ResourceMethodKey)
        -> Result<Self::Reader, DataStoreError>;
}

const MAX_ALLOWED_TOTAL_CAPACITY: usize = 64000;

pub type StoreRegion = Rc<LocalRb<u8, Vec<MaybeUninit<u8>>>>;

/// DefaultDataStore is a collection of ring-buffers for storing collected data. It should be
/// treated as a global struct that should only be initialized once and is not
/// thread-safe (all interactions should be blocking).
pub struct DefaultDataStore {
    buffers: Vec<StoreRegion>,
    buffer_usages: Vec<Rc<AtomicBool>>,
    collector_keys: Vec<ResourceMethodKey>,
}

pub struct DefaultDataStoreReader {
    cons: Consumer<u8, StoreRegion>,
    start_idx: usize,
    current_idx: usize,
    buffer_registration: Rc<AtomicBool>,
}

impl DefaultDataStoreReader {
    fn new(cons: Consumer<u8, StoreRegion>, buffer_registration: Rc<AtomicBool>) -> Self {
        let start_idx = cons.rb().head();
        Self {
            cons,
            start_idx,
            current_idx: start_idx,
            buffer_registration,
        }
    }
}

impl DataStoreReader for DefaultDataStoreReader {
    fn read_next_message(&mut self) -> Result<BytesMut, DataStoreError> {
        let (left, right) = self.cons.as_slices();
        let mut chained = Buf::chain(left, right);
        chained.advance(self.current_idx - self.start_idx);
        if !chained.has_remaining() {
            return Ok(BytesMut::with_capacity(0));
        }
        let encoded_len = decode_varint(&mut chained)? as usize;
        let len_len = length_delimiter_len(encoded_len);
        if encoded_len > chained.remaining() {
            return Err(DataStoreError::DataIntegrityError);
        }

        let mut msg_bytes = BytesMut::with_capacity(0);
        let chained_iter = chained.into_iter().take(encoded_len);
        msg_bytes.extend(chained_iter);
        self.current_idx += len_len + encoded_len;
        Ok(msg_bytes)
    }
    fn messages_remaining(&self) -> Result<usize, DataStoreError> {
        let mut messages = 0;
        let (left, right) = self.cons.as_slices();
        let mut chained = Buf::chain(left, right);
        chained.advance(self.current_idx - self.start_idx);
        loop {
            if !chained.has_remaining() {
                break;
            }
            let encoded_len = decode_varint(&mut chained)? as usize;
            if encoded_len > chained.remaining() {
                return Err(DataStoreError::DataIntegrityError);
            }
            chained.advance(encoded_len);
            messages += 1;
        }
        Ok(messages)
    }
    fn flush(mut self) {
        self.cons.skip(self.current_idx - self.start_idx);
    }
}

impl Drop for DefaultDataStoreReader {
    fn drop(&mut self) {
        self.buffer_registration.store(false, Ordering::Relaxed);
    }
}

impl DefaultDataStore {
    pub fn new(
        collector_settings: Vec<(ResourceMethodKey, usize)>,
    ) -> Result<Self, DataStoreError> {
        if collector_settings.is_empty() {
            return Err(DataStoreError::NoCollectors);
        }
        let mut buffers = Vec::new();
        let mut buffer_usages = Vec::new();
        let mut collector_keys = vec![];
        let mut total_capacity = 0;
        for (collector_key, capacity) in collector_settings {
            collector_keys.push(collector_key);
            if total_capacity + capacity > MAX_ALLOWED_TOTAL_CAPACITY {
                return Err(DataStoreError::MaxAllowedCapacity);
            } else {
                total_capacity += capacity;
            }
            buffers.push(Rc::new(LocalRb::new(capacity)));
            buffer_usages.push(Rc::new(AtomicBool::new(false)));
        }
        Ok(Self {
            buffers,
            buffer_usages,
            collector_keys,
        })
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

    fn buffer_in_use(&self, buffer_index: usize) -> bool {
        self.buffer_usages[buffer_index].load(Ordering::Relaxed)
    }

    fn register_buffer_usage(&self, buffer_index: usize) {
        self.buffer_usages[buffer_index].store(true, Ordering::Relaxed);
    }

    fn unregister_buffer_usage(&self, buffer_index: usize) {
        self.buffer_usages[buffer_index].store(false, Ordering::Relaxed);
    }

    // for testing purposes only
    #[allow(dead_code)]
    pub(crate) fn is_collector_store_empty(
        &self,
        collector_key: &ResourceMethodKey,
    ) -> Result<bool, DataStoreError> {
        let buffer_index = self.get_index_for_collector(collector_key)?;
        let buffer = Rc::clone(&self.buffers[buffer_index]);
        Ok(buffer.is_empty())
    }
}

impl DataStore for DefaultDataStore {
    type Reader = DefaultDataStoreReader;

    fn write_message(
        &mut self,
        collector_key: &ResourceMethodKey,
        message: SensorData,
        write_mode: WriteMode,
    ) -> Result<(), DataStoreError> {
        let buffer_index = self.get_index_for_collector(collector_key)?;
        let buffer = Rc::clone(&self.buffers[buffer_index]);
        if self.buffer_in_use(buffer_index) {
            return Err(DataStoreError::BufferInUse(collector_key.clone()));
        } else {
            self.register_buffer_usage(buffer_index);
        }
        defer! {
            self.unregister_buffer_usage(buffer_index);
        }
        let encode_len = message.encoded_len();
        let total_encode_len = length_delimiter_len(encode_len) + encode_len;

        // if the message is larger than the entire capacity of the buffer,
        // then it will wrap around and corrupt itself. So we should error when
        // the message is too large. The user can then reconfigure with a larger
        // cache size as a workaround
        let buffer_capacity = buffer.capacity();
        if encode_len > buffer_capacity {
            return Err(DataStoreError::DataTooLarge(
                collector_key.clone(),
                encode_len,
                buffer_capacity,
            ));
        }

        while total_encode_len > buffer.vacant_len() {
            if !matches!(write_mode, WriteMode::OverwriteOldest) {
                return Err(DataStoreError::DataBufferFull(collector_key.clone()));
            }
            let mut cons = unsafe { Consumer::new(buffer.clone()) };
            let (left, right) = cons.as_slices();
            let mut chained = Buf::chain(left, right);
            let encoded_len = decode_varint(&mut chained)? as usize;

            let advance = length_delimiter_len(encoded_len);
            unsafe { cons.advance(advance) };
            cons.skip(encoded_len);
        }
        unsafe {
            let mut prod = Producer::new(buffer.clone());
            let (left, right) = prod.free_space_as_slices();
            let mut chained = BufMut::chain_mut(left, right);
            message.encode_length_delimited(&mut chained)?;
            prod.advance(total_encode_len);
        }
        Ok(())
    }

    fn from_resource_method_settings(
        settings: Vec<(ResourceMethodKey, usize)>,
    ) -> Result<Self, DataStoreError> {
        Self::new(settings)
    }

    fn get_reader(
        &self,
        collector_key: &ResourceMethodKey,
    ) -> Result<DefaultDataStoreReader, DataStoreError> {
        let buffer_index = self.get_index_for_collector(collector_key)?;
        if self.buffer_in_use(buffer_index) {
            return Err(DataStoreError::BufferInUse(collector_key.clone()));
        }
        self.register_buffer_usage(buffer_index);
        let buffer = Rc::clone(&self.buffers[buffer_index]);
        let buffer_registration = Rc::clone(&self.buffer_usages[buffer_index]);

        Ok(DefaultDataStoreReader::new(
            unsafe { Consumer::new(buffer) },
            buffer_registration,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Instant;

    use crate::common::data_collector::{CollectionMethod, ResourceMethodKey};
    use crate::common::data_store::DataStore;
    use crate::common::data_store::DataStoreError;
    use crate::common::data_store::DataStoreReader;
    use crate::common::data_store::WriteMode;
    use crate::google::protobuf::Timestamp;
    use crate::google::protobuf::{value::Kind, Struct, Value};
    use crate::proto::app::data_sync::v1::sensor_data::Data;
    use crate::proto::app::data_sync::v1::{MimeType, SensorData, SensorMetadata};
    use prost::{length_delimiter_len, Message};
    use rand::distributions::Alphanumeric;
    use rand::Rng;

    #[test_log::test]
    fn test_data_store() {
        // test failure on attempt to initialize with no collectors
        let collector_keys: Vec<(ResourceMethodKey, usize)> = vec![];
        let store_create_attempt = super::DefaultDataStore::new(collector_keys);
        assert!(matches!(
            store_create_attempt,
            Err(DataStoreError::NoCollectors)
        ));

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
                mime_type: MimeType::Unspecified.into(),
                annotations: None,
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
        let collector_keys = vec![(thing_key.clone(), 5120), (thing_2_key.clone(), 5120)];
        let store = super::DefaultDataStore::new(collector_keys);
        assert!(store.is_ok());
        let mut store = store.unwrap();

        let res = store.write_message(&thing_key, empty_message, Default::default());
        assert!(res.is_ok());
        let res = store.write_message(&thing_key, data_message, Default::default());
        assert!(res.is_ok());
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
                mime_type: MimeType::Unspecified.into(),
                annotations: None,
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
        let res = store.write_message(&thing_key, data_message, Default::default());
        assert!(res.is_ok());

        let res = store.write_message(&thing_2_key, empty_message_2, Default::default());
        assert!(res.is_ok());
        let res = store.write_message(&thing_2_key, data_message_no_metadata, Default::default());
        assert!(res.is_ok());

        let reader = store.get_reader(&thing_key);
        assert!(reader.is_ok());
        let mut reader = reader.unwrap();

        let num_msgs = reader.messages_remaining();
        assert!(num_msgs.is_ok());
        let num_msgs = num_msgs.unwrap();
        assert_eq!(num_msgs, 3);

        let read_message = reader.read_next_message();
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

        let read_message = reader.read_next_message();
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
                mime_type: MimeType::Unspecified.into(),
                annotations: None,
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

        let read_message = reader.read_next_message();
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
                mime_type: MimeType::Unspecified.into(),
                annotations: None,
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

        let reader_2 = store.get_reader(&thing_2_key);
        assert!(reader_2.is_ok());
        let mut reader_2 = reader_2.unwrap();

        let num_msgs = reader_2.messages_remaining();
        assert!(num_msgs.is_ok());
        let num_msgs = num_msgs.unwrap();
        assert_eq!(num_msgs, 2);

        let read_message = reader_2.read_next_message();
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

        let read_message = reader_2.read_next_message();
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

        let read_message = reader_2.read_next_message();
        assert!(read_message.is_ok());
        let read_message = read_message.unwrap();
        assert_eq!(read_message.len(), 0);

        reader.flush();
        let region_empty = store.is_collector_store_empty(&thing_key);
        assert!(region_empty.is_ok());
        assert!(region_empty.unwrap());
        reader_2.flush();
        let region_empty = store.is_collector_store_empty(&thing_2_key);
        assert!(region_empty.is_ok());
        assert!(region_empty.unwrap());

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
        let collector_capacity_bytes = 5120;
        let collector_keys = vec![
            (thing_key.clone(), collector_capacity_bytes),
            (thing_2_key.clone(), collector_capacity_bytes),
        ];
        std::mem::drop(store);
        let store = super::DefaultDataStore::new(collector_keys);
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

        let message_capacity_for_buffer: usize = collector_capacity_bytes / message_byte_size_total;

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
                match res {
                    Ok(()) => unreachable!(),
                    Err(DataStoreError::DataBufferFull(key)) => {
                        assert_eq!(key, collector_key.clone());
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

    #[test_log::test]
    fn test_error_on_message_too_large() {
        use crate::google::protobuf::value::Kind::{StringValue, StructValue};
        let thing_key = ResourceMethodKey {
            r_name: "thing".to_string(),
            component_type: "rdk::component::sensor".to_string(),
            method: CollectionMethod::Readings,
        };
        let collector_keys = vec![(thing_key.clone(), 8000)];
        let data_store = super::DefaultDataStore::new(collector_keys);
        assert!(data_store.is_ok());
        let mut data_store = data_store.unwrap();

        let time = Instant::now();
        let mut fields = HashMap::new();
        for i in 0..100 {
            let value: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(300)
                .map(char::from)
                .collect();
            fields.insert(
                format!("thingo-{i}"),
                Value {
                    kind: Some(StringValue(value)),
                },
            );
        }

        let inner_data = HashMap::from([(
            "readings".to_string(),
            Value {
                kind: Some(StructValue(Struct { fields })),
            },
        )]);

        let reading_received_ts = time.elapsed();
        let data = Data::Struct(Struct { fields: inner_data });
        let message = SensorData {
            metadata: Some(SensorMetadata {
                time_received: Some(Timestamp {
                    seconds: reading_received_ts.as_secs() as i64,
                    nanos: reading_received_ts.subsec_nanos() as i32,
                }),
                time_requested: Some(Timestamp {
                    seconds: reading_received_ts.as_secs() as i64,
                    nanos: reading_received_ts.subsec_nanos() as i32,
                }),
                mime_type: MimeType::Unspecified.into(),
                annotations: None,
            }),
            data: Some(data),
        };

        assert!(matches!(
            data_store.write_message(&thing_key, message, WriteMode::OverwriteOldest),
            Err(DataStoreError::DataTooLarge(_, _, _))
        ));
    }
}
