use std::collections::HashMap;

use base64::{engine::general_purpose, Engine};
use micro_rdk::{
    common::sensor::GenericReadingsResult,
    google::protobuf::{value::Kind, Value},
};

use crate::parse_helpers::{
    errors::NmeaParseError,
    parsers::{DataCursor, FieldSet},
};

pub trait Message: Sized + Clone {
    const PGN: u32;
    fn from_cursor(cursor: DataCursor) -> Result<Self, NmeaParseError>;
    fn to_readings(self) -> Result<GenericReadingsResult, NmeaParseError>;
    fn pgn(&self) -> u32 {
        Self::PGN
    }
}

#[derive(Debug, Clone)]
pub struct UnparsedNmeaMessageBody {
    data: Vec<u8>,
    pgn: u32,
}

impl UnparsedNmeaMessageBody {
    pub fn from_bytes(data: Vec<u8>, pgn: u32) -> Result<Self, NmeaParseError> {
        Ok(Self { data, pgn })
    }

    pub fn to_readings(self) -> Result<GenericReadingsResult, NmeaParseError> {
        let data_string = general_purpose::STANDARD.encode(self.data);
        Ok(HashMap::from([
            (
                "data".to_string(),
                Value {
                    kind: Some(Kind::StringValue(data_string)),
                },
            ),
            (
                "pgn".to_string(),
                Value {
                    kind: Some(Kind::NumberValue(self.pgn as f64)),
                },
            ),
        ]))
    }

    pub fn pgn(&self) -> u32 {
        self.pgn
    }
}

pub trait PolymorphicPgnParent<T> {
    fn read_match_value(&self) -> T;
}

pub trait MessageVariant<T>: FieldSet {
    const MATCH_VALUE: T;
}
