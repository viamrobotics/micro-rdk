use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Ident, Type};

// Our Golang variant of this library currently automatically performs standard conversions
// for particular units. `UnitConversion` auto-generates this logic for a given supported result unit.
#[derive(Debug)]
pub(crate) enum UnitConversion {
    KelvinToCelsius,
    CoulombToAmpereHour,
    PascalToBar,
    RadianToDegree,
    RadPerSecToDegPerSec,
    MetersPerSecToKnots,
    NoConversionNecessary,
}

impl From<&str> for UnitConversion {
    fn from(value: &str) -> Self {
        match value {
            "Coulomb" => Self::CoulombToAmpereHour,
            "Pa" => Self::PascalToBar,
            "K" => Self::KelvinToCelsius,
            "rad" => Self::RadianToDegree,
            "rad/s" => Self::RadPerSecToDegPerSec,
            "m/s" => Self::MetersPerSecToKnots,
            _ => Self::NoConversionNecessary,
        }
    }
}

impl UnitConversion {
    pub(crate) fn tokens(&self) -> TokenStream2 {
        match self {
            Self::KelvinToCelsius => quote! {
                let result = (result as f64) - 273.15;
            },
            Self::CoulombToAmpereHour => quote! {
                let result = (result as f64) / 3600.0;
            },
            Self::PascalToBar => quote! {
                let result = (result as f64) / 100000.0;
            },
            Self::RadianToDegree | Self::RadPerSecToDegPerSec => quote! {
                let result = (result as f64) * (180.0 / std::f64::consts::PI);
            },
            Self::MetersPerSecToKnots => quote! {
                let result = (result as f64) * 1.94384;
            },
            Self::NoConversionNecessary => quote! {},
        }
    }
}

pub(crate) fn error_tokens(msg: &str) -> TokenStream {
    syn::Error::new(Span::call_site(), msg)
        .to_compile_error()
        .into()
}

pub(crate) fn get_micro_nmea_crate_ident() -> Ident {
    let found_crate =
        crate_name("micro-rdk-nmea").expect("micro-rdk-nmea is present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

pub(crate) fn get_micro_rdk_crate_ident() -> Ident {
    let found_crate = crate_name("micro-rdk").expect("micro-rdk is present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

pub(crate) fn get_proto_import_prefix() -> TokenStream2 {
    let crate_ident = get_micro_rdk_crate_ident();
    quote! {#crate_ident::google::protobuf}
}

pub(crate) fn is_supported_numeric_type(field_type: &Type) -> bool {
    match field_type {
        Type::Path(type_path) => {
            type_path.path.is_ident("u32")
                || type_path.path.is_ident("u16")
                || type_path.path.is_ident("u8")
                || type_path.path.is_ident("i32")
                || type_path.path.is_ident("i16")
                || type_path.path.is_ident("i64")
                || type_path.path.is_ident("u64")
                || type_path.path.is_ident("i8")
                || type_path.path.is_ident("u128")
                || type_path.path.is_ident("f32")
        }
        _ => false,
    }
}

pub(crate) fn is_string_type(field_type: &Type) -> bool {
    match field_type {
        Type::Path(type_path) => type_path.path.is_ident("String"),
        _ => false,
    }
}

pub(crate) fn is_supported_array_type(field_type: &Type) -> bool {
    match field_type {
        Type::Array(array_ty) => match &(*array_ty.elem) {
            Type::Path(type_path) => type_path.path.is_ident("u8"),
            _ => false,
        },
        _ => false,
    }
}
