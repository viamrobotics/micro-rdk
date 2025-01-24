use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{DeriveInput, Field, Ident, Type};

use crate::attributes::MacroAttributes;
use crate::utils::{error_tokens, get_micro_nmea_crate_ident, is_supported_integer_type};

/// Represents a subset of auto-generated code statements for the implementation of a particular
/// NMEA message. Each field in a message struct contributes its own set of statements to the macro
/// sorted into buckets by category. Those statements are then merged into the set of statements
/// compiled by the previous field until the code for the message is complete, at which point the
/// composition can be turned into a TokenStream that can be returned by the macro function
pub(crate) struct PgnComposition {
    pub(crate) attribute_getters: Vec<TokenStream2>,
    pub(crate) parsing_logic: Vec<TokenStream2>,
    pub(crate) struct_initialization: Vec<TokenStream2>,
    pub(crate) proto_conversion_logic: Vec<TokenStream2>,
}

impl PgnComposition {
    pub(crate) fn new() -> Self {
        Self {
            attribute_getters: vec![],
            parsing_logic: vec![],
            struct_initialization: vec![],
            proto_conversion_logic: vec![],
        }
    }

    pub(crate) fn merge(&mut self, mut other: Self) {
        self.attribute_getters.append(&mut other.attribute_getters);
        self.parsing_logic.append(&mut other.parsing_logic);
        self.struct_initialization
            .append(&mut other.struct_initialization);
        self.proto_conversion_logic
            .append(&mut other.proto_conversion_logic);
    }

    pub(crate) fn from_field(field: &Field) -> Result<Self, TokenStream> {
        let mut statements = Self::new();
        if let Some(name) = &field.ident {
            if name == "source_id" {
                let num_ty = &field.ty;
                statements.attribute_getters.push(quote! {
                    pub fn #name(&self) -> #num_ty { self.#name }
                });
                statements.struct_initialization.push(quote! { source_id, });
                return Ok(statements);
            }

            let macro_attrs = MacroAttributes::from_field(field)?;
            if macro_attrs.offset != 0 {
                let offset = macro_attrs.offset / 8;
                statements
                    .parsing_logic
                    .push(quote! { let _ = cursor.read(#offset)?; });
            }

            let new_statements = if is_supported_integer_type(&field.ty) {
                handle_number_field(name, field, &macro_attrs)?
            } else if macro_attrs.is_lookup {
                handle_lookup_field(name, &field.ty, &macro_attrs)?
            } else {
                let err_msg = format!(
                    "field type for {:?} unsupported for PGN message, macro attributes: {:?}",
                    name.to_string(),
                    macro_attrs
                );
                return Err(error_tokens(&err_msg));
            };

            statements.merge(new_statements);
            Ok(statements)
        } else {
            Err(error_tokens(
                "could not parse parsing/getter statements for field",
            ))
        }
    }

    pub(crate) fn from_input(input: &DeriveInput) -> Result<Self, TokenStream> {
        let src_fields = if let syn::Data::Struct(syn::DataStruct { ref fields, .. }) = input.data {
            fields
        } else {
            return Err(
                syn::Error::new(Span::call_site(), "PgnMessageDerive expected struct")
                    .to_compile_error()
                    .into(),
            );
        };

        let named_fields = if let syn::Fields::Named(f) = src_fields {
            &f.named
        } else {
            return Err(crate::utils::error_tokens(
                "PgnMessageDerive expected struct with named fields",
            ));
        };
        let mut statements = Self::new();
        for field in named_fields.iter() {
            match PgnComposition::from_field(field) {
                Ok(new_statements) => {
                    statements.merge(new_statements);
                }
                Err(err) => {
                    return Err(err);
                }
            };
        }
        Ok(statements)
    }

    pub(crate) fn into_token_stream(self, input: &DeriveInput) -> TokenStream2 {
        let name = &input.ident;
        let parsing_logic = self.parsing_logic;
        let attribute_getters = self.attribute_getters;
        let struct_initialization = self.struct_initialization;
        let proto_conversion_logic = self.proto_conversion_logic;
        let (impl_generics, src_generics, src_where_clause) = input.generics.split_for_impl();
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        let mrdk_crate = crate::utils::get_micro_rdk_crate_ident();
        quote! {
            impl #impl_generics #name #src_generics #src_where_clause {
                pub fn from_bytes(data: Vec<u8>, source_id: u8) -> Result<Self, #error_ident> {
                    use #crate_ident::parse_helpers::parsers::{DataCursor, FieldReader};
                    let mut cursor = DataCursor::new(data);
                    #(#parsing_logic)*
                    Ok(Self {
                        #(#struct_initialization)*
                    })
                }
                #(#attribute_getters)*

                pub fn to_readings(self) -> Result<#mrdk_crate::common::sensor::GenericReadingsResult, #error_ident> {
                    let mut readings = std::collections::HashMap::new();
                    #(#proto_conversion_logic)*
                    Ok(readings)
                }
            }
        }
    }
}

fn handle_number_field(
    name: &Ident,
    field: &Field,
    macro_attrs: &MacroAttributes,
) -> Result<PgnComposition, TokenStream> {
    let bits_size: usize = macro_attrs.bits.unwrap();
    let scale_token = macro_attrs.scale_token.as_ref();
    let unit = macro_attrs.unit.as_ref();

    let num_ty = &field.ty;
    let mut new_statements = PgnComposition::new();
    let proto_import_prefix = crate::utils::get_proto_import_prefix();
    let prop_name = name.to_string();
    let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});

    let crate_ident = crate::utils::get_micro_nmea_crate_ident();
    let error_ident = quote! {
        #crate_ident::parse_helpers::errors::NumberFieldError
    };
    let raw_fn_name = format_ident!("{}_raw", name);

    new_statements.attribute_getters.push(quote! {
        pub fn #raw_fn_name(&self) -> #num_ty { self.#name }
    });

    let mut return_type = quote! {#num_ty};
    let raw_value_statement = quote! {
        let mut result = self.#raw_fn_name();
    };
    let mut scaling_logic = quote! {};
    let mut unit_conversion_logic = quote! {};

    if let Some(scale_token) = scale_token {
        let name_as_string_ident = name.to_string();
        let max_token = match bits_size {
            8 | 16 | 32 | 64 => {
                quote! { let max = <#num_ty>::MAX; }
            }
            x => {
                let x = x as u32;
                quote! {
                    let base: #num_ty = 2;
                    let max = base.pow(#x);
                }
            }
        };
        scaling_logic = match bits_size {
            x if x > 4 => quote! {
                #max_token
                let result = match result {
                    x if x == max => { return Err(#error_ident::FieldNotPresent(#name_as_string_ident.to_string())); },
                    x => {
                        (x as f64) * #scale_token
                    }
                };
            },
            x if x >= 4 => quote! {
                #max_token
                let result = match result {
                    x if x == max => { return Err(#error_ident::FieldNotPresent(#name_as_string_ident.to_string())); },
                    x if x == (max - 1) => { return Err(#error_ident::FieldError(#name_as_string_ident.to_string())); },
                    x => {
                        (x as f64) * #scale_token
                    }
                };
            },
            _ => quote! {},
        };
        return_type = quote! {f64};
    }

    if let Some(unit) = unit {
        unit_conversion_logic = unit.tokens();
        return_type = quote! {f64};
    }

    new_statements.attribute_getters.push(quote! {
        pub fn #name(&self) -> Result<#return_type, #error_ident> {
            #raw_value_statement
            #scaling_logic
            #unit_conversion_logic
            Ok(result)
        }
    });

    new_statements.proto_conversion_logic.push(quote! {
        let value = #proto_import_prefix::Value {
            kind: Some(#proto_import_prefix::value::Kind::NumberValue(
                self.#name()? as f64
            ))
        };
        readings.insert(#label.to_string(), value);
    });

    let nmea_crate = get_micro_nmea_crate_ident();
    new_statements.parsing_logic.push(quote! {
        let reader = #nmea_crate::parse_helpers::parsers::NumberField::<#num_ty>::new(#bits_size)?;
        let #name = reader.read_from_cursor(&mut cursor)?;
    });

    new_statements.struct_initialization.push(quote! {#name,});
    Ok(new_statements)
}

fn handle_lookup_field(
    name: &Ident,
    field_type: &Type,
    macro_attrs: &MacroAttributes,
) -> Result<PgnComposition, TokenStream> {
    let mut new_statements = PgnComposition::new();
    let bits_size = macro_attrs.bits.unwrap();
    if let Type::Path(type_path) = field_type {
        let enum_type = type_path.clone();
        new_statements.attribute_getters.push(quote! {
            pub fn #name(&self) -> #enum_type { self.#name }
        });

        let nmea_crate = get_micro_nmea_crate_ident();
        let setters = quote! {
            let reader = #nmea_crate::parse_helpers::parsers::LookupField::<#enum_type>::new(#bits_size)?;
            let #name = reader.read_from_cursor(&mut cursor)?;
        };

        new_statements.parsing_logic.push(setters);

        new_statements.struct_initialization.push(quote! {#name,});
        let proto_import_prefix = crate::utils::get_proto_import_prefix();
        let prop_name = name.to_string();
        let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});
        new_statements.proto_conversion_logic.push(quote! {
            let value = self.#name();
            let value = #proto_import_prefix::Value {
                kind: Some(#proto_import_prefix::value::Kind::StringValue(value.to_string()))
            };
            readings.insert(#label.to_string(), value);
        })
    }
    Ok(new_statements)
}
