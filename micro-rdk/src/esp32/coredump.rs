use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use bytes::BytesMut;
use esp_idf_svc::sys::{
    esp_partition_erase_range, esp_partition_find_first, esp_partition_read,
    esp_partition_subtype_t_ESP_PARTITION_SUBTYPE_DATA_COREDUMP, esp_partition_t,
    esp_partition_type_t_ESP_PARTITION_TYPE_DATA,
};

use crate::{
    common::{
        config::ConfigType,
        generic::{DoCommand, GenericError},
        registry::{ComponentRegistry, Dependency},
        sensor::{GenericReadingsResult, Readings, Sensor, SensorError, SensorType},
        status::Status,
    },
    google::protobuf::{self, value::Kind, Struct, Value},
};

pub struct Coredump {
    len: usize,
    coredump_partition_ptr: &'static esp_partition_t,
}

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_sensor("coredump", &Coredump::from_config)
        .is_err()
    {
        log::error!("couldn't register coredump sensor");
    }
}

/// Instantiate a coredump sensor if : any partition of type data and subtype coredump can be found, we will use the first found partition is more than one exists (similar to the behavior of the coredump code in esp-idf)
/// A coredump is consider available if the 4th byte of the version field is not 0xFF (flash erase value)
/// The coredump is implemented as a sensor and can be downloaded through the DoCommand interface ( since GetReading cannot return a stream of bytes object)
impl Coredump {
    pub fn from_config(_: ConfigType, _: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let mut len = 0;
        // if a pointer is found it will be stored as a static reference since it will remain valid for the lifetime of the program
        let partition_iterator = unsafe {
            esp_partition_find_first(
                esp_partition_type_t_ESP_PARTITION_TYPE_DATA,
                esp_partition_subtype_t_ESP_PARTITION_SUBTYPE_DATA_COREDUMP,
                std::ptr::null(),
            )
            .as_ref()
            .ok_or(SensorError::SensorGenericError(
                "no coredump partition found",
            ))?
        };
        // the first two fields of the coredump are length (uint32_t) and version (uint32_t)
        let mut first_bytes = [0xFF_u8; 8];
        esp_idf_svc::sys::esp!(unsafe {
            esp_partition_read(partition_iterator, 0, first_bytes.as_mut_ptr() as *mut _, 8)
        })?;

        if first_bytes[7] != 0xFF {
            len = usize::from_le_bytes(first_bytes[0..4].try_into().unwrap()); // safe because we would have read 4 bytes
        }

        Ok(Arc::new(Mutex::new(Coredump {
            len,
            coredump_partition_ptr: partition_iterator,
        })))
    }
}

impl Sensor for Coredump {}

impl Readings for Coredump {
    fn get_generic_readings(
        &mut self,
    ) -> Result<crate::common::sensor::GenericReadingsResult, SensorError> {
        let has_coredump = self.len > 0;
        let res = GenericReadingsResult::from([(
            "has_coredump".to_owned(),
            Value {
                kind: Some(Kind::BoolValue(has_coredump)),
            },
        )]);
        Ok(res)
    }
}

impl Status for Coredump {
    fn get_status(
        &self,
    ) -> Result<Option<crate::google::protobuf::Struct>, crate::common::status::StatusError> {
        Ok(Some(protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

struct Null;

const CORE_FRAGMENT_SIZE: usize = 3072;

impl DoCommand for Coredump {
    fn do_command(
        &mut self,
        command_struct: Option<protobuf::Struct>,
    ) -> Result<Option<protobuf::Struct>, crate::common::generic::GenericError> {
        if let Some(cmd) = command_struct {
            // Coredump would be chunked into 4096 bytes encoded in Base64
            // len(bases64) = 4*(n/3) therefore n = (4096/4)*3 rounded down
            // which yield n = 3072
            if cmd.fields.get("sizes").is_some() {
                return Ok(Some(Struct {
                    fields: HashMap::from([
                        (
                            "nb_chunk".to_owned(),
                            Value {
                                kind: Some(Kind::NumberValue(
                                    (self.len / CORE_FRAGMENT_SIZE) as f64,
                                )),
                            },
                        ),
                        (
                            "len".to_owned(),
                            Value {
                                kind: Some(Kind::NumberValue(self.len as f64)),
                            },
                        ),
                    ]),
                }));
            }
            if cmd.fields.get("erase_coredump").is_some() {
                esp_idf_svc::sys::esp!(unsafe {
                    esp_partition_erase_range(
                        self.coredump_partition_ptr,
                        0_usize,
                        self.coredump_partition_ptr.size as usize,
                    )
                })
                .map_err(|e| GenericError::Other(e.into()))?;
                self.len = 0;
                return Ok(None);
            }
            if let Some(Kind::NumberValue(val)) = cmd
                .fields
                .get("get_nth_chunk")
                .and_then(|v| v.kind.as_ref())
            {
                val.is_sign_positive()
                    .then_some(Null)
                    .ok_or(GenericError::Other(
                        "chunk requested outside of bounds".into(),
                    ))?;

                let offset = (val.floor() as usize) * CORE_FRAGMENT_SIZE;

                offset
                    .lt(&self.len)
                    .then_some(Null)
                    .ok_or(GenericError::Other(
                        "chunk requested outside of bounds".into(),
                    ))?;

                let to_read: usize = CORE_FRAGMENT_SIZE.min(self.len - offset);
                let mut buf = BytesMut::with_capacity(CORE_FRAGMENT_SIZE);

                esp_idf_svc::sys::esp!(unsafe {
                    esp_partition_read(
                        self.coredump_partition_ptr,
                        offset,
                        buf.as_mut_ptr() as *mut _,
                        to_read,
                    )
                })
                .map_err(|e| GenericError::Other(e.into()))?;
                unsafe {
                    buf.set_len(to_read);
                }

                let as_b64 = STANDARD.encode(buf);
                return Ok(Some(Struct {
                    fields: HashMap::from([
                        (
                            "chunk".to_owned(),
                            Value {
                                kind: Some(Kind::NumberValue(val.floor())),
                            },
                        ),
                        (
                            "payload".to_owned(),
                            Value {
                                kind: Some(Kind::StringValue(as_b64)),
                            },
                        ),
                    ]),
                }));
            }
        }
        Err(GenericError::Other("couldn't parse request".into()))
    }
}
