use std::marker::PhantomData;

use micro_rdk::common::sensor::GenericReadingsResult;

use super::{
    enums::Lookup,
    errors::{NmeaParseError, NumberFieldError},
};

/// Trait for reading a data type (`FieldType`) from a byte slice.
pub trait FieldReader {
    type FieldType;

    /// Takes a byte slice (`data`) and starting index. Returns a tuple consisting of
    /// the next index to read from the byte sequence and the parsed value.
    fn read_from_data(
        &self,
        data: &[u8],
        start_idx: usize,
    ) -> Result<(usize, Self::FieldType), NmeaParseError>;
}

/// A field reader for parsing a basic number type. A reader with bit_size n will read its value
/// as the first n bits of `size_of::<T>()` bytes with the remaining bits as zeroes (the resulting bytes
/// will be parsed as Little-Endian). See invocation of the number_field macro below for the currently
/// supported number types.
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

macro_rules! number_field {
    ($($t:ty),*) => {
        $(
            impl FieldReader for NumberField<$t> {
                type FieldType = $t;

                fn read_from_data(&self, data: &[u8], start_idx: usize) -> Result<(usize, Self::FieldType), NmeaParseError> {
                    let type_size = std::mem::size_of::<$t>();
                    match self.bit_size {
                        x if x == (type_size * 8) => {
                            let value: &[u8] = &data[start_idx..(start_idx + type_size)];
                            Ok((start_idx + type_size, <$t>::from_le_bytes(value.try_into()?)))
                        }
                        x => {
                            let shift = (type_size * 8) - x;
                            let end_idx = start_idx + (x / 8);
                            let value: &[u8] = &data[start_idx..(end_idx + 1)];
                            let value = <$t>::from_le_bytes(value.try_into()?);
                            Ok((end_idx, value >> shift << shift))
                        }
                    }
                }
            }
        )*
    };
}

number_field!(u8, i8, u16, i16, u32, i32, u64, i64);

/// A field reader for parsing data into a field type that implements the `Lookup` trait.
/// The `bit_size` property is used to parse a raw number value first with a methodology similar to the one
/// defined in the documentation for NumberField, after which the raw value is passed to `Lookup::from_value`
pub struct LookupField<T> {
    bit_size: usize,
    _marker: PhantomData<T>,
}

impl<T> LookupField<T> {
    pub fn new(bit_size: usize) -> Self {
        Self {
            bit_size,
            _marker: Default::default(),
        }
    }
}

impl<T> FieldReader for LookupField<T>
where
    T: Lookup,
{
    type FieldType = T;

    fn read_from_data(
        &self,
        data: &[u8],
        start_idx: usize,
    ) -> Result<(usize, Self::FieldType), NmeaParseError> {
        let end_idx = start_idx + (self.bit_size / 8);
        let data_slice: &[u8] = &data[start_idx..(end_idx + 1)];
        let enum_value = match self.bit_size {
            8 => data[start_idx] as u32,
            16 => u16::from_le_bytes(data_slice.try_into()?) as u32,
            32 => u32::from_le_bytes(data_slice.try_into()?),
            x if x < 8 => {
                let shift = 8 - x;
                (data[start_idx] >> shift) as u32
            }
            x if x < 16 => {
                let shift = 16 - x;
                let raw_val = u16::from_le_bytes(data_slice.try_into()?);
                (raw_val >> shift) as u32
            }
            x if x < 32 => {
                let shift = 32 - x;
                let raw_val = u32::from_le_bytes(data_slice.try_into()?);
                raw_val >> shift
            }
            _ => unreachable!("lookup field raw value cannot be more than 32 bits"),
        };
        Ok((end_idx, T::from_value(enum_value)))
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

    fn read_from_data(
        &self,
        data: &[u8],
        start_idx: usize,
    ) -> Result<(usize, Self::FieldType), NmeaParseError> {
        let mut res: [<NumberField<T> as FieldReader>::FieldType; N] = [Default::default(); N];
        let mut end_idx = start_idx;
        let field_reader: NumberField<T> = Default::default();
        for i in 0..N {
            let (bytes_read, next_elem) = field_reader.read_from_data(data, end_idx)?;
            res[i] = next_elem;
            end_idx = bytes_read
        }
        Ok((end_idx, res))
    }
}

/// Some NMEA 2000 messages have a set of fields that may be repeated a number of times (usually specified by the value
/// of another field). This trait is for structs that implement this set of fields, most likely using the `FieldsetDerive` macro.
pub trait FieldSet: Sized {
    fn from_bytes(data: &[u8], current_index: usize) -> Result<(usize, Self), NmeaParseError>;
    fn to_readings(&self) -> Result<GenericReadingsResult, NmeaParseError>;
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

    fn read_from_data(
        &self,
        data: &[u8],
        start_idx: usize,
    ) -> Result<(usize, Self::FieldType), NmeaParseError> {
        let mut idx = start_idx;
        let mut res = Vec::new();
        for _ in 0..self.length {
            let (new_start, new_elem) = T::from_bytes(data, idx)?;
            idx = new_start;
            res.push(new_elem);
        }
        Ok((idx, res))
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_helpers::{
        enums::MagneticVariationSource, errors::NmeaParseError, parsers::FieldReader,
    };

    use super::{ArrayField, FieldSet, FieldSetList, LookupField, NumberField};

    #[test]
    fn number_field_test() {
        let single_byte_reader = NumberField::<u8>::new(4);
        assert!(single_byte_reader.is_ok());
        let single_byte_reader = single_byte_reader.unwrap();

        // 125 = 01111101, first four bits as byte => 01110000 = 112
        let data_vec: Vec<u8> = vec![100, 6, 125, 73];
        let data: &[u8] = &data_vec[..];
        let res = single_byte_reader.read_from_data(data, 2);
        assert!(res.is_ok());
        let (next_index, res) = res.unwrap();
        assert_eq!(next_index, 2);
        assert_eq!(res, 112);

        let reader = NumberField::<u16>::new(12);
        assert!(reader.is_ok());
        let reader = reader.unwrap();
        let data_vec: Vec<u8> = vec![100, 6, 125, 179, 152, 113];
        let data: &[u8] = &data_vec[..];

        // [179, 152] is 39091 in u16, reading the first 12 bits and ignoring the last 4
        // should yield 39088
        let res = reader.read_from_data(data, 3);
        assert!(res.is_ok());
        let (next_index, res) = res.unwrap();
        assert_eq!(next_index, 4);
        assert_eq!(res, 39088);
    }

    #[test]
    fn lookup_field_test() {
        let reader = LookupField::<MagneticVariationSource>::new(4);

        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113];
        let data: &[u8] = &data_vec[..];

        let res = reader.read_from_data(data, 3);
        let (next_index, res) = res.unwrap();
        assert_eq!(next_index, 3);
        assert!(matches!(res, MagneticVariationSource::Wmm2000));
    }

    #[test]
    fn array_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113, 42, 42];
        let data: &[u8] = &data_vec[..];

        let reader = ArrayField::<u8, 3>::new();

        let res = reader.read_from_data(data, 2);
        assert!(res.is_ok());
        let (next_index, res) = res.unwrap();

        assert_eq!(next_index, 5);
        assert_eq!(res, [111, 77, 152]);

        let reader = ArrayField::<u16, 2>::new();

        let res = reader.read_from_data(data, 1);
        assert!(res.is_ok());
        let (next_index, res) = res.unwrap();

        // [6, 11] = 28422, [77, 152] = 38989
        assert_eq!(next_index, 5);
        assert_eq!(res, [28422, 38989]);

        let reader = ArrayField::<i32, 2>::new();

        let res = reader.read_from_data(data, 0);
        assert!(res.is_ok());
        let (next_index, res) = res.unwrap();

        // [100, 6, 111, 77] = 1299121764, [152, 113, 42, 42] = 707424664
        assert_eq!(next_index, 8);
        assert_eq!(res, [1299121764, 707424664]);
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
    }

    #[test]
    fn fieldset_field_test() {
        let data_vec: Vec<u8> = vec![100, 6, 111, 77, 152, 113, 42, 42, 1, 2, 3];
        let data: &[u8] = &data_vec[..];

        let reader = FieldSetList::<TestFieldSet>::new(2);
        let res = reader.read_from_data(data, 1);
        assert!(res.is_ok());
        let (next_index, res) = res.unwrap();

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

        assert_eq!(next_index, 11);
        assert_eq!(res[0], expected_at_0);
        assert_eq!(res[1], expected_at_1);
    }
}
