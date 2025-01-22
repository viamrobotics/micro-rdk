use std::{array::TryFromSliceError, marker::PhantomData};

use micro_rdk::{common::sensor::GenericReadingsResult, google::protobuf::Timestamp};

use super::{
    enums::NmeaEnumeratedField,
    errors::{NmeaParseError, NumberFieldError},
};

/// Cursor that consumes can consume bytes from a data vector by bit size.
pub struct DataCursor {
    data: Vec<u8>,
}

impl DataCursor {
    pub fn new(data: Vec<u8>) -> Self {
        DataCursor { data }
    }

    pub fn read(&mut self, bit_size: usize) -> Result<Vec<u8>, NmeaParseError> {
        if bit_size / 8 > self.data.len() {
            return Err(NmeaParseError::NotEnoughData);
        }
        let mut res = Vec::new();
        res.extend(self.data.drain(..(bit_size / 8)));

        // If the next bit_size takes us into the middle of a byte, then
        // we want the next byte of the result to be shifted so as to ignore the remaining bits.
        // The first byte of the data should have the bits that were included in the result
        // set to zero.
        //
        // This is due to the way number fields in NMEA messages appear to be formatted
        // from observation of successfully parsed data, which is that overflowing bits
        // are read in MsB order, but the resulting bytes are read in Little-Endian.
        //
        // For example, let's say the data is [179, 152], and we want to read a u16 from a bit size of 12
        //
        // [179, 152] is 39091 in u16, reading the first 12 bits and ignoring the last 4
        // should yield [10110011 10011000] => [10110011 00001001] => 2483 (in Little-Endian)
        if bit_size % 8 != 0 {
            res.push(self.data[0] >> (8 - (bit_size % 8)));
            let mask: u8 = 255 >> (bit_size % 8);
            if let Some(first_byte) = self.data.get_mut(0) {
                *first_byte |= mask;
            }
        }
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
// this data from the CAN frame). For an explanation of the fields in this header, see
// the section labeled "ISO-11783 and NMEA2000 header" at https://canboat.github.io/canboat/canboat.html
pub struct NmeaMessageMetadata {
    timestamp: Timestamp,
    priority: u16,
    src: u16,
    dst: u16,
}

impl NmeaMessageMetadata {
    pub fn from_bytes(data: [u8; 32]) -> Result<Self, TryFromSliceError> {
        let seconds = u64::from_le_bytes(data[8..16].try_into()?) as i64;
        let millis = u64::from_le_bytes(data[16..24].try_into()?);
        let timestamp = Timestamp {
            seconds,
            nanos: (millis * 1000) as i32,
        };

        let dst = u16::from_le_bytes(data[26..28].try_into()?);
        let src = u16::from_le_bytes(data[28..30].try_into()?);
        let priority = u16::from_le_bytes(data[30..32].try_into()?);
        Ok(Self {
            timestamp,
            priority,
            src,
            dst,
        })
    }

    pub fn src(&self) -> u16 {
        self.src
    }

    pub fn dst(&self) -> u16 {
        self.dst
    }

    pub fn priority(&self) -> u16 {
        self.priority
    }

    pub fn timestamp(&self) -> Timestamp {
        self.timestamp.clone()
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::parse_helpers::{
        enums::MagneticVariationSource,
        errors::NmeaParseError,
        parsers::{DataCursor, FieldReader, NmeaMessageMetadata},
    };

    use super::{ArrayField, FieldSet, FieldSetList, LookupField, NumberField};

    #[test]
    fn parse_metadata() {
        let data_str = "C/UBAHg+gD8l2A2A/////40fszsAAAAACAD/AAIAAwAAhgEAALwC/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(data_str, &mut data);
        assert!(res.is_ok());

        let metadata = NmeaMessageMetadata::from_bytes(data[0..32].try_into().unwrap());
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
        // 125 = 01111101, first four bits as byte => 00000111 = 7
        assert_eq!(res.unwrap(), 7);

        let reader = NumberField::<u16>::new(12);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 125, 179, 152, 113];
        let mut cursor = DataCursor::new(data_vec);

        // [179, 152] is 39091 in u16, reading the first 12 bits and ignoring the last 4
        // should yield [10110011 10011000] => [10110011 00001001] => 2483 ( in Little-Endian)
        assert!(cursor.read(24).is_ok());
        let res = reader.read_from_cursor(&mut cursor);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 2483);

        let reader = NumberField::<u32>::new(3);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![154, 6, 125, 179, 152, 113];
        let mut cursor = DataCursor::new(data_vec);

        // 154 is 10011010, reading the first 3 bits should yield 100 = 4
        let res = reader.read_from_cursor(&mut cursor);
        println!("res: {:?}", res);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 4);
    }

    #[test]
    fn lookup_field_test() {
        let reader = LookupField::<MagneticVariationSource>::new(4);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 111, 138, 152, 113];
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
