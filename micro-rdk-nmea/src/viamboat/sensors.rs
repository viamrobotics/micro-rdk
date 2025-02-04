//! This is a collection of sensors that parse data useful to boats from a sensor providing
//! raw NMEA message data. The data is expected to be a hashmap with key-value pairs of the form
//! "<PGN in hex>-<source>": "<raw data encoded as a string in base64>" (example below)
//!
//! ```
//! {
//!     "1FD0A-16": "Cv0BAHg+gD8voKFnAAAAAIdQAwAAAAAACAD/ABYAAgBbAADgeg8A/w==",
//!     "1FD0C-23": "DP0BAHg+gD8voKFnAAAAAL+PAwAAAAAACAD/ACMAAgD/AABQkAT//w==",
//!     "1FD06-16": "Bv0BAHg+gD8voKFnAAAAAFpOAwAAAAAACAD/ABYAAgBb//////YD/w==",
//!     "1FD06-23": "Bv0BAHg+gD8voKFnAAAAAL1FBAAAAAAACAD/ACMAAgD/1HT//////w==",
//!     "1FD07-16": "B/0BAHg+gD8voKFnAAAAANNOBAAAAAAACAD/ABYAAgBb/////3/2Aw==",
//!     "1FD07-23": "B/0BAHg+gD8voKFnAAAAAI9kAwAAAAAACAD/ACMAAgD/wNR0/3///w=="
//! }
//! ```
//!
//! The raw sensor is extracted by name provided in the config under the "raw_pgn_sensor" key. Some sensors
//! also optionally take a list of PGNs and sources by which to filter the data from the raw sensor.

use std::{
    collections::HashMap,
    f64::consts::PI,
    sync::{Arc, Mutex},
};

use crate::{
    messages::pgns::{NmeaMessage, NmeaMessageBody},
    parse_helpers::{
        enums::DirectionReference,
        errors::{NmeaParseError, NumberFieldError},
    },
};
use base64::{engine::general_purpose, DecodeError, Engine};
use micro_rdk::{
    common::{
        config::ConfigType,
        math_utils::Vector3,
        movement_sensor::{
            GeoPosition, MovementSensor, MovementSensorSupportedMethods, MovementSensorType,
        },
        registry::{Dependency, ResourceKey},
        robot::Resource,
        sensor::{
            GenericReadingsResult, Readings, Sensor, SensorError, SensorType,
            COMPONENT_NAME as SensorCompName,
        },
        status::Status,
    },
    google::protobuf::{value::Kind, ListValue, Struct, Value},
    DoCommand, MovementSensorReadings,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ViamboatSensorError {
    #[error("value type was not string")]
    ValueTypeError,
    #[error("value was empty")]
    EmptyValueError,
    #[error("no available readings")]
    NoAvailableReadings,
    #[error(transparent)]
    NumberFieldError(#[from] NumberFieldError),
    #[error(transparent)]
    DecodeError(#[from] DecodeError),
    #[error(transparent)]
    ParsingError(#[from] NmeaParseError),
    #[error(transparent)]
    GatewayError(#[from] SensorError),
}

struct PgnGateway {
    sensor: SensorType,
}

fn readings_to_messages(
    readings: &GenericReadingsResult,
    pgns: Option<Vec<u32>>,
    sources: Option<Vec<u8>>,
) -> Result<Vec<NmeaMessage>, ViamboatSensorError> {
    let filtered_on_pgn = readings.iter().filter(|(&ref k, _)| {
        if let Some(pgns) = &pgns {
            let split_key: Vec<&str> = k.split_terminator("-").collect();
            if split_key.len() == 2 {
                if let Ok(msg_pgn) = u32::from_str_radix(split_key[0], 16) {
                    pgns.iter().any(|&x| x == msg_pgn)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            true
        }
    });

    let filtered_on_src = filtered_on_pgn.filter(|(&ref k, _)| {
        if let Some(sources) = &sources {
            let split_key: Vec<&str> = k.split_terminator("-").collect();
            if split_key.len() == 2 {
                if let Ok(src) = u32::from_str_radix(split_key[1], 10) {
                    sources.iter().any(|&x| (x as u32) == src)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            true
        }
    });

    let res: Result<Vec<NmeaMessage>, ViamboatSensorError> = filtered_on_src
        .map(|(_, v)| {
            let inner = v
                .kind
                .as_ref()
                .ok_or(ViamboatSensorError::EmptyValueError)?;
            match inner {
                Kind::StringValue(value_str) => {
                    let mut data = Vec::<u8>::new();
                    general_purpose::STANDARD.decode_vec(value_str.as_str(), &mut data)?;
                    Ok(NmeaMessage::try_from(data)?)
                }
                _ => Err(ViamboatSensorError::ValueTypeError),
            }
        })
        .collect();

    res.and_then(|msgs| {
        if msgs.is_empty() {
            Err(ViamboatSensorError::NoAvailableReadings)
        } else {
            Ok(msgs)
        }
    })
}

impl PgnGateway {
    fn retrieve_messages(
        &mut self,
        pgns: Option<Vec<u32>>,
        sources: Option<Vec<u8>>,
    ) -> Result<Vec<NmeaMessage>, ViamboatSensorError> {
        let all_pgns = self.sensor.get_generic_readings()?;
        readings_to_messages(&all_pgns, pgns, sources)
    }
}

pub fn get_raw_sensor_key(cfg: ConfigType) -> Vec<ResourceKey> {
    let mut dep_keys = vec![];
    if let Ok(sensor_name) = cfg.get_attribute::<String>("raw_pgn_sensor") {
        dep_keys.push(ResourceKey::new(SensorCompName, sensor_name));
    }
    dep_keys
}

fn get_raw_sensor_dependency(
    cfg: &ConfigType,
    deps: Vec<Dependency>,
) -> Result<SensorType, SensorError> {
    let raw_pgn_sensor_name = cfg.get_attribute::<String>("raw_pgn_sensor")?;
    let mut raw_pgn_sensor: Option<SensorType> = None;
    for Dependency(k, v) in deps {
        if let Resource::Sensor(found_sensor) = v {
            if k.1 == raw_pgn_sensor_name {
                let _ = raw_pgn_sensor.insert(found_sensor.clone());
            }
        }
    }
    raw_pgn_sensor.ok_or(SensorError::ConfigError("missing dependent raw pgn sensor"))
}

const DEPTH_PGN: u32 = 128267;

/// DepthSensor reports on the water depth below the boat
#[derive(DoCommand)]
pub struct DepthSensor {
    gateway: PgnGateway,
    sources: Option<Vec<u8>>,
}

impl DepthSensor {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let sources = cfg.get_attribute::<Vec<u8>>("sources").ok();
        Ok(Arc::new(Mutex::new(Self {
            gateway: PgnGateway {
                sensor: get_raw_sensor_dependency(&cfg, deps)?,
            },
            sources,
        })))
    }
}

impl Sensor for DepthSensor {}

impl Readings for DepthSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        let messages = self
            .gateway
            .retrieve_messages(Some(vec![DEPTH_PGN]), self.sources.clone())
            .map_err(|err| SensorError::SensorDriverError(err.to_string()))?;

        let first_message = &messages[0];
        let depth = match &first_message.data {
            NmeaMessageBody::WaterDepth(msg) => msg
                .depth()
                .map_err(|err| SensorError::SensorDriverError(err.to_string()))?,
            _ => unreachable!(),
        };
        Ok(HashMap::from([(
            "depth".to_string(),
            Value {
                kind: Some(Kind::NumberValue(depth)),
            },
        )]))
    }
}

impl Status for DepthSensor {
    fn get_status(
        &self,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::status::StatusError>
    {
        Ok(None)
    }
}

const POSITION_PGN: u32 = 129025;
const COG_SOG_PGN: u32 = 129026;
const VESSEL_HEADING_PGN: u32 = 127250;
const ATTITUDE_PGN: u32 = 127257;

/// ViamboatMovementSensor is a sensor that consolidates information about a boat
/// from a curated selection of NMEA messages. If no sources are provided, valid messages
/// from all sources will be extracted. For a given message PGN, if there are
/// multiple resulting messages from different sources, only information from the first
/// valid message will be used.
#[derive(DoCommand, MovementSensorReadings)]
pub struct ViamboatMovementSensor {
    gateway: PgnGateway,
    sources: Option<Vec<u8>>,
}

#[derive(Default)]
struct MovementSensorData {
    point: Option<GeoPosition>,
    speed_over_ground: Option<f64>,
    course_over_ground: Option<f64>,
    heading: Option<f64>,
    heading_reference: Option<DirectionReference>,
    yaw: Option<f64>,
    pitch: Option<f64>,
    roll: Option<f64>,
}

impl ViamboatMovementSensor {
    pub fn from_config(
        cfg: ConfigType,
        deps: Vec<Dependency>,
    ) -> Result<MovementSensorType, SensorError> {
        let sources = cfg.get_attribute::<Vec<u8>>("sources").ok();
        Ok(Arc::new(Mutex::new(Self {
            gateway: PgnGateway {
                sensor: get_raw_sensor_dependency(&cfg, deps)?,
            },
            sources,
        })))
    }

    fn get_data(&mut self) -> Result<MovementSensorData, ViamboatSensorError> {
        let messages = self.gateway.retrieve_messages(
            Some(vec![
                POSITION_PGN,
                COG_SOG_PGN,
                VESSEL_HEADING_PGN,
                ATTITUDE_PGN,
            ]),
            self.sources.clone(),
        )?;

        let mut res: MovementSensorData = Default::default();

        for msg in messages {
            match msg.data {
                NmeaMessageBody::PositionRapidUpdate(data) => {
                    if res.point.is_none() {
                        match data.latitude() {
                            Ok(lat) => match data.longitude() {
                                Ok(lon) => {
                                    let _ = res.point.insert(GeoPosition { lat, lon, alt: 0.0 });
                                }
                                Err(err) => {
                                    log::error!("error acquiring longitude: {:?}", err);
                                }
                            },
                            Err(err) => {
                                log::error!("error acquiring latitude: {:?}", err);
                            }
                        }
                    }
                }
                NmeaMessageBody::CogSog(data) => {
                    match data.speed_over_ground() {
                        Ok(sog) => {
                            let _ = res.speed_over_ground.get_or_insert(sog);
                        }
                        Err(err) => {
                            log::error!("error acquiring speed over ground: {:?}", err);
                        }
                    };
                    match data.course_over_ground() {
                        Ok(cog) => {
                            let _ = res.course_over_ground.get_or_insert(cog);
                        }
                        Err(err) => {
                            log::error!("error acquiring speed over ground: {:?}", err);
                        }
                    };
                }
                NmeaMessageBody::VesselHeading(data) => {
                    match data.heading() {
                        Ok(heading) => {
                            let _ = res.heading.get_or_insert(heading);
                        }
                        Err(err) => {
                            log::error!("error acquiring heading: {:?}", err);
                        }
                    };
                    let _ = res.heading_reference.get_or_insert(data.reference());
                }
                NmeaMessageBody::Attitude(data) => {
                    match data.yaw() {
                        Ok(yaw_deg) => {
                            let _ = res.yaw.get_or_insert(yaw_deg * (PI / 180.0));
                        }
                        Err(err) => {
                            log::error!("error acquiring yaw: {:?}", err);
                        }
                    };
                    match data.pitch() {
                        Ok(pitch_deg) => {
                            let _ = res.pitch.get_or_insert(pitch_deg * (PI / 180.0));
                        }
                        Err(err) => {
                            log::error!("error acquiring pitch: {:?}", err);
                        }
                    };
                    match data.roll() {
                        Ok(roll_deg) => {
                            let _ = res.roll.get_or_insert(roll_deg * (PI / 180.0));
                        }
                        Err(err) => {
                            log::error!("error acquiring roll: {:?}", err);
                        }
                    };
                }
                _ => unreachable!(),
            };
        }

        Ok(res)
    }
}

impl MovementSensor for ViamboatMovementSensor {
    fn get_position(&mut self) -> Result<GeoPosition, SensorError> {
        let data = self
            .get_data()
            .map_err(|err| SensorError::SensorDriverError(err.to_string()))?;
        match data.point {
            Some(pt) => Ok(pt),
            None => {
                log::error!("GPS data not yet available");
                Ok(GeoPosition {
                    lat: 0.0,
                    lon: 0.0,
                    alt: 0.0,
                })
            }
        }
    }
    fn get_linear_velocity(
        &mut self,
    ) -> Result<micro_rdk::common::math_utils::Vector3, SensorError> {
        let data = self
            .get_data()
            .map_err(|err| SensorError::SensorDriverError(err.to_string()))?;
        Ok(Vector3 {
            x: 0.0,
            y: data.speed_over_ground.unwrap_or_default(),
            z: 0.0,
        })
    }
    fn get_linear_acceleration(&mut self) -> Result<Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "linear acceleration not available",
        ))
    }
    fn get_angular_velocity(&mut self) -> Result<Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "angular velocity not available",
        ))
    }
    fn get_compass_heading(&mut self) -> Result<f64, SensorError> {
        let data = self
            .get_data()
            .map_err(|err| SensorError::SensorDriverError(err.to_string()))?;
        Ok(
            if data.heading.is_none() || data.speed_over_ground.is_some_and(|sog| sog > 1.0) {
                data.course_over_ground.unwrap_or_default()
            } else {
                data.heading.unwrap()
            },
        )
    }
    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: true,
            linear_velocity_supported: true,
            linear_acceleration_supported: false,
            angular_velocity_supported: false,
            compass_heading_supported: true,
        }
    }
}

impl Status for ViamboatMovementSensor {
    fn get_status(
        &self,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::status::StatusError>
    {
        Ok(None)
    }
}

/// PgnSensor provides message data for a list of PGNs and sources. If no PGNs are provided,
/// the messages will encompass all PGNs. If no sources are provided, the messages will encompass
/// all sources
#[derive(DoCommand)]
pub struct PgnSensor {
    gateway: PgnGateway,
    pgns: Vec<u32>,
    srcs: Vec<u8>,
}

impl PgnSensor {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let pgns = cfg.get_attribute::<Vec<u32>>("pgns").unwrap_or_default();
        let srcs = cfg.get_attribute::<Vec<u8>>("srcs").unwrap_or_default();
        Ok(Arc::new(Mutex::new(Self {
            gateway: PgnGateway {
                sensor: get_raw_sensor_dependency(&cfg, deps)?,
            },
            pgns,
            srcs,
        })))
    }
}

impl Sensor for PgnSensor {}

impl Readings for PgnSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        let pgns = if self.pgns.is_empty() {
            None
        } else {
            Some(self.pgns.clone())
        };
        let srcs = if self.srcs.is_empty() {
            None
        } else {
            Some(self.srcs.clone())
        };
        let messages = self.gateway.retrieve_messages(pgns, srcs);
        let readings = messages.map(|msgs| {
            let readings: Result<Vec<GenericReadingsResult>, NmeaParseError> =
                msgs.into_iter().map(|msg| msg.to_readings()).collect();
            readings.map_err(|err| ViamboatSensorError::from(err))
        });
        let readings = readings.and_then(|inner| inner).map(|readings_vec| {
            let value_mapped: Vec<Value> = readings_vec
                .into_iter()
                .map(|fields| Value {
                    kind: Some(Kind::StructValue(Struct { fields })),
                })
                .collect();
            value_mapped
        });
        match readings {
            Ok(readings) => Ok(HashMap::from([(
                "data".to_string(),
                Value {
                    kind: Some(Kind::ListValue(ListValue { values: readings })),
                },
            )])),
            Err(ViamboatSensorError::NoAvailableReadings) => Ok(HashMap::from([(
                "data".to_string(),
                Value {
                    kind: Some(Kind::ListValue(ListValue { values: vec![] })),
                },
            )])),
            Err(err) => Err(SensorError::SensorDriverError(err.to_string())),
        }
    }
}

impl Status for PgnSensor {
    fn get_status(
        &self,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::status::StatusError>
    {
        Ok(None)
    }
}

/// DebugPgnSensor provides the complete raw and parsed data, meant to be used for
/// debugging purposes.
#[derive(DoCommand)]
pub struct DebugPgnSensor {
    gateway: PgnGateway,
}

impl DebugPgnSensor {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        Ok(Arc::new(Mutex::new(Self {
            gateway: PgnGateway {
                sensor: get_raw_sensor_dependency(&cfg, deps)?,
            },
        })))
    }
}

impl Sensor for DebugPgnSensor {}

impl Readings for DebugPgnSensor {
    fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
        let all_pgns = self.gateway.sensor.get_generic_readings()?;
        let messages = readings_to_messages(&all_pgns, None, None)
            .map_err(|err| SensorError::SensorDriverError(err.to_string()))?;
        let message_protos: Result<Vec<Value>, NmeaParseError> = messages
            .into_iter()
            .map(|msg| {
                let readings = msg.to_readings().unwrap_or(HashMap::new());
                Ok(Value {
                    kind: Some(Kind::StructValue(Struct { fields: readings })),
                })
            })
            .collect();

        Ok(HashMap::from([
            (
                "raw".to_string(),
                Value {
                    kind: Some(Kind::StructValue(Struct { fields: all_pgns })),
                },
            ),
            (
                "parsed".to_string(),
                Value {
                    kind: Some(Kind::ListValue(ListValue {
                        values: message_protos
                            .map_err(|err| SensorError::SensorDriverError(err.to_string()))?,
                    })),
                },
            ),
        ]))
    }
}

impl Status for DebugPgnSensor {
    fn get_status(
        &self,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::status::StatusError>
    {
        Ok(None)
    }
}
