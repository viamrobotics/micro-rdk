use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, Field, Ident, Lit, Meta, Type};

use crate::utils::{error_tokens, UnitConversion};

/// `MacroAttributes` represents the important information derived from the macro attributes
/// attached to the field of a struct using the PgnMessageDerive macro.
///
/// `bits`: size in bits of the field in the NMEA message byte slice
/// `offset`: the amount of bits to skip in the message data before parsing the field
/// `label`: name of the key for the field when serializing the message to a GenericReadingsResult
/// `scale_token`: scale factor to be applied to the raw value of the field
/// `unit`: unit that the field's value should be converted to before serialization
/// `is_lookup`: whether the field is a lookup field
/// `length_field`: if this field is a list of fieldsets, this is the field whose value represents
/// the expected length of this list
///
/// Ex. of usage (each attribute generates an instance of `MacroAttributes`)
///
/// #[derive(PgnMessageDerive, Debug)]
/// pub struct TemperatureExtendedRange {
///     source_id: u8,
///     instance: u8,
///     #[lookup]
///     #[label = "temp_src"]
///     source: TemperatureSource,
///     #[bits = 24]
///     #[scale = 0.001]
///     #[unit = "C"]
///     temperature: u32,
///     #[scale = 0.1]
///     #[offset = 16]
///     set_temperature: u16,
/// }

#[derive(Debug)]
pub(crate) struct MacroAttributes {
    pub(crate) bits: Option<usize>,
    pub(crate) offset: usize,
    pub(crate) label: Option<TokenStream2>,
    pub(crate) scale_token: Option<TokenStream2>,
    pub(crate) unit: Option<UnitConversion>,
    pub(crate) is_lookup: bool,
    pub(crate) length_field: Option<Ident>,
}

// Attempt to deduce the bit size from the data type
fn get_bits(field_ty: &Type) -> Result<usize, TokenStream> {
    Ok(match field_ty {
        Type::Path(type_path) if type_path.path.is_ident("u32") => 32,
        Type::Path(type_path) if type_path.path.is_ident("u16") => 16,
        Type::Path(type_path) if type_path.path.is_ident("u8") => 8,
        Type::Path(type_path) if type_path.path.is_ident("i32") => 32,
        Type::Path(type_path) if type_path.path.is_ident("i16") => 16,
        Type::Path(type_path) if type_path.path.is_ident("i8") => 8,
        Type::Path(type_path) if type_path.path.is_ident("i64") => 64,
        Type::Path(type_path) if type_path.path.is_ident("u64") => 64,
        Type::Array(type_array) => {
            if let Expr::Lit(len) = &type_array.len {
                if let Lit::Int(len) = &len.lit {
                    if let Type::Path(type_path) = type_array.elem.as_ref() {
                        if !type_path.path.is_ident("u8") {
                            return Err(error_tokens("array instance type must be u8"));
                        }
                    }
                    len.base10_parse::<usize>()
                        .map(|x| x * 8)
                        .map_err(|err| err.to_compile_error())?
                } else {
                    return Err(error_tokens("array length is unexpected literal type"));
                }
            } else {
                return Err(error_tokens("array length is unexpected non-literal type"));
            }
        }
        _ => 8,
    })
}

impl MacroAttributes {
    pub(crate) fn from_field(field: &Field) -> Result<Self, TokenStream> {
        let mut macro_attrs = MacroAttributes {
            is_lookup: false,
            scale_token: None,
            bits: None,
            offset: 0,
            label: None,
            length_field: None,
            unit: None,
        };

        for attr in field.attrs.iter() {
            for seg in attr.path().segments.iter() {
                let ident = &seg.ident;
                match ident.to_string().as_str() {
                    "lookup" => macro_attrs.is_lookup = true,
                    "scale" => {
                        let meta = &attr.meta;

                        macro_attrs.scale_token = Some(match meta {
                            Meta::NameValue(named) => {
                                if let Expr::Lit(ref expr_lit) = named.value {
                                    let scale_lit = expr_lit.lit.clone();
                                    quote!(#scale_lit)
                                } else {
                                    return Err(error_tokens("scale parameter must be float"));
                                }
                            }
                            _ => {
                                return Err(error_tokens(
                                    "scale received unexpected attribute value",
                                ));
                            }
                        });
                    }
                    "bits" => {
                        macro_attrs.bits.get_or_insert(match &attr.meta {
                            Meta::NameValue(named) => {
                                if let Expr::Lit(ref expr_lit) = named.value {
                                    let bits_lit = expr_lit.lit.clone();
                                    if let Lit::Int(bits_lit) = bits_lit {
                                        match bits_lit.base10_parse::<usize>() {
                                            Ok(bits) => bits,
                                            Err(err) => {
                                                return Err(error_tokens(err.to_string().as_str()));
                                            }
                                        }
                                    } else {
                                        return Err(error_tokens("bits parameter must be int"));
                                    }
                                } else {
                                    return Err(error_tokens("bits parameter must be int"));
                                }
                            }
                            _ => {
                                return Err(error_tokens(
                                    "bits received unexpected attribute value",
                                ));
                            }
                        });
                    }
                    "offset" => {
                        macro_attrs.offset = match &attr.meta {
                            Meta::NameValue(named) => {
                                if let Expr::Lit(ref expr_lit) = named.value {
                                    let offset_lit = expr_lit.lit.clone();
                                    if let Lit::Int(offset_lit) = offset_lit {
                                        match offset_lit.base10_parse::<usize>() {
                                            Ok(offset) => offset,
                                            Err(err) => {
                                                return Err(error_tokens(err.to_string().as_str()));
                                            }
                                        }
                                    } else {
                                        return Err(error_tokens("offset parameter must be int"));
                                    }
                                } else {
                                    return Err(error_tokens("offset parameter must be int"));
                                }
                            }
                            _ => {
                                return Err(error_tokens(
                                    "offset received unexpected attribute value",
                                ));
                            }
                        };
                    }
                    "label" => {
                        macro_attrs.label = Some(match &attr.meta {
                            Meta::NameValue(named) => {
                                if let Expr::Lit(ref expr_lit) = named.value {
                                    let label_lit = expr_lit.lit.clone();
                                    if let Lit::Str(label_lit) = label_lit {
                                        let label_token = label_lit.token();
                                        quote! {#label_token}
                                    } else {
                                        return Err(error_tokens("label parameter must be str"));
                                    }
                                } else {
                                    return Err(error_tokens("label parameter must be str"));
                                }
                            }
                            _ => {
                                return Err(error_tokens(
                                    "label received unexpected attribute value",
                                ));
                            }
                        })
                    }
                    "length_field" => {
                        let length_field_path: syn::Path = match &attr.meta {
                            Meta::NameValue(named) => {
                                if let Expr::Lit(ref expr_lit) = named.value {
                                    if let Lit::Str(lit_str) = &expr_lit.lit {
                                        lit_str.parse().map_err(|_| error_tokens("uh oh"))?
                                    } else {
                                        return Err(error_tokens(
                                            "length_field parameter must be string",
                                        ));
                                    }
                                } else {
                                    return Err(error_tokens(
                                        "length_field parameter must be string",
                                    ));
                                }
                            }
                            _ => {
                                return Err(error_tokens(
                                    "length_field received unexpected attribute value",
                                ));
                            }
                        };
                        macro_attrs.length_field = Some(
                            length_field_path
                                .get_ident()
                                .ok_or(error_tokens(
                                    "length_field did not resolve to Ident properly",
                                ))?
                                .clone(),
                        );
                    }
                    "unit" => {
                        macro_attrs.unit = Some(match &attr.meta {
                            Meta::NameValue(named) => {
                                if let Expr::Lit(ref expr_lit) = named.value {
                                    let unit_lit = expr_lit.lit.clone();
                                    if let Lit::Str(unit_lit) = unit_lit {
                                        let unit_token = unit_lit.token();
                                        let unit_str = unit_token.to_string();
                                        UnitConversion::try_from(
                                            unit_str.as_str().trim_matches('"'),
                                        )?
                                    } else {
                                        return Err(error_tokens("unit parameter must be str"));
                                    }
                                } else {
                                    return Err(error_tokens("unit parameter must be str"));
                                }
                            }
                            _ => {
                                return Err(error_tokens(
                                    "unit received unexpected attribute value",
                                ));
                            }
                        })
                    }
                    _ => {}
                };
            }
        }
        macro_attrs.bits.get_or_insert(get_bits(&field.ty)?);
        Ok(macro_attrs)
    }
}
