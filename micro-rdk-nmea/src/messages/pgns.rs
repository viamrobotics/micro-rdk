use std::collections::HashMap;
use std::marker::PhantomData;

use micro_rdk::{
    common::sensor::GenericReadingsResult,
    google::protobuf::{value::Kind, Struct, Value},
};
use micro_rdk_nmea_macros::{FieldsetDerive, PgnMessageDerive};

use super::message::{Message, UnparsedNmeaMessageBody};
use crate::parse_helpers::{
    enums::{Gns, GnsIntegrity, GnsMethod, RangeResidualMode, SatelliteStatus, TemperatureSource},
    errors::NmeaParseError,
    parsers::{DataCursor, FieldSet, NmeaMessageMetadata},
};

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct WaterDepth {
    #[pgn = 128267]
    _pgn: PhantomData<u32>,
    source_id: u8,
    #[scale = 0.01]
    depth: u32,
    #[scale = 0.001]
    offset: i16,
    #[scale = 10.0]
    range: u8,
}

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct TemperatureExtendedRange {
    #[pgn = 130316]
    _pgn: PhantomData<u32>,
    source_id: u8,
    instance: u8,
    #[lookup]
    source: TemperatureSource,
    #[bits = 24]
    #[scale = 0.001]
    #[unit = "C"]
    temperature: u32,
    #[scale = 0.1]
    set_temperature: u16,
}

#[derive(FieldsetDerive, Clone, Debug)]
pub struct ReferenceStation {
    #[bits = 12]
    reference_station_id: u16,
    #[scale = 0.01]
    age_of_dgnss_corrections: u16,
}

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct GnssPositionData {
    #[pgn = 129029]
    _pgn: PhantomData<u32>,
    source_id: u8,
    date: u16,
    #[scale = 0.0001]
    time: u32,
    #[scale = 1e-16]
    latitude: i64,
    #[scale = 1e-16]
    longitude: i64,
    #[scale = 1e-06]
    altitude: i64,
    #[lookup]
    #[bits = 4]
    gnss_type: Gns,
    #[lookup]
    #[bits = 4]
    method: GnsMethod,
    #[lookup]
    #[bits = 2]
    integrity: GnsIntegrity,
    #[offset = 6]
    number_of_svs: u8,
    #[scale = 0.01]
    hdop: i16,
    #[scale = 0.01]
    pdop: i16,
    #[scale = 0.01]
    geoidal_separation: i32,
    reference_stations: u8,
    #[fieldset]
    #[length_field = "reference_stations"]
    reference_station_structs: Vec<ReferenceStation>,
}

#[derive(FieldsetDerive, Clone, Debug)]
pub struct Satellite {
    prn: u8,
    #[scale = 0.0001]
    #[unit = "deg"]
    elevation: i16,
    #[scale = 0.0001]
    #[unit = "deg"]
    azimuth: u16,
    #[scale = 0.01]
    snr: u16,
    range_residuals: i32,
    #[lookup]
    #[bits = 4]
    status: SatelliteStatus,
    // normally we would handle "reserved" fields by using the offset attribute
    // on the next field, but in the edge case of a reserved field being the last
    // field of a fieldset we need to include it
    #[bits = 4]
    reserved: u8,
}

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct GnssSatsInView {
    #[pgn = 129540]
    _pgn: PhantomData<u32>,
    source_id: u8,
    #[lookup]
    #[bits = 2]
    range_residual_mode: RangeResidualMode,
    #[offset = 6]
    sats_in_view: u8,
    #[fieldset]
    #[length_field = "sats_in_view"]
    satellites: Vec<Satellite>,
}

macro_rules! define_pgns {
    ( $($pgndef:ident),* ) => {
        #[derive(Clone, Debug)]
        pub enum NmeaMessageBody {
            $($pgndef($pgndef)),*,
            Unsupported(UnparsedNmeaMessageBody)
        }

        impl NmeaMessageBody {
            pub fn pgn(&self) -> u32 {
                match self {
                    $(Self::$pgndef(msg) => msg.pgn()),*,
                    Self::Unsupported(unparsed) => unparsed.pgn()
                }
            }

            pub fn from_bytes(pgn: u32, bytes: Vec<u8>) -> Result<Self, crate::parse_helpers::errors::NmeaParseError> {
                Ok(match pgn {
                    $($pgndef::PGN => {
                        let cursor = DataCursor::new(bytes);
                        Self::$pgndef($pgndef::from_cursor(cursor)?)
                    }),*,
                    x => Self::Unsupported(UnparsedNmeaMessageBody::from_bytes(bytes, x)?)
                })
            }

            pub fn to_readings(self) -> Result<GenericReadingsResult, crate::parse_helpers::errors::NmeaParseError> {
                match self {
                    $(Self::$pgndef(msg) => msg.to_readings()),*,
                    Self::Unsupported(msg) => msg.to_readings()
                }
            }
        }
    };
}

pub const MESSAGE_DATA_OFFSET: usize = 32;

define_pgns!(WaterDepth, TemperatureExtendedRange, GnssSatsInView);

pub struct NmeaMessage {
    pub metadata: NmeaMessageMetadata,
    pub data: NmeaMessageBody,
}

impl TryFrom<Vec<u8>> for NmeaMessage {
    type Error = NmeaParseError;
    fn try_from(mut value: Vec<u8>) -> Result<Self, Self::Error> {
        let msg_data = value.split_off(MESSAGE_DATA_OFFSET);
        let metadata = NmeaMessageMetadata::try_from(value)?;
        let data = NmeaMessageBody::from_bytes(metadata.pgn(), msg_data)?;
        Ok(Self { metadata, data })
    }
}

impl NmeaMessage {
    pub fn to_readings(self) -> Result<GenericReadingsResult, NmeaParseError> {
        Ok(HashMap::from([
            (
                "prio".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(self.metadata.priority() as f64)),
                },
            ),
            (
                "pgn".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(self.metadata.pgn() as f64)),
                },
            ),
            (
                "src".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(self.metadata.src() as f64)),
                },
            ),
            (
                "dst".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(self.metadata.dst() as f64)),
                },
            ),
            (
                "fields".to_string(),
                Value {
                    kind: Some(Kind::StructValue(Struct {
                        fields: self.data.to_readings()?,
                    })),
                },
            ),
        ]))
    }
}
