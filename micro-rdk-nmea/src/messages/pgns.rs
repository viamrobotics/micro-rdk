#![allow(unused_macros)]

macro_rules! impl_match {
    ( $matchtype:ident, $parent:ty, ($($prop:ident),*), $(($variant:ty, $(($prop2:ident, $val:expr)),*)),* ) => {
        impl $crate::messages::message::PolymorphicPgnParent<$matchtype> for $parent {
            fn read_match_value(&self) -> $matchtype {
                $matchtype {
                    $($prop: self.$prop(),)*
                }
            }
        }

        #[derive(Debug, Clone)]
        pub enum $variantlabel {
            $($varianttylabel($variantty)),*,
        }

        #[derive(Debug, Clone)]
        pub struct $messagelabel {
            base: $parent,
            variant: $variantlabel
        }

        impl $crate::messages::message::Message for $messagelabel {
            const PGN: u32 = $pgn;
            fn from_cursor(mut cursor: $crate::parse_helpers::parsers::DataCursor) -> Result<Self, $crate::parse_helpers::errors::NmeaParseError> {
                let base = $parent::from_data(&mut cursor)?;
                let match_value = base.read_match_value();
                let variant = match match_value {
                    $(
                        <$variantty>::MATCH_VALUE => {
                            Ok($variantlabel::$varianttylabel(<$variantty>::from_data(&mut cursor)?))
                        }
                    ),*
                    _ => Err($crate::parse_helpers::errors::NmeaParseError::UnsupportedMatchValue)
                }?;
                Ok(Self { base, variant })
            }
            fn to_readings(self) -> Result<GenericReadingsResult, $crate::parse_helpers::errors::NmeaParseError> {
                let mut base_readings = self.base.to_readings()?;
                let variant_readings = match &self.variant {
                    $(
                        $variantlabel::$varianttylabel(var) => {
                            var.to_readings()
                        }
                    ),*
                }?;
                base_readings.extend(variant_readings);
                Ok(base_readings)
            }
        }
    };
}

macro_rules! define_proprietary_pgn {
    ( $pgn:expr, $messagelabel:ident, $parent:ident, $variantlabel:ident, $matchval:ty, $($varianttylabel:ident, $variantty:ty),* ) => {

        #[derive(Debug, Clone)]
        pub enum $variantlabel {
            $($varianttylabel($variantty)),*,
        }

        #[derive(Debug, Clone)]
        pub struct $messagelabel {
            base: $parent,
            variant: $variantlabel
        }

        impl $crate::messages::message::Message for $messagelabel {
            const PGN: u32 = $pgn;
            fn from_cursor(mut cursor: $crate::parse_helpers::parsers::DataCursor) -> Result<Self, $crate::parse_helpers::errors::NmeaParseError> {
                let base = $parent::from_data(&mut cursor)?;
                let match_value = base.read_match_value();
                let variant = match match_value {
                    $(
                        <$variantty>::MATCH_VALUE => {
                            Ok($variantlabel::$varianttylabel(<$variantty>::from_data(&mut cursor)?))
                        }
                    ),*
                    _ => Err($crate::parse_helpers::errors::NmeaParseError::UnsupportedMatchValue)
                }?;
                Ok(Self { base, variant })
            }
            fn to_readings(self) -> Result<GenericReadingsResult, $crate::parse_helpers::errors::NmeaParseError> {
                let mut base_readings = self.base.to_readings()?;
                let variant_readings = match &self.variant {
                    $(
                        $variantlabel::$varianttylabel(var) => {
                            var.to_readings()
                        }
                    ),*
                }?;
                base_readings.extend(variant_readings);
                Ok(base_readings)
            }
        }
    };
}

#[macro_export]
macro_rules! define_pgns {
    ( $($pgndef:ident),* ) => {
        #[derive(Clone, Debug)]
        pub enum NmeaMessageBody {
            $($pgndef($pgndef)),*,
            Unsupported($crate::messages::message::UnparsedNmeaMessageBody)
        }

        impl NmeaMessageBody {
            pub fn pgn(&self) -> u32 {
                match self {
                    $(Self::$pgndef(msg) => msg.pgn_id()),*,
                    Self::Unsupported(unparsed) => unparsed.pgn_id()
                }
            }

            pub fn from_bytes(pgn: u32, bytes: Vec<u8>) -> Result<Self, $crate::parse_helpers::errors::NmeaParseError> {
                Ok(match pgn {
                    $($pgndef::PGN => {
                        let cursor = DataCursor::new(bytes);
                        Self::$pgndef($pgndef::from_cursor(cursor)?)
                    }),*,
                    x => Self::Unsupported($crate::messages::message::UnparsedNmeaMessageBody::from_bytes(bytes, x)?)
                })
            }

            pub fn to_readings(self) -> Result<GenericReadingsResult, $crate::parse_helpers::errors::NmeaParseError> {
                match self {
                    $(Self::$pgndef(msg) => msg.to_readings()),*,
                    Self::Unsupported(msg) => msg.to_readings()
                }
            }
        }

        pub const MESSAGE_DATA_OFFSET: usize = 32;

        pub struct NmeaMessage {
            pub(crate) metadata: NmeaMessageMetadata,
            pub(crate) data: NmeaMessageBody,
        }

        impl TryFrom<Vec<u8>> for NmeaMessage {
            type Error = $crate::parse_helpers::errors::NmeaParseError;
            fn try_from(mut value: Vec<u8>) -> Result<Self, Self::Error> {
                let msg_data = value.split_off(MESSAGE_DATA_OFFSET);
                let metadata = NmeaMessageMetadata::try_from(value)?;
                let data = NmeaMessageBody::from_bytes(metadata.pgn(), msg_data)?;
                Ok(Self { metadata, data })
            }
        }

        impl NmeaMessage {
            pub fn to_readings(self) -> Result<GenericReadingsResult, $crate::parse_helpers::errors::NmeaParseError> {
                Ok(std::collections::HashMap::from([
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
    };
}
