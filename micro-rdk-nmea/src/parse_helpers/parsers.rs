use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use micro_rdk::{common::sensor::GenericReadingsResult, google::protobuf::Value};

use super::{
    enums::NmeaEnumeratedField,
    errors::{NmeaParseError, NumberFieldError},
};

/// Cursor that consumes can consume bytes from a data vector by bit size.
pub struct DataCursor {
    data: Vec<u8>,
    // the amount of bits by which the previously read field overflowed into
    // the first byte
    bit_offset: usize,
}

impl DataCursor {
    pub fn new(data: Vec<u8>) -> Self {
        DataCursor {
            data,
            bit_offset: 0,
        }
    }

    pub fn read(&mut self, bit_size: usize) -> Result<Vec<u8>, NmeaParseError> {
        let bits_to_read = bit_size + self.bit_offset;
        if bits_to_read > self.data.len() * 8 {
            return Err(NmeaParseError::NotEnoughData);
        }
        let mut res = Vec::new();
        res.extend(self.data.drain(..(bits_to_read / 8)));

        let next_offset = bits_to_read % 8;
        // if our bit_size overflows to the middle of a byte by x bits, we want to select the last
        // x bits of the next byte without consuming it
        if next_offset != 0 {
            res.push(self.data[0] & (255 >> (8 - next_offset)));
        }

        // if the previous field overflowed into the first byte by x bits, we want
        // to shift that byte by x
        if self.bit_offset != 0 {
            if let Some(first_byte) = res.get_mut(0) {
                *first_byte >>= self.bit_offset;
            }
        }
        self.bit_offset = next_offset;
        Ok(res)
    }
}

/// Trait for reading a data type (`FieldType`) from a DataCursor.
pub trait FieldReader {
    type FieldType;
    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError>;
}

/// A field reader for parsing a basic number type. A reader with bit_size n will read its value
/// as the first n bits of `size_of::<T>()` bytes with the remaining bits as zeroes (the resulting bytes
/// will be parsed as Little-Endian). See invocation of the generate_integer_field_readers macro below
/// for the currently supported number types.
pub struct NumberField<T> {
    bit_size: usize,
    _marker: PhantomData<T>,
}

impl<T> NumberField<T> {
    pub fn new(bit_size: usize) -> Result<Self, NumberFieldError> {
        let max_bit_size = std::mem::size_of::<T>() * 8;
        if bit_size > max_bit_size {
            Err(NumberFieldError::ImproperBitSize(bit_size, max_bit_size))
        } else {
            Ok(Self {
                bit_size,
                _marker: Default::default(),
            })
        }
    }
}

pub struct NumberFieldWithScale<T> {
    number_field: NumberField<T>,
    scale: f64,
}

impl<T> NumberFieldWithScale<T> {
    pub fn new(bit_size: usize, scale: f64) -> Result<Self, NumberFieldError> {
        let number_field = NumberField::<T>::new(bit_size)?;
        Ok(Self {
            number_field,
            scale,
        })
    }
}

impl<T> Default for NumberField<T>
where
    T: Sized,
{
    fn default() -> Self {
        Self {
            bit_size: size_of::<T>() * 8,
            _marker: Default::default(),
        }
    }
}

macro_rules! generate_integer_field_readers {
    ($($t:ty),*) => {
        $(
            impl FieldReader for NumberField<$t> {
                type FieldType = $t;

                fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
                    let mut data = cursor.read(self.bit_size)?;
                    let max_size = std::mem::size_of::<Self::FieldType>();
                    if self.bit_size / 8 > max_size {
                        Err(NumberFieldError::ImproperBitSize(self.bit_size, max_size * 8).into())
                    } else {
                        data.resize(max_size, 0);
                        Ok(<$t>::from_le_bytes(data[..].try_into()?))
                    }
                }
            }

            impl FieldReader for NumberFieldWithScale<$t> {
                type FieldType = f64;
                fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
                    self.number_field.read_from_cursor(cursor).and_then(|x| {
                        match x {
                            x if x == <$t>::MAX => { return Err(NumberFieldError::FieldNotPresent("type unavailable".to_string()).into()); },
                            x => {
                                Ok((x as f64) * self.scale)
                            }
                        }
                    })
                }
            }
        )*
    };
}

generate_integer_field_readers!(u8, i8, u16, i16, u32, i32, u64, i64);

impl FieldReader for NumberField<f32> {
    type FieldType = f32;

    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        if self.bit_size != 32 {
            return Err(NumberFieldError::SizeNotAllowedforF32.into());
        }
        let data = cursor.read(self.bit_size)?;
        Ok(f32::from_le_bytes(data[..].try_into()?))
    }
}

pub struct BinaryCodedDecimalField {
    bit_size: usize,
}

impl BinaryCodedDecimalField {
    pub fn new(bit_size: usize) -> Self {
        Self { bit_size }
    }
}

impl FieldReader for BinaryCodedDecimalField {
    type FieldType = u128;

    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let data = cursor.read(self.bit_size)?;
        let mut value = u64::from_le_bytes(data[..].try_into()?) as u128;
        if value == 0 {
            return Ok(0);
        }

        let mut res: u128 = 0;
        let mut mult: u128 = 1;

        while value > 0 {
            let digit = value % 10;
            res += digit * mult;
            value /= 10;
            mult <<= 4;
        }

        Ok(res)
    }
}

pub struct FixedSizeStringField {
    bit_size: usize,
}

impl FixedSizeStringField {
    pub fn new(bit_size: usize) -> Self {
        Self { bit_size }
    }
}

fn trim_string_bytes(string_data: &mut Vec<u8>) {
    let last_index = string_data
        .iter()
        .position(|byte| {
            if (*byte == 0xFF) || (*byte == 0) {
                return true;
            }
            let char_at = *byte as char;
            (char_at == '@') || (char_at == ' ')
        })
        .unwrap_or(string_data.len());
    let _ = string_data.split_off(last_index);
}

impl FieldReader for FixedSizeStringField {
    type FieldType = String;
    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let mut string_data = cursor.read(self.bit_size)?;
        trim_string_bytes(&mut string_data);
        Ok(String::from_utf8(string_data)?)
    }
}

pub struct VariableLengthStringField;

impl FieldReader for VariableLengthStringField {
    type FieldType = String;
    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let length = cursor.read(8)?[0];
        let mut string_data = cursor.read((length * 8) as usize)?;
        trim_string_bytes(&mut string_data);
        Ok(String::from_utf8(string_data)?)
    }
}

pub struct VariableLengthAndEncodingStringField;

impl FieldReader for VariableLengthAndEncodingStringField {
    type FieldType = String;
    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let length = cursor.read(8)?[0];
        let encoding = cursor.read(8)?[0];
        let mut string_data = cursor.read((length * 8) as usize)?;
        trim_string_bytes(&mut string_data);
        Ok(match encoding {
            0 => {
                let utf16_vec: Vec<u16> = string_data
                    .chunks_exact(2)
                    .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                    .collect();
                String::from_utf16(utf16_vec.as_slice())?
            }
            1 => String::from_utf8(string_data)?,
            x => return Err(NmeaParseError::UnexpectedEncoding(x)),
        })
    }
}

/// A field reader for parsing data into a field type that implements the `Lookup` trait.
/// The `bit_size` property is used to parse a raw number value first with a methodology similar to the one
/// defined in the documentation for NumberField, after which the raw value is passed to `Lookup::from_value`
pub struct LookupField<T> {
    bit_size: usize,
    _marker: PhantomData<T>,
}

impl<T> LookupField<T> {
    pub fn new(bit_size: usize) -> Result<Self, NumberFieldError> {
        if bit_size > 32 {
            Err(NumberFieldError::ImproperBitSize(bit_size, 32))
        } else {
            Ok(Self {
                bit_size,
                _marker: Default::default(),
            })
        }
    }
}

impl<T> FieldReader for LookupField<T>
where
    T: NmeaEnumeratedField,
{
    type FieldType = T;

    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let number_parser = NumberField::<u32>::new(self.bit_size)?;
        let enum_value = number_parser.read_from_cursor(cursor)?;
        Ok(enum_value.into())
    }
}

/// A field reader for parsing an array [T; N] from a byte slice of data, where
/// T is a number type supported by `NumberField`.
pub struct ArrayField<T, const N: usize>(PhantomData<NumberField<T>>);

impl<T, const N: usize> ArrayField<T, N> {
    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<T, const N: usize> Default for ArrayField<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> FieldReader for ArrayField<T, N>
where
    NumberField<T>: FieldReader,
    <NumberField<T> as FieldReader>::FieldType: Default + Copy,
{
    type FieldType = [<NumberField<T> as FieldReader>::FieldType; N];

    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let mut res: [<NumberField<T> as FieldReader>::FieldType; N] = [Default::default(); N];
        let field_reader: NumberField<T> = Default::default();
        // TODO: Upon upgrade to Rust 1.84, change below to using try_from_fn
        for thing in res.iter_mut().take(N) {
            *thing = field_reader.read_from_cursor(cursor)?;
        }
        Ok(res)
    }
}

/// Some NMEA 2000 messages have a set of fields that may be repeated a number of times (usually specified by the value
/// of another field). This trait is for structs that implement this set of fields, most likely using the `FieldsetDerive` macro.
pub trait FieldSet: Sized {
    fn to_readings(&self) -> Result<GenericReadingsResult, NmeaParseError>;
    fn from_data(cursor: &mut DataCursor) -> Result<Self, NmeaParseError>;
}

/// A field reader that parses a vector of structs representing a field set (see `FieldSet`) from a byte slice.
/// The expected length of the vector must be provided on initialization.
pub struct FieldSetList<T> {
    length: usize,
    _marker: PhantomData<T>,
}

impl<T> FieldSetList<T> {
    pub fn new(length: usize) -> Self {
        Self {
            length,
            _marker: Default::default(),
        }
    }
}

impl<T> FieldReader for FieldSetList<T>
where
    T: FieldSet,
{
    type FieldType = Vec<T>;

    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let mut res = Vec::new();
        for _ in 0..self.length {
            let elem = T::from_data(cursor)?;
            res.push(elem);
        }
        Ok(res)
    }
}

pub trait PolymorphicDataType: Sized {
    type EnumType: NmeaEnumeratedField + Copy;
    fn from_data(
        cursor: &mut DataCursor,
        enum_type: Self::EnumType,
    ) -> Result<Self, NmeaParseError>;
    fn to_value(self) -> Value;
}

pub struct PolymorphicDataTypeReader<T: PolymorphicDataType> {
    _marker: PhantomData<T>,
    lookup_value: T::EnumType,
}

impl<T> PolymorphicDataTypeReader<T>
where
    T: PolymorphicDataType,
{
    pub fn new(lookup_value: T::EnumType) -> Self {
        Self {
            _marker: PhantomData,
            lookup_value,
        }
    }
}

impl<T> FieldReader for PolymorphicDataTypeReader<T>
where
    T: PolymorphicDataType,
{
    type FieldType = T;
    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        T::from_data(cursor, self.lookup_value)
    }
}

#[macro_export]
macro_rules! polymorphic_type {
    ($name:ident, $enumname:ident, $(($value:expr, $var:ident, $reader:expr, $field_type:ty)),*, $errorlabel:ident) => {

        #[derive(Debug, Clone)]
        pub enum $name {
            $($var($field_type)),*,
        }

        #[derive(Debug, Clone, Copy)]
        pub enum $enumname {
            $($var),*,
            $errorlabel
        }

        impl From<u32> for $enumname {
            fn from(value: u32) -> Self {
                match value {
                    $($value => Self::$var),*,
                    _ => Self::$errorlabel
                }
            }
        }

        impl std::fmt::Display for $enumname {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", match self {
                    $(Self::$var => stringify!($var)),*,
                    Self::$errorlabel => "could not parse"
                }.to_string())
            }
        }

        impl $crate::parse_helpers::enums::NmeaEnumeratedField for $enumname {}

        impl $crate::parse_helpers::parsers::PolymorphicDataType for $name {
            type EnumType = $enumname;

            fn from_data(cursor: &mut $crate::parse_helpers::parsers::DataCursor, enum_type: Self::EnumType) -> Result<Self, $crate::parse_helpers::errors::NmeaParseError> {
                match enum_type {
                    $(
                        $enumname::$var => {
                            Ok(Self::$var($reader.read_from_cursor(cursor)?))
                        }
                    ),*,
                    $enumname::$errorlabel => {
                        Err($crate::parse_helpers::errors::NmeaParseError::UnknownPolymorphicLookupValue)
                    }
                }
            }

            fn to_value(self) -> micro_rdk::google::protobuf::Value {
                micro_rdk::google::protobuf::Value { kind: None }
            }
        }
    };
}

// The first 32 bytes of the unparsed message stores metadata appearing in the header
// of an NMEA message (the library assumes that a previous process has correctly serialized
// this data from the CAN frame).
#[derive(Debug, Clone)]
pub struct NmeaMessageMetadata {
    pgn: u32,
    // timestamp: DateTime<Utc>,
    dst: u16,
    src: u16,
    priority: u16,
}

impl NmeaMessageMetadata {
    pub fn src(&self) -> u16 {
        self.src
    }

    pub fn dst(&self) -> u16 {
        self.dst
    }

    pub fn priority(&self) -> u16 {
        self.priority
    }

    // pub fn timestamp(&self) -> DateTime<Utc> {
    //     self.timestamp
    // }

    pub fn pgn(&self) -> u32 {
        self.pgn
    }
}

impl TryFrom<Vec<u8>> for NmeaMessageMetadata {
    type Error = NmeaParseError;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() < 32 {
            return Err(NmeaParseError::NotEnoughData);
        }
        let pgn = u32::from_le_bytes(value[0..4].try_into()?);
        let _seconds = u64::from_le_bytes(value[8..16].try_into()?) as i64;
        let _millis = u64::from_le_bytes(value[16..24].try_into()?);
        // let timestamp = DateTime::from_timestamp(seconds, (millis * 1000) as u32)
        //     .ok_or(NmeaParseError::MalformedTimestamp)?;

        let dst = u16::from_le_bytes(value[26..28].try_into()?);
        let src = u16::from_le_bytes(value[28..30].try_into()?);
        let priority = u16::from_le_bytes(value[30..32].try_into()?);
        Ok(Self {
            // timestamp,
            priority,
            src,
            dst,
            pgn,
        })
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::{
        define_nmea_enum,
        parse_helpers::{
            enums::NmeaEnumeratedField,
            errors::NmeaParseError,
            parsers::{DataCursor, FieldReader, NmeaMessageMetadata},
        },
    };

    use super::{
        ArrayField, FieldSet, FieldSetList, LookupField, NumberField, NumberFieldWithScale,
        PolymorphicDataTypeReader, VariableLengthStringField,
    };

    pub const MESSAGE_DATA_OFFSET: usize = 32;

    #[test]
    fn parse_metadata() {
        let data_str = "C/UBAHg+gD8l2A2A/////40fszsAAAAACAD/AAIAAwAAhgEAALwC/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(data_str, &mut data);
        assert!(res.is_ok());

        let _ = data.split_off(MESSAGE_DATA_OFFSET);
        let metadata = NmeaMessageMetadata::try_from(data);
        assert!(metadata.is_ok());
        let metadata = metadata.unwrap();

        assert_eq!(metadata.priority, 3);
        assert_eq!(metadata.dst, 255);
        assert_eq!(metadata.src, 2);
    }

    #[test]
    fn number_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 125, 73];
        let mut cursor = DataCursor::new(data_vec);

        let reader = NumberField::<u8>::new(4);
        assert!(reader.is_ok());
        let reader = reader.unwrap();
        assert!(cursor.read(16).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        // 125 = 01111101, last four bits as byte => 00001101 = 13
        assert_eq!(res.unwrap(), 13);

        let reader = NumberField::<u16>::new(12);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 125, 179, 152, 113];
        let mut cursor = DataCursor::new(data_vec);

        // [179, 152] is 39091 in u16, reading the first 12 bits (8 + last 4 of the second byte)
        // should yield [10110011 10011000] => [10110011 00001000] => 2227 (in Little-Endian)
        assert!(cursor.read(24).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 2227);

        let reader = NumberField::<u32>::new(3);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![154, 6, 125, 179, 152, 113];
        let mut cursor = DataCursor::new(data_vec);

        // 154 is 10011010, reading the last 3 bits should yield 010 = 2
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 2);
    }

    define_nmea_enum!(TestLookup, (0, A, "A"), (8, B, "B"), UnknownLookupField);

    #[test]
    fn lookup_field_test() {
        let reader = LookupField::<TestLookup>::new(4);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 111, 72, 152, 113];
        let mut cursor = DataCursor::new(data_vec);

        assert!(cursor.read(24).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert!(matches!(res.unwrap(), TestLookup::B));
    }

    #[test]
    fn array_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113, 42, 42];
        let mut cursor = DataCursor::new(data_vec.clone());

        let reader = ArrayField::<u8, 3>::new();
        assert!(cursor.read(16).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert_eq!(res.unwrap(), [111, 77, 152]);

        let mut cursor = DataCursor::new(data_vec.clone());
        let reader = ArrayField::<u16, 2>::new();
        // [6, 11] = 28422, [77, 152] = 38989
        assert!(cursor.read(8).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert_eq!(res.unwrap(), [28422, 38989]);

        let mut cursor = DataCursor::new(data_vec);
        let reader = ArrayField::<i32, 2>::new();
        // [100, 6, 111, 77] = 1299121764, [152, 113, 42, 42] = 707424664
        let res = reader.read_from_cursor(&mut cursor);
        assert_eq!(res.unwrap(), [1299121764, 707424664]);
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TestFieldSet {
        a: u16,
        b: u8,
        c: u16,
    }

    impl FieldSet for TestFieldSet {
        fn to_readings(
            &self,
        ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, NmeaParseError> {
            Err(NmeaParseError::UnsupportedPgn(0))
        }

        fn from_data(cursor: &mut super::DataCursor) -> Result<Self, NmeaParseError> {
            let u16_reader = NumberField::<u16>::default();
            let u8_reader = NumberField::<u8>::default();

            let a = u16_reader.read_from_cursor(cursor)?;
            let b = u8_reader.read_from_cursor(cursor)?;
            let c = u16_reader.read_from_cursor(cursor)?;

            Ok(TestFieldSet { a, b, c })
        }
    }

    #[test]
    fn fieldset_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113, 42, 42, 1, 2, 3];
        let mut cursor = DataCursor::new(data_vec);

        let reader = FieldSetList::<TestFieldSet>::new(2);
        assert!(cursor.read(8).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        let res = res.unwrap();

        let expected_at_0 = TestFieldSet {
            a: 28422, // [6, 11]
            b: 77,
            c: 29080, // [152, 113]
        };
        let expected_at_1 = TestFieldSet {
            a: 10794, // [42, 42]
            b: 1,
            c: 770, // [2, 3]
        };

        assert_eq!(res[0], expected_at_0);
        assert_eq!(res[1], expected_at_1);
    }

    define_nmea_enum!(
        TestSeedLookup,
        (0, A, "A"),
        (99, B, "B"),
        (11, C, "C"),
        UnknownLookupField
    );

    polymorphic_type!(
        TestSeedPolymorphism,
        TestKey,
        (
            0,
            NumberTypeA,
            NumberFieldWithScale::<i16>::new(16, 0.0001)?,
            f64
        ),
        (
            41,
            NumberTypeB,
            NumberFieldWithScale::<i16>::new(16, 60.0)?,
            f64
        ),
        (
            4863,
            LookupType,
            LookupField::<TestSeedLookup>::new(8)?,
            TestSeedLookup
        ),
        UnknownLookupField
    );

    #[test]
    fn polymorphic_field_test() {
        let data_vec: Vec<u8> = vec![32, 78, 99, 3, 0];
        let mut cursor = DataCursor::new(data_vec);

        let lookup_value = TestKey::NumberTypeA;
        let reader = PolymorphicDataTypeReader::<TestSeedPolymorphism>::new(lookup_value);
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert!(matches!(
            res.unwrap(),
            TestSeedPolymorphism::NumberTypeA(2.0)
        ));

        let lookup_value = TestKey::LookupType;
        let reader = PolymorphicDataTypeReader::<TestSeedPolymorphism>::new(lookup_value);
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert!(matches!(
            res.unwrap(),
            TestSeedPolymorphism::LookupType(TestSeedLookup::B)
        ));

        let lookup_value = TestKey::NumberTypeB;
        let reader = PolymorphicDataTypeReader::<TestSeedPolymorphism>::new(lookup_value);
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert!(matches!(
            res.unwrap(),
            TestSeedPolymorphism::NumberTypeB(180.0)
        ))
    }

    #[test]
    fn string_field_test() {
        let test_str_bytes = b"ffreghorsgeuilf@ @  ".to_vec();
        let result_str = "ffreghorsgeuilf".to_string();

        let mut bytes_to_read: Vec<u8> = vec![test_str_bytes.len() as u8];
        bytes_to_read.extend(test_str_bytes);
        bytes_to_read.extend(b"other garbage afterwards".to_vec());
        let mut cursor = DataCursor::new(bytes_to_read);

        let reader = VariableLengthStringField {};
        let res = reader.read_from_cursor(&mut cursor);

        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res, result_str);
    }
}
