//! Contains the DataStore trait and a usable StaticMemoryDataStore.
//! Implementers of the trait are meant to be written to by DataCollectors (RSDK-6992, RSDK-6994)
//! and read from by a task that uploads the data to app (RSDK-6995)

use crate::proto::app::data_sync::v1::DataCaptureUploadRequest;
use bytes::BytesMut;
use prost::{EncodeError, Message};
use std::sync::{Arc, Mutex};
use thiserror::Error;

static mut DATA_OFFSETS: [(usize, usize); 25600] = [(0, 0); 25600];
static mut DATA_STORE: [u8; 1024000] = [0xFF; 1024000];

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error(transparent)]
    EncodingError(#[from] EncodeError),
    #[error("DataCaptureUploadRequestTooLarge")]
    DataTooLarge,
    #[error("unimplemented")]
    Unimplemented,
}

lazy_static::lazy_static! {
    static ref DATA_STORE_VIEW: Arc<Mutex<StaticMemoryDataStore>> = StaticMemoryDataStore::new();
}

pub fn get_reference_to_static_data_store() -> Arc<Mutex<dyn DataStore>> {
    DATA_STORE_VIEW.clone()
}

pub trait DataStore {
    /// Attempts to store all of requests in the input vector. Any requests unable to be written
    /// due to exceeding capacity are returned in the result.
    fn store_upload_requests(
        &mut self,
        requests: Vec<DataCaptureUploadRequest>,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataStoreError>;
    fn read_messages<'a>(
        &mut self,
        number_of_messages: usize,
    ) -> Result<&'a [DataCaptureUploadRequest], DataStoreError>;
    /// WARNING: implementations of clear are meant to reset the entire data store. Must
    /// only be called when it is guaranteed that no other process has access to the data store.
    fn clear(&mut self);
}

impl<T> DataStore for Mutex<T>
where
    T: ?Sized + DataStore,
{
    fn store_upload_requests(
        &mut self,
        requests: Vec<DataCaptureUploadRequest>,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataStoreError> {
        self.get_mut().unwrap().store_upload_requests(requests)
    }

    fn read_messages<'a>(
        &mut self,
        number_of_messages: usize,
    ) -> Result<&'a [DataCaptureUploadRequest], DataStoreError> {
        self.get_mut().unwrap().read_messages(number_of_messages)
    }

    fn clear(&mut self) {
        self.get_mut().unwrap().clear()
    }
}

impl<T> DataStore for Arc<Mutex<T>>
where
    T: ?Sized + DataStore,
{
    fn store_upload_requests(
        &mut self,
        requests: Vec<DataCaptureUploadRequest>,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataStoreError> {
        self.lock().unwrap().store_upload_requests(requests)
    }

    fn read_messages<'a>(
        &mut self,
        number_of_messages: usize,
    ) -> Result<&'a [DataCaptureUploadRequest], DataStoreError> {
        self.lock().unwrap().read_messages(number_of_messages)
    }

    fn clear(&mut self) {
        self.lock().unwrap().clear()
    }
}

/// StaticMemoryDataStore is an entity that governs the bytes memory
/// reserved in DATA_STORE and treats it like a ring buffer of DataCaptureUploadRequests.
/// When a new message written via store_upload_requests is larger than the remaining size left in
/// DATA_STORE it wraps around to overwrite some or all of the oldest message. Subsequent writes
/// then continue to overwrite. This means when a message is partially destroyed,
/// its remaining data is inaccessible as the boundaries of the message in DATA_OFFSETS
/// will be overwritten
#[derive(Clone, Copy)]
struct StaticMemoryDataStore {
    reader_index: usize,
    writer_index: usize,
    write_message_ptr: usize,
    read_message_ptr: usize,
}

impl StaticMemoryDataStore {
    fn new() -> Arc<Mutex<Self>> {
        let last_message_ptr = unsafe {
            DATA_OFFSETS
                .iter()
                .rev()
                .position(|&x| (x.0 != 0) || (x.1 != 0))
                .unwrap_or_default()
        };
        let writer_index = unsafe { DATA_OFFSETS[last_message_ptr].1 };
        Arc::new(Mutex::new(StaticMemoryDataStore {
            reader_index: 0,
            writer_index,
            write_message_ptr: last_message_ptr + 1,
            read_message_ptr: 0,
        }))
    }

    /// Helper function for when we've wrapped around to the beginning of DATA_STORE when writing a
    /// new message. We essentially want to remove all stale offsets in DATA_OFFSETS and shift the remaining valid
    /// offsets (and the read_message_ptr) to the left accordingly
    unsafe fn adjust_overlap_index(
        &mut self,
        new_first_message_end_idx: usize,
    ) -> Result<(), DataStoreError> {
        let new_second_message_ptr = DATA_OFFSETS[1..]
            .iter()
            .position(|&x| x.0 > new_first_message_end_idx)
            .map(|s| s + 1);
        if let Some(new_second_message_ptr) = new_second_message_ptr {
            if new_second_message_ptr > self.read_message_ptr {
                return Err(DataStoreError::DataTooLarge);
            }
            let last_shift_idx = DATA_OFFSETS.len() - new_second_message_ptr;
            for (i, elem) in DATA_OFFSETS[1..last_shift_idx].iter_mut().enumerate() {
                let new_elem_idx = new_second_message_ptr + i;
                *elem = (DATA_OFFSETS[new_elem_idx].0, DATA_OFFSETS[new_elem_idx].1);
            }
            for elem in DATA_OFFSETS[last_shift_idx..].iter_mut() {
                *elem = (0, 0);
            }
            self.read_message_ptr -= new_second_message_ptr;
            Ok(())
        } else {
            Err(DataStoreError::DataTooLarge)
        }
    }
}

impl DataStore for StaticMemoryDataStore {
    fn store_upload_requests(
        &mut self,
        requests: Vec<DataCaptureUploadRequest>,
    ) -> Result<Vec<DataCaptureUploadRequest>, DataStoreError> {
        let mut return_remaining = false;
        let mut res = vec![];
        for req in requests {
            // if we've previously overtaken the read pointer, the rest of the requests should simply be returned
            if return_remaining {
                res.push(req);
                continue;
            }
            let encode_len = req.encoded_len();
            if encode_len > unsafe { DATA_STORE.len() / 2 } {
                return Err(DataStoreError::DataTooLarge);
            }
            let new_write_msg_ptr = unsafe {
                if self.writer_index + encode_len < DATA_STORE.len() {
                    self.write_message_ptr + 1
                } else {
                    1
                }
            };

            // we are about to overtake the read pointer, stop writing
            if new_write_msg_ptr == self.read_message_ptr {
                return_remaining = true;
                continue;
            }

            let mut buf = BytesMut::with_capacity(req.encoded_len());
            req.encode(&mut buf)?;
            unsafe {
                self.writer_index = if self.writer_index + encode_len < DATA_STORE.len() {
                    let region =
                        &mut DATA_STORE[self.writer_index..(self.writer_index + encode_len)];
                    for (idx, val) in buf.into_iter().enumerate() {
                        region[idx] = val;
                    }
                    DATA_OFFSETS[self.write_message_ptr] =
                        (self.writer_index, self.writer_index + encode_len);
                    self.writer_index + encode_len
                } else {
                    // we are about to overflow, wrap to beginning and overwrite oldest messages
                    let region = &mut DATA_STORE[self.writer_index..];
                    let wrap_idx = (encode_len + self.writer_index) - DATA_STORE.len();
                    let region_2 = &mut DATA_STORE[0..wrap_idx];
                    for (idx, val) in buf.into_iter().enumerate() {
                        if (idx + self.writer_index) < DATA_STORE.len() {
                            region[idx] = val;
                        } else {
                            region_2[idx - region.len()] = val;
                        }
                    }
                    DATA_OFFSETS[0] = (self.writer_index, wrap_idx);
                    self.adjust_overlap_index(wrap_idx)?;
                    wrap_idx
                };
            };
            self.write_message_ptr = new_write_msg_ptr;
        }
        Ok(res)
    }

    fn read_messages<'a>(
        &mut self,
        number_of_messages: usize,
    ) -> Result<&'a [DataCaptureUploadRequest], DataStoreError> {
        // advance reader temporarily for testing write
        self.read_message_ptr += number_of_messages;
        unsafe {
            self.reader_index = DATA_OFFSETS[self.read_message_ptr].0;
        }
        Err(DataStoreError::Unimplemented)
    }

    fn clear(&mut self) {
        unsafe {
            DATA_STORE = [0xFF; 1024000];
            DATA_OFFSETS = [(0, 0); 25600];
        }
        self.reader_index = 0;
        self.writer_index = 0;
        self.write_message_ptr = 0;
        self.read_message_ptr = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::DATA_OFFSETS;
    use crate::common::data_store::get_reference_to_static_data_store;
    use crate::google::protobuf::{value::Kind, Struct, Value};
    use crate::proto::app::data_sync::v1::{sensor_data::Data, DataType, UploadMetadata};
    use crate::proto::app::data_sync::v1::{DataCaptureUploadRequest, SensorData};
    use prost::Message;

    #[test_log::test]
    fn test_write_message() {
        let store_clone = get_reference_to_static_data_store();
        let mut store = store_clone.lock().unwrap();

        let mut requests = vec![];
        store.clear();
        let msg_1 = DataCaptureUploadRequest {
            metadata: None,
            sensor_contents: vec![],
        };
        let msg_len_1 = msg_1.encoded_len();
        requests.push(msg_1);
        let msg_2 = DataCaptureUploadRequest {
            metadata: Some(UploadMetadata {
                part_id: "part_id".to_string(),
                component_type: "component_a".to_string(),
                component_name: "test_comp".to_string(),
                method_name: "do_it".to_string(),
                r#type: DataType::TabularSensor.into(),
                ..Default::default()
            }),
            sensor_contents: vec![],
        };
        let msg_len_2 = msg_2.encoded_len();
        requests.push(msg_2);
        let msg_3 = DataCaptureUploadRequest {
            metadata: None,
            sensor_contents: vec![SensorData {
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
            }],
        };
        let msg_len_3 = msg_3.encoded_len();
        requests.push(msg_3);

        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);

        let mut expected_data_offsets: [(usize, usize); 25600] = [(0, 0); 25600];
        expected_data_offsets[0] = (0, msg_len_1);
        expected_data_offsets[1] = (msg_len_1, msg_len_1 + msg_len_2);
        expected_data_offsets[2] = (msg_len_1 + msg_len_2, msg_len_1 + msg_len_2 + msg_len_3);
        unsafe {
            assert_eq!(expected_data_offsets, DATA_OFFSETS);
        }
        store.clear();
    }

    #[test_log::test]
    fn test_write_message_wrap() {
        let store_clone = get_reference_to_static_data_store();
        let mut store = store_clone.lock().unwrap();
        store.clear();

        let num_of_initial_messages: usize = 24975;
        let mut requests = vec![];
        for _ in 0..num_of_initial_messages {
            requests.push(DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![SensorData {
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
                }],
            });
        }
        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);

        // advance the read pointer so we don't return before overwrite
        let _ = store.read_messages(2);

        let msg = DataCaptureUploadRequest {
            metadata: None,
            sensor_contents: vec![SensorData {
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
            }],
        };
        assert!(store.store_upload_requests(vec![msg]).is_ok());
        unsafe {
            assert_eq!((1023975, 16), DATA_OFFSETS[0]);
        }
        store.clear();
    }

    #[test_log::test]
    fn test_write_message_wrap_over_multiple_messages() {
        let store_clone = get_reference_to_static_data_store();
        let mut store = store_clone.lock().unwrap();
        store.clear();

        let num_of_initial_messages: usize = 24975;
        let mut requests = vec![];
        for _ in 0..num_of_initial_messages {
            requests.push(DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![SensorData {
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
                }],
            });
        }
        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);

        // advance the read pointer so we don't return before overwrite
        let _ = store.read_messages(58);

        let mut sensor_data = HashMap::new();
        for idx in 0..100 {
            sensor_data.insert(
                format!("thing_{:?}", idx),
                Value {
                    kind: Some(Kind::NumberValue(245.01)),
                },
            );
        }
        // encoded length of this message is 2296 bytes
        let msg = DataCaptureUploadRequest {
            metadata: None,
            sensor_contents: vec![SensorData {
                metadata: None,
                data: Some(Data::Struct(Struct {
                    fields: sensor_data,
                })),
            }],
        };
        assert!(store.store_upload_requests(vec![msg]).is_ok());
        unsafe {
            assert_eq!((1023975, 2271), DATA_OFFSETS[0]);
            assert_eq!((2296, 2337), DATA_OFFSETS[1]);
            assert_eq!((2337, 2378), DATA_OFFSETS[2]);
        }
        store.clear();
    }

    #[test_log::test]
    fn test_write_pointer_does_not_overtake_read_pointer() {
        let store_clone = get_reference_to_static_data_store();
        let mut store = store_clone.lock().unwrap();
        store.clear();
        let num_of_initial_messages: usize = 3;

        let mut requests = vec![];
        for _ in 0..num_of_initial_messages {
            requests.push(DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![SensorData {
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
                }],
            });
        }
        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 0);

        let _ = store.read_messages(4);

        let mut requests = vec![];
        for _ in 0..2 {
            requests.push(DataCaptureUploadRequest {
                metadata: None,
                sensor_contents: vec![SensorData {
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
                }],
            });
        }
        let res = store.store_upload_requests(requests);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 1);
        store.clear()
    }
}
