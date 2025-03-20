use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{DeriveInput, Field, GenericArgument, Ident, PathArguments, Type};

use crate::attributes::MacroAttributes;
use crate::utils::{error_tokens, get_micro_nmea_crate_ident, is_supported_integer_type};

#[derive(Debug, Clone, Copy)]
pub(crate) enum CodeGenPurpose {
    Message,
    Fieldset,
}

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
    pub(crate) pgn_declaration: Option<TokenStream2>,
}

impl PgnComposition {
    pub(crate) fn new() -> Self {
        Self {
            attribute_getters: vec![],
            parsing_logic: vec![],
            struct_initialization: vec![],
            proto_conversion_logic: vec![],
            pgn_declaration: None,
        }
    }

    pub(crate) fn set_pgn_declaration(&mut self, dec: TokenStream2) {
        self.pgn_declaration = Some(dec)
    }

    pub(crate) fn merge(&mut self, mut other: Self) {
        self.attribute_getters.append(&mut other.attribute_getters);
        self.parsing_logic.append(&mut other.parsing_logic);
        self.struct_initialization
            .append(&mut other.struct_initialization);
        self.proto_conversion_logic
            .append(&mut other.proto_conversion_logic);
        if other.pgn_declaration.is_some() {
            self.pgn_declaration = other.pgn_declaration;
        }
    }

    pub(crate) fn from_field(field: &Field, purpose: CodeGenPurpose) -> Result<Self, TokenStream> {
        let mut statements = Self::new();
        if let Some(name) = &field.ident {
            let macro_attrs = MacroAttributes::from_field(field)?;

            if name == "_pgn" {
                if let Some(pgn) = macro_attrs.pgn {
                    statements.set_pgn_declaration(quote! {
                        const PGN: u32 = #pgn;
                    });
                    statements
                        .struct_initialization
                        .push(quote! { _pgn: std::marker::PhantomData, });
                    return Ok(statements);
                } else {
                    let err_msg = format!(
                        "pgn field must define pgn attribute, macro attributes: {:?}",
                        macro_attrs
                    );
                    return Err(error_tokens(&err_msg));
                }
            }

            if macro_attrs.offset != 0 {
                let offset = macro_attrs.offset;
                statements
                    .parsing_logic
                    .push(quote! { let _ = cursor.read(#offset)?; });
            }

            let new_statements = if is_supported_integer_type(&field.ty) {
                handle_number_field(name, field, &macro_attrs, purpose)?
            } else if macro_attrs.is_lookup {
                handle_lookup_field(name, &field.ty, &macro_attrs, purpose)?
            } else if field.attrs.iter().any(|attr| {
                attr.path()
                    .segments
                    .iter()
                    .any(|seg| seg.ident.to_string().as_str() == "fieldset")
            }) {
                handle_fieldset(name, field, &macro_attrs, purpose)?
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

    pub(crate) fn from_input(
        input: &DeriveInput,
        purpose: CodeGenPurpose,
    ) -> Result<Self, TokenStream> {
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
            match PgnComposition::from_field(field, purpose) {
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

    pub(crate) fn into_token_stream(
        self,
        input: &DeriveInput,
    ) -> Result<TokenStream2, TokenStream> {
        let name = &input.ident;
        let parsing_logic = self.parsing_logic;
        let attribute_getters = self.attribute_getters;
        let struct_initialization = self.struct_initialization;
        let proto_conversion_logic = self.proto_conversion_logic;
        if self.pgn_declaration.is_none() {
            return Err(error_tokens("pgn field of type u32 required"));
        }
        let pgn_declaration = self.pgn_declaration.unwrap();
        let (impl_generics, src_generics, src_where_clause) = input.generics.split_for_impl();
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        let mrdk_crate = crate::utils::get_micro_rdk_crate_ident();
        Ok(quote! {
            impl #impl_generics #name #src_generics #src_where_clause {
                #(#attribute_getters)*
            }

            impl #impl_generics #crate_ident::messages::message::Message for #name #src_generics #src_where_clause {
                #pgn_declaration

                fn from_cursor(mut cursor: #crate_ident::parse_helpers::parsers::DataCursor) -> Result<Self, #error_ident> {
                    use #crate_ident::parse_helpers::parsers::FieldReader;
                    #(#parsing_logic)*
                    Ok(Self {
                        #(#struct_initialization)*
                    })
                }


                fn to_readings(self) -> Result<#mrdk_crate::common::sensor::GenericReadingsResult, #error_ident> {
                    let mut readings = std::collections::HashMap::new();
                    #(#proto_conversion_logic)*
                    Ok(readings)
                }
            }
        })
    }

    pub(crate) fn into_fieldset_token_stream(self, input: &DeriveInput) -> TokenStream2 {
        let name = &input.ident;
        let parsing_logic = self.parsing_logic;
        let attribute_getters = self.attribute_getters;
        let struct_initialization = self.struct_initialization;
        let proto_conversion_logic = self.proto_conversion_logic;
        let (impl_generics, src_generics, src_where_clause) = input.generics.split_for_impl();
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let mrdk_crate = crate::utils::get_micro_rdk_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        let field_set_ident = quote! {#crate_ident::parse_helpers::parsers::FieldSet};

        quote! {
            impl #impl_generics #name #src_generics #src_where_clause {
                #(#attribute_getters)*
            }

            impl #impl_generics #field_set_ident for #name #src_generics #src_where_clause {
                fn from_data(cursor: &mut DataCursor) -> Result<Self, #error_ident> {
                    use #crate_ident::parse_helpers::parsers::FieldReader;
                    #(#parsing_logic)*
                    Ok(Self {
                        #(#struct_initialization)*
                    })
                }

                fn to_readings(&self) -> Result<#mrdk_crate::common::sensor::GenericReadingsResult, #error_ident> {
                    let mut readings = std::collections::HashMap::new();
                    #(#proto_conversion_logic)*
                    Ok(readings)
                }
            }
        }
    }
}

fn get_read_statement(name: &Ident, purpose: CodeGenPurpose) -> TokenStream2 {
    match purpose {
        CodeGenPurpose::Message => quote! {let #name = reader.read_from_cursor(&mut cursor)?;},
        CodeGenPurpose::Fieldset => quote! {let #name = reader.read_from_cursor(cursor)?;},
    }
}

fn handle_number_field(
    name: &Ident,
    field: &Field,
    macro_attrs: &MacroAttributes,
    purpose: CodeGenPurpose,
) -> Result<PgnComposition, TokenStream> {
    let bits_size: usize = macro_attrs.bits;
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
            x if ((x < 4) && x > 1) => quote! {
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
    let read_statement = get_read_statement(name, purpose);
    new_statements.parsing_logic.push(quote! {
        let reader = #nmea_crate::parse_helpers::parsers::NumberField::<#num_ty>::new(#bits_size)?;
        #read_statement
    });

    new_statements.struct_initialization.push(quote! {#name,});
    Ok(new_statements)
}

fn handle_lookup_field(
    name: &Ident,
    field_type: &Type,
    macro_attrs: &MacroAttributes,
    purpose: CodeGenPurpose,
) -> Result<PgnComposition, TokenStream> {
    let mut new_statements = PgnComposition::new();
    let bits_size = macro_attrs.bits;
    if let Type::Path(type_path) = field_type {
        let enum_type = type_path.clone();
        new_statements.attribute_getters.push(quote! {
            pub fn #name(&self) -> #enum_type { self.#name }
        });

        let nmea_crate = get_micro_nmea_crate_ident();
        let read_statement = get_read_statement(name, purpose);
        let setters = quote! {
            let reader = #nmea_crate::parse_helpers::parsers::LookupField::<#enum_type>::new(#bits_size)?;
            #read_statement
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

fn handle_fieldset(
    name: &Ident,
    field: &Field,
    macro_attrs: &MacroAttributes,
    purpose: CodeGenPurpose,
) -> Result<PgnComposition, TokenStream> {
    let mut new_statements = PgnComposition::new();
    if field.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .iter()
            .any(|seg| seg.ident.to_string().as_str() == "fieldset")
    }) {
        let f_type = match &field.ty {
            Type::Path(type_path) => {
                let vec_seg = &type_path.path.segments[0];
                if &vec_seg.ident.to_string() != "Vec" {
                    Err(error_tokens("fieldset must be Vec"))
                } else if let PathArguments::AngleBracketed(args) = &vec_seg.arguments {
                    let type_arg = &args.args[0];
                    if let GenericArgument::Type(f_type) = type_arg {
                        Ok(f_type.to_token_stream())
                    } else {
                        Err(error_tokens("fieldset must be Vec with type"))
                    }
                } else {
                    Err(error_tokens("fieldset must be Vec with angle brackets"))
                }
            }
            _ => Err(error_tokens("improper field type")),
        }?;

        let length_field_token = macro_attrs.length_field.as_ref().ok_or(error_tokens(
            "length_field field must be specified for fieldset",
        ))?;

        let nmea_crate = get_micro_nmea_crate_ident();
        let read_statement = get_read_statement(name, purpose);
        new_statements.parsing_logic.push(quote! {
            let reader = #nmea_crate::parse_helpers::parsers::FieldSetList::<#f_type>::new(#length_field_token as usize);
            #read_statement
        });

        new_statements.attribute_getters.push(quote! {
            pub fn #name(&self) -> Vec<#f_type> { self.#name.clone() }
        });
        new_statements.struct_initialization.push(quote! {#name,});
        let proto_import_prefix = crate::utils::get_proto_import_prefix();
        let prop_name = name.to_string();
        let label = macro_attrs.label.clone().unwrap_or(quote! {#prop_name});
        let crate_ident = crate::utils::get_micro_nmea_crate_ident();
        let error_ident = quote! {#crate_ident::parse_helpers::errors::NmeaParseError};
        new_statements.proto_conversion_logic.push(quote! {
            let values: Result<Vec<#proto_import_prefix::Value>, #error_ident> = self.#name().iter().map(|inst| {
                inst.to_readings().map(|fields| {
                    #proto_import_prefix::Value {
                        kind: Some(#proto_import_prefix::value::Kind::StructValue(#proto_import_prefix::Struct {
                            fields: fields
                        }))
                    }
                })
            }).collect();
            let value = #proto_import_prefix::Value {
                kind: Some(#proto_import_prefix::value::Kind::ListValue(#proto_import_prefix::ListValue {
                    values: values?
                }))
            };
            readings.insert(#label.to_string(), value);
        });
        Ok(new_statements)
    } else {
        Err(error_tokens("msg"))
    }
}
