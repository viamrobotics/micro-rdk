pub trait NmeaEnumeratedField: Sized + From<u32> + ToString {}

/// For generating a lookup data type found in an NMEA message. The first argument is the name of the
/// enum type that will be generated. Each successive argument is a tuple with
/// (raw number value, name of enum instance, string representation)
///
/// Note: we implement From<u32> rather than TryFrom<u32> because our equivalent library
/// written in Go does not fail on unrecognized lookups.
#[macro_export]
macro_rules! define_nmea_enum {
    ( $name:ident, $(($value:expr, $var:ident, $label:expr)),*, $default:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum $name {
            $($var),*,
            $default
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                match value {
                    $($value => Self::$var),*,
                    _ => Self::$default
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", match self {
                    $(Self::$var => $label),*,
                    Self::$default => "could not parse"
                }.to_string())
            }
        }

        impl NmeaEnumeratedField for $name {}
    };

}
