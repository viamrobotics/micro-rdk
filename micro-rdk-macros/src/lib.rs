//! Collection of macros useful for implementing traits for various components. NOTE: THESE
//! MACROS WILL NOT WORK PROPERLY IF YOU RENAME THE MICRO-RDK DEPENDENCY USING THE PACKAGE
//! ATTRIBUTE IN CARGO.TOML.
//!
//! DoCommand - trivially implements the DoCommand trait (most component traits require this trait to be
//! satisfied to allow for driver-specific commands that fall outside of the component's API, but most
//! implementations have no real need for it)
//!
//! MovementSensorReadings - provides a default implementation of the Readings trait for implementers
//! of the MovementSensor trait. `get_generic_readings` will return a struct of key-value pairs for every
//! method that is declared supported by `get_properties`
//!
//! PowerSensorReadings - provides a default implementation of the Readings trait for implementers
//! of the PowerSensor trait. `get_generic_readings` will return a struct containing the voltage (in volts),
//! current (in amperes), power (in watts), and whether or not the power supply is AC.
//!
//! # Example using `MovementSensorReadings`
//!
//! ```ignore
//! use std::collections::HashMap;
//! use micro_rdk::common::{
//!     movement_sensor::{MovementSensor, MovementSensorSupportedMethods},
//!     status::Status,
//! };
//! use micro_rdk::{DoCommand, MovementSensorReadings};
//!
//! #[derive(DoCommand, MovementSensorReadings)]
//! pub struct MyMovementSensor {}
//!
//! impl MovementSensor for MyMovementSensor {
//!     fn get_angular_velocity(&mut self) -> anyhow::Result<micro_rdk::common::math_utils::Vector3> {
//!         anyhow::bail!("unimplemented")
//!     }
//!     fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
//!         Ok(25.0)
//!     }
//!     fn get_linear_acceleration(
//!         &mut self,
//!     ) -> anyhow::Result<micro_rdk::common::math_utils::Vector3> {
//!         anyhow::bail!("unimplemented")
//!     }
//!     fn get_linear_velocity(&mut self) -> anyhow::Result<micro_rdk::common::math_utils::Vector3> {
//!         anyhow::bail!("unimplemented")
//!     }
//!     fn get_position(&mut self) -> anyhow::Result<micro_rdk::common::movement_sensor::GeoPosition> {
//!         anyhow::bail!("unimplemented")
//!     }
//!     fn get_properties(&self) -> MovementSensorSupportedMethods {
//!         MovementSensorSupportedMethods {
//!             position_supported: false,
//!             linear_velocity_supported: false,
//!             angular_velocity_supported: false,
//!             linear_acceleration_supported: false,
//!             compass_heading_supported: false,
//!         }
//!     }
//! }
//!
//! impl Status for MyMovementSensor {
//!     fn get_status(&self) -> anyhow::Result<Option<micro_rdk::google::protobuf::Struct>> {
//!         Ok(Some(micro_rdk::google::protobuf::Struct {
//!             fields: HashMap::new(),
//!         }))
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::Ident;

fn get_micro_rdk_crate_ident() -> Ident {
    let found_crate = crate_name("micro-rdk").expect("micro-rdk is present in `Cargo.toml`");
    match found_crate {
        FoundCrate::Itself => Ident::new("crate", Span::call_site()),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

#[proc_macro_derive(DoCommand)]
pub fn impl_do_command_default(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let crate_ident = get_micro_rdk_crate_ident();
    let gen = quote! {
        impl #impl_generics #crate_ident::common::generic::DoCommand for #name #ty_generics #where_clause {}
    };
    gen.into()
}

#[proc_macro_derive(MovementSensorReadings)]
pub fn impl_readings_for_movement_sensor(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let crate_ident = get_micro_rdk_crate_ident();
    let gen = quote! {
        impl #impl_generics #crate_ident::common::sensor::Readings for #name #ty_generics #where_clause {
            fn get_generic_readings(&mut self) -> Result<#crate_ident::common::sensor::GenericReadingsResult,#crate_ident::common::sensor::SensorError> {
                #crate_ident::common::movement_sensor::get_movement_sensor_generic_readings(self)
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(PowerSensorReadings)]
pub fn impl_readings_for_power_sensor(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let crate_ident = get_micro_rdk_crate_ident();
    let gen = quote! {
        impl #impl_generics #crate_ident::common::sensor::Readings for #name #ty_generics #where_clause {
            fn get_generic_readings(&mut self) -> Result<#crate_ident::common::sensor::GenericReadingsResult,#crate_ident::common::sensor::SensorError> {
                #crate_ident::common::power_sensor::get_power_sensor_generic_readings(self)
            }
        }
    };

    gen.into()
}
