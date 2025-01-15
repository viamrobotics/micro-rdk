use bitvec::prelude::*;
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use micro_rdk::common::sensor::GenericReadingsResult;

use super::{
    enums::NmeaEnumeratedField,
    errors::{NmeaParseError, NumberFieldError},
};

/// Cursor over a byte slice of data. The cursor can be moved by either consuming
/// child-instances of `DataRead` or calling the `advance` function
pub struct DataCursor<'a> {
    data: &'a [u8],
    bit_position: Rc<AtomicUsize>,
}

struct DataRead<'a> {
    data: &'a [u8],
    bit_position: Rc<AtomicUsize>,
    bit_size: usize,
}

impl<'a> Drop for DataRead<'a> {
    fn drop(&mut self) {
        self.bit_position.fetch_add(self.bit_size, Ordering::SeqCst);
    }
}

impl<'a> DataCursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_position: Rc::new(AtomicUsize::new(0)),
        }
    }

    fn read(&self, bits: usize) -> Result<DataRead, NmeaParseError> {
        let bit_position = self.bit_position.load(Ordering::SeqCst);
        let end_byte_position = (bit_position + bits).div_ceil(8);
        if end_byte_position > data.len() {
            Err(NmeaParseError::EndOfBufferExceeded)
        } else {
            let start_byte_position = bit_position / 8;
            Ok(DataRead {
                data: &self.data[start_byte_position..end_byte_position],
                bit_position: self.bit_position.clone(),
                bit_size: bits,
            })
        }
    }

    pub fn advance(&self, bits: usize) -> Result<(), NmeaParseError> {
        let advanced_position = self.bit_position.fetch_add(bits, Ordering::SeqCst);
        if advanced_position > (self.data.len() * 8) {
            self.bit_position.fetch_sub(bits, Ordering::SeqCst);
            Err(NmeaParseError::EndOfBufferExceeded)
        } else {
            Ok(())
        }
    }
}

/// Trait for reading a data type (`FieldType`) from a DataCursor.
pub trait FieldReader {
    type FieldType;
    fn read_from_cursor(&self, cursor: &DataCursor) -> Result<Self::FieldType, NmeaParseError>;
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
            impl TryFrom<DataRead<'_>> for $t {
                type Error = NumberFieldError;
                fn try_from(value: DataRead) -> Result<Self, Self::Error> {
                    let max_size = std::mem::size_of::<Self>();
                    if (value.bit_size / 8) > max_size {
                        return Err(NumberFieldError::ImproperBitSize(value.bit_size, max_size * 8));
                    }

                    let bits = &value.data[..].view_bits::<Msb0>();

                    let start_idx = value.bit_position.load(Ordering::SeqCst) % 8;
                    let end_idx = start_idx + value.bit_size;
                    let mut bit_vec = bits[start_idx..end_idx].to_bitvec();
                    if bit_vec.len() != (max_size * 8) {
                        let last_bit_start = bit_vec.len() - (value.bit_size % 8);
                        let _ = &bit_vec[last_bit_start..].reverse();

                        // pad the bit vector with 0 bits until we have enough bytes to
                        // parse the number
                        for _ in (0..(max_size * 8 - value.bit_size)) {
                            bit_vec.push(false);
                        }


                        let last_bit_start = bit_vec.len() - 8;
                        let _ = &bit_vec[last_bit_start..].reverse();
                    }

                    Ok(bit_vec.load_le::<$t>())
                }
            }

            impl FieldReader for NumberField<$t> {
                type FieldType = $t;

                fn read_from_cursor(&self, cursor: &DataCursor) -> Result<Self::FieldType, NmeaParseError> {
                    Ok(cursor.read(self.bit_size)?.try_into()?)
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

    fn read_from_cursor(&self, cursor: &DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let data_read = cursor.read(self.bit_size)?;
        let enum_value = match self.bit_size {
            x if x <= 8 => Ok(u8::try_from(data_read)? as u32),
            x if x <= 16 => Ok(u16::try_from(data_read)? as u32),
            x if x <= 32 => Ok(u16::try_from(data_read)? as u32),
            _ => unreachable!("malformed lookup field detected"),
        }?;
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

    fn read_from_cursor(&self, cursor: &DataCursor) -> Result<Self::FieldType, NmeaParseError> {
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
    fn from_bytes(data: &[u8], current_index: usize) -> Result<(usize, Self), NmeaParseError>;
    fn to_readings(&self) -> Result<GenericReadingsResult, NmeaParseError>;
    fn from_data(cursor: &DataCursor) -> Result<Self, NmeaParseError>;
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

    fn read_from_cursor(&self, cursor: &DataCursor) -> Result<Self::FieldType, NmeaParseError> {
        let mut res = Vec::new();
        for _ in 0..self.length {
            let elem = T::from_data(&cursor)?;
            res.push(elem);
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_helpers::{
        enums::MagneticVariationSource,
        errors::NmeaParseError,
        parsers::{DataCursor, FieldReader},
    };

    use super::{ArrayField, FieldSet, FieldSetList, LookupField, NumberField};

    #[test]
    fn number_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 125, 73];
        let data: &[u8] = &data_vec[..];
        let cursor = DataCursor::new(data);

        let reader = NumberField::<u8>::new(4);
        assert!(reader.is_ok());
        let reader = reader.unwrap();
        assert!(cursor.advance(16).is_ok());
        let res = reader.read_from_cursor(&cursor);
        assert!(res.is_ok());
        // 125 = 01111101, first four bits as byte => 01110000 = 112
        assert_eq!(res.unwrap(), 7);

        let reader = NumberField::<u16>::new(12);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 125, 179, 152, 113];
        let data: &[u8] = &data_vec[..];
        let cursor = DataCursor::new(data);

        // [179, 152] is 39091 in u16, reading the first 12 bits and ignoring the last 4
        // should yield [10110011 10011000] => [10110011 00001001] =>  (37043 in Little-Endian)
        assert!(cursor.advance(24).is_ok());
        let res = reader.read_from_cursor(&cursor);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 2483);
    }

    #[test]
    fn lookup_field_test() {
        let reader = LookupField::<MagneticVariationSource>::new(4);
        assert!(reader.is_ok());
        let reader = reader.unwrap();

        let data_vec: Vec<u8> = vec![100, 6, 111, 138, 152, 113];
        let data: &[u8] = &data_vec[..];
        let cursor = DataCursor::new(data);

        assert!(cursor.advance(24).is_ok());
        let res = reader.read_from_cursor(&cursor);
        assert!(res.is_ok());
        assert!(matches!(res.unwrap(), MagneticVariationSource::Wmm2020));
    }

    #[test]
    fn array_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113, 42, 42];
        let data: &[u8] = &data_vec[..];
        let cursor = DataCursor::new(data);

        let reader = ArrayField::<u8, 3>::new();
        assert!(cursor.advance(16).is_ok());
        let res = reader.read_from_cursor(&cursor);
        assert_eq!(res.unwrap(), [111, 77, 152]);

        let cursor = DataCursor::new(data);
        let reader = ArrayField::<u16, 2>::new();
        // [6, 11] = 28422, [77, 152] = 38989
        assert!(cursor.advance(8).is_ok());
        let res = reader.read_from_cursor(&cursor);
        assert_eq!(res.unwrap(), [28422, 38989]);

        let cursor = DataCursor::new(data);
        let reader = ArrayField::<i32, 2>::new();
        // [100, 6, 111, 77] = 1299121764, [152, 113, 42, 42] = 707424664
        let res = reader.read_from_cursor(&cursor);
        assert_eq!(res.unwrap(), [1299121764, 707424664]);
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TestFieldSet {
        a: u16,
        b: u8,
        c: u16,
    }

    impl FieldSet for TestFieldSet {
        fn from_bytes(data: &[u8], current_index: usize) -> Result<(usize, Self), NmeaParseError> {
            let a_data: &[u8] = &data[current_index..(current_index + 2)];
            let a = u16::from_le_bytes(a_data.try_into()?);
            let b = data[current_index + 2];
            let c_data: &[u8] = &data[(current_index + 3)..(current_index + 5)];
            let c = u16::from_le_bytes(c_data.try_into()?);

            Ok((current_index + 5, TestFieldSet { a, b, c }))
        }

        fn to_readings(
            &self,
        ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, NmeaParseError> {
            Err(NmeaParseError::UnsupportedPgn(0))
        }

        fn from_data(cursor: &super::DataCursor) -> Result<Self, NmeaParseError> {
            let a = cursor.read(16)?.try_into()?;
            let b = cursor.read(8)?.try_into()?;
            let c = cursor.read(16)?.try_into()?;
            Ok(TestFieldSet { a, b, c })
        }
    }

    #[test]
    fn fieldset_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113, 42, 42, 1, 2, 3];
        let data: &[u8] = &data_vec[..];
        let cursor = DataCursor::new(data);

        let reader = FieldSetList::<TestFieldSet>::new(2);
        assert!(cursor.advance(8).is_ok());
        let res = reader.read_from_cursor(&cursor);
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
