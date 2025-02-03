use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use micro_rdk::common::sensor::GenericReadingsResult;

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
/// will be parsed as Little-Endian). See invocation of the generate_number_field_readers macro below
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

macro_rules! generate_number_field_readers {
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
        )*
    };
}

generate_number_field_readers!(u8, i8, u16, i16, u32, i32, u64, i64);

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

impl<T, const N: usize> FieldReader for ArrayField<T, N>
where
    NumberField<T>: FieldReader,
    <NumberField<T> as FieldReader>::FieldType: Default + Copy,
{
    type FieldType = [<NumberField<T> as FieldReader>::FieldType; N];

    fn read_from_cursor(&self, cursor: &mut DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let mut res: [<NumberField<T> as FieldReader>::FieldType; N] = [Default::default(); N];
        let field_reader: NumberField<T> = Default::default();
        for i in 0..N {
            let next_elem = field_reader.read_from_cursor(cursor)?;
            res[i] = next_elem;
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

// The first 32 bytes of the unparsed message stores metadata appearing in the header
// of an NMEA message (the library assumes that a previous process has correctly serialized
// this data from the CAN frame).
#[derive(Debug, Clone)]
pub struct NmeaMessageMetadata {
    pgn: u32,
    timestamp: DateTime<Utc>,
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

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp.clone()
    }

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
        let seconds = u64::from_le_bytes(value[8..16].try_into()?) as i64;
        let millis = u64::from_le_bytes(value[16..24].try_into()?);
        let timestamp = DateTime::from_timestamp(seconds, (millis * 1000) as u32)
            .ok_or(NmeaParseError::MalformedTimestamp)?;

        let dst = u16::from_le_bytes(value[26..28].try_into()?);
        let src = u16::from_le_bytes(value[28..30].try_into()?);
        let priority = u16::from_le_bytes(value[30..32].try_into()?);
        Ok(Self {
            timestamp,
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
        messages::pgns::MESSAGE_HEADER_OFFSET,
        parse_helpers::{
            enums::MagneticVariationSource,
            errors::NmeaParseError,
            parsers::{DataCursor, FieldReader, NmeaMessageMetadata},
        },
    };

    use super::{ArrayField, FieldSet, FieldSetList, LookupField, NumberField};

    #[test]
    fn parse_metadata() {
        let data_str = "C/UBAHg+gD8l2A2A/////40fszsAAAAACAD/AAIAAwAAhgEAALwC/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(data_str, &mut data);
        assert!(res.is_ok());

        let _ = data.split_off(MESSAGE_HEADER_OFFSET);
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

    #[test]
    fn lookup_field_test() {
        let reader = LookupField::<MagneticVariationSource>::new(4);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 111, 72, 152, 113];
        let mut cursor = DataCursor::new(data_vec);

        assert!(cursor.read(24).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert!(matches!(res.unwrap(), MagneticVariationSource::Wmm2020));
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
}
