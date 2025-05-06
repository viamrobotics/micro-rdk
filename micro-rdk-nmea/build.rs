use check_keyword::CheckKeyword;
use num2words::Num2Words;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::Path;
use std::slice::Iter;

use convert_case::{Case, Casing};

// This is to compute a valid name for a struct, enum member, or field/variable. We
// want to remove special characters and introduce a default convention for
// when a name starts with a number.
fn clean_string_for_rust(input: &str, case: Case) -> String {
    let mut filtered: String = input
        .chars()
        .enumerate()
        .map(|(idx, c)| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else if (c == '-') && (idx == 0) {
                // handling negative number?
                c
            } else {
                '_'
            }
        })
        .collect();
    if filtered
        .chars()
        .position(|c| (c.is_numeric() || (c == '_') || (c == '-')))
        .map(|idx| idx == 0)
        .unwrap_or_default()
    {
        let mut num_string = "".to_string();
        let mut end_idx = 0;
        let mut is_negative = false;
        for (idx, c) in filtered.chars().enumerate() {
            if c.is_numeric() {
                num_string.push(c);
            } else if (c == '_') && (idx == 0) {
                continue;
            } else if (idx == 0) && (c == '-') {
                is_negative = true;
            } else {
                end_idx = idx;
                break;
            }
        }

        let num_words = if num_string.is_empty() {
            "".to_string()
        } else {
            let num = num_string.parse::<u32>().unwrap();
            let mut num_words = Num2Words::new(num).to_words().unwrap();
            if is_negative {
                num_words = format!("Minus{}", num_words);
            }
            num_words
        };

        (num_words + &filtered.split_off(end_idx)).to_case(case)
    } else {
        filtered.to_case(case)
    }
}

#[derive(Serialize, Deserialize)]
struct EnumValueTypeSettings {
    value: u32,
    field_type: String,
    lookup_name: String,
    lookup_size: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct LookupJson {
    name: String,
    lookup_type: u8,
    size: usize,
}

#[derive(Serialize, Deserialize)]
struct FieldParametersJson {
    name: String,
    field_type: String,
    size: usize,
    resolution: f64,
    lookup: LookupJson,
    has_sign: bool,
    unit: String,
}

#[derive(Debug, Clone, Copy)]
enum SimplifiedNumberType {
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    Float32,
}

impl SimplifiedNumberType {
    fn get_string(&self) -> String {
        match self {
            Self::Int8 => "i8",
            Self::Uint8 => "u8",
            Self::Int16 => "i16",
            Self::Uint16 => "u16",
            Self::Int32 => "i32",
            Self::Uint32 => "u32",
            Self::Int64 => "i64",
            Self::Uint64 => "u64",
            Self::Float32 => "f32",
        }
        .to_string()
    }
}

struct NumberFieldParameters {
    num_type: SimplifiedNumberType,
    scale: f64,
    unit: String,
    size: usize,
    is_mmsi: bool,
    value_offset: i32,
}

impl NumberFieldParameters {
    fn has_trivial_size(&self) -> bool {
        self.size
            == match self.num_type {
                SimplifiedNumberType::Int16 | SimplifiedNumberType::Uint16 => 16,
                SimplifiedNumberType::Int32
                | SimplifiedNumberType::Uint32
                | SimplifiedNumberType::Float32 => 32,
                SimplifiedNumberType::Int64 | SimplifiedNumberType::Uint64 => 64,
                SimplifiedNumberType::Int8 | SimplifiedNumberType::Uint8 => 8,
            }
    }

    fn to_polymorphic_type_parameters(&self) -> (String, String) {
        let mut final_type: String = self.num_type.get_string();
        let reader_instance = if self.scale == 1.0 {
            format!(
                "NumberField::<{}>::new({})?",
                self.num_type.get_string(),
                self.size
            )
        } else {
            let res = if self.scale.fract() == 0.0 {
                format!(
                    "NumberFieldWithScale::<{}>::new({}, {:.1})?",
                    self.num_type.get_string(),
                    self.size,
                    self.scale
                )
            } else {
                format!(
                    "NumberFieldWithScale::<{}>::new({}, {})?",
                    self.num_type.get_string(),
                    self.size,
                    self.scale
                )
            };
            final_type = "f64".to_string();
            res
        };
        (reader_instance, final_type)
    }
}

impl TryFrom<&EnumValueTypeSettings> for NumberFieldParameters {
    type Error = String;
    fn try_from(value: &EnumValueTypeSettings) -> Result<NumberFieldParameters, Self::Error> {
        field_type_to_number_field_params(&value.field_type)
    }
}

impl TryFrom<&FieldParametersJson> for NumberFieldParameters {
    type Error = String;
    fn try_from(value: &FieldParametersJson) -> Result<Self, Self::Error> {
        field_type_to_number_field_params(&value.field_type)
    }
}

struct ReservedField {
    size: usize,
}

#[derive(Debug, Clone, Copy)]
enum StringFieldType {
    Fixed,
    VarLength,
    VarLengthWithEncoding,
}

impl TryFrom<&str> for StringFieldType {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "STRING_FIX" => Self::Fixed,
            "STRING_LZ" => Self::VarLength,
            "STRING_LAU" => Self::VarLengthWithEncoding,
            x => {
                return Err(format!(
                    "received unexpected value {} for string field type",
                    x
                ));
            }
        })
    }
}

struct StringFieldParameters {
    string_field_type: StringFieldType,
    size: usize,
}

struct BinaryFieldParameters {
    size: usize,
}

struct DecimalFieldParameters {
    size: usize,
}

struct PolymorphicParameters {
    key_field: String,
    lookup_name: String,
}

enum FieldParameters {
    NumberField(NumberFieldParameters),
    StringField(StringFieldParameters),
    BinaryField(BinaryFieldParameters),
    ReservedField(ReservedField),
    LookupField(LookupJson),
    Polymorphic(PolymorphicParameters),
    DecimalField(DecimalFieldParameters),
}

struct Field {
    name: String,
    params: FieldParameters,
}

impl Field {
    fn from_polymorphic_parameters(name: String, params: PolymorphicParameters) -> Self {
        Self {
            name,
            params: FieldParameters::Polymorphic(params),
        }
    }

    fn into_field_string(self, offset: usize) -> String {
        let field_name_cleaned = clean_string_for_rust(self.name.as_str(), Case::Snake).into_safe();
        let offset_tag = if offset != 0 {
            format!("\t#[offset = {}]\n", offset)
        } else {
            "".to_string()
        };
        format!(
            "{}{}\t{}: {},\n\n",
            offset_tag,
            self.tags(),
            field_name_cleaned,
            self.r#type()
        )
    }

    fn tags(&self) -> String {
        match &self.params {
            FieldParameters::LookupField(lookup_info) => {
                let size = lookup_info.size;
                format!("\t#[lookup]\n\t#[bits = {}]\n", size)
            }
            FieldParameters::StringField(params) => match params.string_field_type {
                StringFieldType::Fixed => {
                    format!("\t#[bits = {}]\n", params.size)
                }
                StringFieldType::VarLength => "".to_string(),
                StringFieldType::VarLengthWithEncoding => "\t#[variable_encoding]\n".to_string(),
            },
            FieldParameters::NumberField(params) => {
                let mut tags = "".to_string();
                if params.scale != 1.0 {
                    let scale_tag = if params.scale.fract() == 0.0 {
                        format!("\t#[scale = {:.1}]\n", params.scale)
                    } else {
                        format!("\t#[scale = {}]\n", params.scale)
                    };
                    tags.push_str(&scale_tag);
                }
                if params.is_mmsi {
                    tags.push_str("\t#[mmsi]\n");
                }
                if !params.unit.is_empty() {
                    let unit_tag = format!("\t#[unit = \"{}\"]\n", params.unit);
                    tags.push_str(&unit_tag);
                }
                if !params.has_trivial_size() {
                    let size_tag = format!("\t#[bits = {}]\n", params.size);
                    tags.push_str(&size_tag);
                }
                if params.value_offset != 0 {
                    let offset_tag = format!("\t#[value_offset = {}]\n", params.value_offset);
                    tags.push_str(&offset_tag);
                }
                tags
            }
            FieldParameters::DecimalField(params) => {
                format!("\t#[bits = {}]\n", params.size)
            }
            FieldParameters::Polymorphic(params) => {
                format!(
                    "\t#[polymorphic]\n\t#[lookup_field = \"{}\"]\n",
                    params.key_field
                )
            }
            _ => "".to_string(),
        }
    }

    fn r#type(&self) -> String {
        match &self.params {
            FieldParameters::LookupField(lookup_info) => {
                let lookup_name = lookup_info.name.clone().to_case(Case::Pascal);
                format!("{}Lookup", lookup_name)
            }
            FieldParameters::BinaryField(params) => {
                format!("[u8; {}]", params.size)
            }
            FieldParameters::StringField(_) => "String".to_string(),
            FieldParameters::NumberField(params) => params.num_type.get_string(),
            FieldParameters::Polymorphic(params) => {
                params.lookup_name.clone().to_case(Case::Pascal)
            }
            FieldParameters::DecimalField(_) => "u128".to_string(),
            _ => "".to_string(),
        }
    }
}

impl TryFrom<&FieldParametersJson> for Field {
    type Error = String;
    fn try_from(value: &FieldParametersJson) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name.clone(),
            params: FieldParameters::try_from(value)?,
        })
    }
}

impl TryFrom<&FieldParametersJson> for FieldParameters {
    type Error = String;
    fn try_from(value: &FieldParametersJson) -> Result<Self, Self::Error> {
        if value.lookup.name.as_str() != "" {
            let mut val = value.lookup.clone();
            val.size = value.size;
            Ok(Self::LookupField(val))
        } else {
            Ok(match value.field_type.as_str() {
                "BINARY" => Self::BinaryField(BinaryFieldParameters { size: value.size }),
                "STRING_FIX" | "STRING_LZ" | "STRING_LAU" => {
                    Self::StringField(StringFieldParameters {
                        string_field_type: StringFieldType::try_from(value.field_type.as_str())?,
                        size: value.size,
                    })
                }
                "RESERVED" | "SPARE" => Self::ReservedField(ReservedField { size: value.size }),
                "INTEGER" | "UNSIGNED_INTEGER" => {
                    let params = match (value.size, value.has_sign) {
                        x if (x.0 <= 8) && x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Int8,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 8) && !x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Uint8,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 16) && x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Int16,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 16) && !x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Uint16,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 32) && x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Int32,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 32) && !x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Uint32,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 64) && x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Int64,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        x if (x.0 <= 64) && !x.1 => NumberFieldParameters {
                            num_type: SimplifiedNumberType::Uint64,
                            size: x.0,
                            scale: 1.0,
                            unit: value.unit.clone(),
                            is_mmsi: false,
                            value_offset: 0,
                        },
                        _ => unreachable!(),
                    };
                    Self::NumberField(params)
                }
                "DECIMAL" => Self::DecimalField(DecimalFieldParameters { size: value.size }),
                _ => Self::NumberField(field_type_to_number_field_params(&value.field_type)?),
            })
        }
    }
}

#[derive(Serialize, Deserialize)]
struct MessageJson {
    pgn: u32,
    field_list: Vec<FieldParametersJson>,
    repeating_count_1: usize,
    repeating_start_1: usize,
    repeating_count_2: usize,
    repeating_start_2: usize,
}

impl MessageJson {
    fn into_bytes(self) -> Result<Option<(String, Vec<u8>)>, String> {
        let standard_pgn_range_1 = 61440..65279;
        let standard_pgn_range_2 = 126976..130815;
        // excluded until we support variable length fieldsets and indirect lookups (also there are two 129808 representations which follows the pattern for proprietary messages)
        let exclude_pgns: Vec<u32> = vec![126464, 129805, 129808, 60928, 65240];

        if (standard_pgn_range_1.contains(&self.pgn) || standard_pgn_range_2.contains(&self.pgn))
            && !exclude_pgns.contains(&self.pgn)
        {
            let mut message_struct_bytes: Vec<u8> = Vec::new();

            let (fieldset_name_a, repeating_start_1, repeating_end_1) =
                if self.repeating_start_1 != 0 {
                    let repeating_start_1 = self.repeating_start_1 - 1;
                    let repeating_end_1 = repeating_start_1 + self.repeating_count_1;
                    (
                        format!("Pgn{}FieldsetA", self.pgn),
                        repeating_start_1,
                        repeating_end_1,
                    )
                } else {
                    ("".to_string(), 0, 0)
                };

            let (fieldset_name_b, repeating_start_2, repeating_end_2) =
                if self.repeating_start_2 != 0 {
                    let repeating_start_2 = self.repeating_start_2 - 1;
                    let repeating_end_2 = repeating_start_2 + self.repeating_count_2;
                    (
                        format!("Pgn{}FieldsetB", self.pgn),
                        repeating_start_2,
                        repeating_end_2,
                    )
                } else {
                    ("".to_string(), 0, 0)
                };

            if repeating_end_1 > self.field_list.len() {
                println!("check pgn {:?}", self.pgn)
            }

            let a_field_objs = self.field_list[repeating_start_1..repeating_end_1].iter();
            let b_field_objs = self.field_list[repeating_start_2..repeating_end_2].iter();

            if a_field_objs.len() != 0 {
                let mut fieldset_a_bytes =
                    write_fieldset_struct(fieldset_name_a.clone(), a_field_objs)?;
                message_struct_bytes.append(&mut fieldset_a_bytes);
                if b_field_objs.len() != 0 {
                    let mut fieldset_b_bytes =
                        write_fieldset_struct(fieldset_name_b.clone(), b_field_objs)?;
                    message_struct_bytes.append(&mut fieldset_b_bytes);
                };
            };

            let mut derive_tag_bytes = b"#[derive(PgnMessageDerive, Clone, Debug)]\n".to_vec();
            message_struct_bytes.append(&mut derive_tag_bytes);
            let struct_name = format!("Pgn{}Message", self.pgn);
            let mut struct_entry = format!("pub struct {} {{\n", struct_name)
                .as_bytes()
                .to_vec();
            message_struct_bytes.append(&mut struct_entry);
            let mut pgn_field = format!(
                "\t#[pgn = {}]\n\t_pgn: std::marker::PhantomData<u32>,\n\n",
                self.pgn
            )
            .as_bytes()
            .to_vec();
            message_struct_bytes.append(&mut pgn_field);

            write_field_segments(
                &mut message_struct_bytes,
                self.field_list.iter(),
                repeating_start_1,
                repeating_end_1,
                repeating_start_2,
                repeating_end_2,
                fieldset_name_a,
                fieldset_name_b,
            )?;
            message_struct_bytes.append(&mut b"}\n\n".to_vec());

            Ok(Some((struct_name, message_struct_bytes)))
        } else {
            Ok(None)
        }
    }
}

fn field_type_to_number_field_params(field_type: &str) -> Result<NumberFieldParameters, String> {
    Ok(match field_type {
        "INT8" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int8,
            scale: 1.0,
            unit: "".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "FIX8" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int8,
            scale: 1.0,
            unit: "".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "UINT8" | "INSTANCE" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 1.0,
            unit: "".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "UFIX8" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 1.0,
            unit: "".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "INT16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1.0,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1.0,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "FIX16_1" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.1,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "UINT16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "UFIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VERSION" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.001,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "DILUTION_OF_PRECISION_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "SIGNALTONOISERATIO_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "SIGNALTONOISERATIO_UFIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.01,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },

        "UFIX16_3" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.001,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "INT32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 1.0,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "FIX32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 1.0,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "UINT32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "UFIX32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "INT64" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int64,
            scale: 1.0,
            unit: "".to_string(),
            size: 64,
            is_mmsi: false,
            value_offset: 0,
        },
        "FIX64" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int64,
            scale: 1.0,
            unit: "".to_string(),
            size: 64,
            is_mmsi: false,
            value_offset: 0,
        },
        "UINT64" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint64,
            scale: 1.0,
            unit: "".to_string(),
            size: 64,
            is_mmsi: false,
            value_offset: 0,
        },
        "UFIX64" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint64,
            scale: 1.0,
            unit: "".to_string(),
            size: 64,
            is_mmsi: false,
            value_offset: 0,
        },
        "UFIX32_2" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.001,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "FIX32_2" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 0.01,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "FLOAT" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Float32,
            scale: 1.0,
            unit: "".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "ANGLE_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1.0e-4,
            unit: "rad".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "ANGLE_FIX16_DDEG" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.1,
            unit: "deg".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "ANGLE_UFIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0e-4,
            unit: "rad".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "LENGTH_UFIX32_CM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.01,
            unit: "m".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "LENGTH_UFIX16_CM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.01,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "LENGTH_UFIX16_DM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.1,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "LENGTH_UFIX8_DAM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 10.0,
            unit: "m".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "LENGTH_UFIX32_M" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "m".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "LENGTH_UFIX32_MM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.001,
            unit: "m".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "GEO_FIX32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 1.0e-7,
            unit: "deg".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "GEO_FIX64" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int64,
            scale: 1.0e-16,
            unit: "deg".to_string(),
            size: 64,
            is_mmsi: false,
            value_offset: 0,
        },
        "SPEED_UFIX16_CM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.01,
            unit: "m/s".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "SPEED_FIX16_CM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "m/s".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "DISTANCE_FIX32_CM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 0.01,
            unit: "m".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "DISTANCE_FIX16_CM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "DISTANCE_FIX16_M" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1.0,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "DISTANCE_FIX16_MM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.001,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "DISTANCE_FIX32_MM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 0.001,
            unit: "m".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "DISTANCE_FIX64" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int64,
            scale: 1.0e-6,
            unit: "m".to_string(),
            size: 64,
            is_mmsi: false,
            value_offset: 0,
        },
        "GAIN_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "MAGNETIC_FIELD_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "T".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },

        "TEMPERATURE" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.01,
            unit: "K".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TEMPERATURE_HIGH" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.1,
            unit: "K".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TEMPERATURE_UFIX24" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.001,
            unit: "K".to_string(),
            size: 24,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLUMETRIC_FLOW" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.1,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "CONCENTRATION_UINT16_PPM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLUME_UFIX16_L" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLUME_UFIX16_DL" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.1,
            unit: "".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PERCENTAGE_FIX16_D" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.1,
            unit: "%".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PERCENTAGE_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1.0,
            unit: "%".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PERCENTAGE_UINT8" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 1.0,
            unit: "%".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "PERCENTAGE_INT8" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int8,
            scale: 1.0,
            unit: "%".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_FIX32_MS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 0.001,
            unit: "sec".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX32_S" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "sec".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX32_MS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.001,
            unit: "sec".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX24_MS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.001,
            unit: "sec".to_string(),
            size: 24,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.0001,
            unit: "sec".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.0001,
            unit: "sec".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX16_S" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "sec".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX16_MS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.001,
            unit: "sec".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX16_CS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.01,
            unit: "sec".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX8_5MS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.005,
            unit: "sec".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX8_P12S" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 2.0_f64.powi(12),
            unit: "sec".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_FIX16_MIN" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 60.0,
            unit: "sec".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_FIX16_5CS" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.05,
            unit: "sec".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "TIME_UFIX16_MIN" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 60.0,
            unit: "sec".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "DATE" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "days".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "MMSI" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "".to_string(),
            size: 32,
            is_mmsi: true,
            value_offset: 0,
        },
        "VOLTAGE_UFIX16_10MV" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.01,
            unit: "V".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLTAGE_FIX16_10MV" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.01,
            unit: "V".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLTAGE_UFIX16_50MV" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.05,
            unit: "V".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLTAGE_UFIX16_100MV" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.1,
            unit: "V".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLTAGE_UFIX16_200MV" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.2,
            unit: "V".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "VOLTAGE_UFIX16_V" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "V".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "CURRENT_UFIX16_A" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "A".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "CURRENT_UFIX16_DA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.1,
            unit: "A".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "CURRENT_FIX16_DA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.1,
            unit: "A".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "CURRENT_FIX24_CA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 0.01,
            unit: "A".to_string(),
            size: 24,
            is_mmsi: false,
            value_offset: 0,
        },
        "ELECTRIC_CHARGE_UFIX16_AH" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "Ah".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PEUKERT_EXPONENT" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 0.002,
            unit: "".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "ENERGY_UINT32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "kWh".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "POWER_FIX32_OFFSET" | "POWER_FIX32_VA_OFFSET" | "POWER_FIX32_VAR_OFFSET" => {
            NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint32,
                scale: 1.0,
                unit: "".to_string(),
                size: 32,
                is_mmsi: false,
                value_offset: 2000000000, // turns into negative
            }
        }
        "POWER_UINT16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "W".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "POWER_UINT16_VAR" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "var".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "POWER_INT32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 1.0,
            unit: "W".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "POWER_UINT32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "W".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "POWER_UINT32_VAR" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "var".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "POWER_UINT32_VA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 1.0,
            unit: "VA".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "ROTATION_FIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: (1.0e-3 / 32.0),
            unit: "rad/s".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "ROTATION_FIX32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: (1.0e-6 / 32.0),
            unit: "rad/s".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "ROTATION_UFIX16_RPM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.25,
            unit: "rpm".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_UFIX16_HPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 100.0,
            unit: "Pa".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_UFIX16_KPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1000.0,
            unit: "Pa".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_UFIX32_DPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 0.1,
            unit: "Pa".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_FIX32_DPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int32,
            scale: 0.1,
            unit: "Pa".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_FIX16_KPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1000.0,
            unit: "Pa".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_UINT8_2KPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 2000.0,
            unit: "Pa".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_UINT8_KPA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint8,
            scale: 1000.0,
            unit: "Pa".to_string(),
            size: 8,
            is_mmsi: false,
            value_offset: 0,
        },
        "PRESSURE_RATE_FIX16_PA" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 1.0,
            unit: "Pa/hr".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "RADIO_FREQUENCY_UFIX32" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint32,
            scale: 10.0,
            unit: "Hz".to_string(),
            size: 32,
            is_mmsi: false,
            value_offset: 0,
        },
        "FREQUENCY_UFIX16" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 1.0,
            unit: "Hz".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "SPEED_FIX16_MM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Int16,
            scale: 0.001,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        "SPEED_UFIX16_DM" => NumberFieldParameters {
            num_type: SimplifiedNumberType::Uint16,
            scale: 0.1,
            unit: "m".to_string(),
            size: 16,
            is_mmsi: false,
            value_offset: 0,
        },
        x => {
            return Err(format!("encountered unsupported field type {}", x));
        }
    })
}

fn write_basic_enums(lookups: &Map<String, Value>, enums_file: &mut File) {
    for (enum_name, value_map_value) in lookups {
        let mut prefix = "define_nmea_enum!(\n".as_bytes().to_vec();
        let adj_name = format!("{}Lookup", enum_name);
        writeln!(
            &mut prefix,
            "\t{},",
            adj_name.as_str().to_case(Case::Pascal)
        )
        .inspect_err(|err| println!("failed to write line: {:?}", err))
        .unwrap();
        let value_map = match value_map_value {
            Value::Object(obj) => obj,
            _ => {
                panic!("improperly formatted lookup")
            }
        };
        for (enum_str, num_val_val) in value_map.iter() {
            let enum_val = clean_string_for_rust(enum_str.clone().as_str(), Case::Pascal);
            let num_val = match num_val_val {
                Value::Number(num) => num,
                _ => {
                    panic!("improperly formatted lookup")
                }
            };
            let num = num_val
                .as_u64()
                .expect("value for lookup label must be an integer");
            writeln!(&mut prefix, "\t({}, {}, \"{}\"),", num, enum_val, enum_str)
                .inspect_err(|err| println!("failed to write line: {:?}", err))
                .unwrap();
        }
        writeln!(&mut prefix, "\tUnknownLookupField\n);")
            .inspect_err(|err| println!("failed to write line: {:?}", err))
            .unwrap();
        enums_file
            .write_all(&prefix)
            .expect("could not write parsed lookup to enums.rs");
    }
}

fn write_polymorphic_types(
    polymorphisms: &Map<String, Value>,
    enums_file: &mut File,
    polymorphic_types_file: &mut File,
) {
    let mut polymorphic_lookups = Map::<String, Value>::new();

    for (enum_name, types) in polymorphisms {
        let mut polymorphic_macro_lines: Vec<String> = Vec::new();
        polymorphic_macro_lines.push("polymorphic_type!(\n".to_string());

        let enum_type_name = enum_name.as_str().to_case(Case::Pascal);
        polymorphic_macro_lines.push(format!("\t{},\n", enum_type_name));
        let enum_type_enum_name = format!("{}Key", enum_type_name);
        polymorphic_macro_lines.push(format!("\t{},\n", enum_type_enum_name));

        let mut enum_values = Map::<String, Value>::new();

        let types_unwrapped: HashMap<String, EnumValueTypeSettings> =
            serde_json::from_value(types.clone()).expect("unable to process enum value");

        for (type_name, type_settings) in types_unwrapped.iter() {
            let (reader, typ) = if !type_settings.lookup_name.is_empty() {
                let lookup_enum_name = format!(
                    "{}Lookup",
                    &type_settings.lookup_name.as_str().to_case(Case::Pascal)
                );
                (
                    format!(
                        "LookupField::<{}>::new({})?",
                        lookup_enum_name, type_settings.lookup_size
                    ),
                    lookup_enum_name,
                )
            } else {
                let num_type_parameters = NumberFieldParameters::try_from(type_settings)
                    .inspect_err(|err| panic!("{}", err))
                    .unwrap();
                num_type_parameters.to_polymorphic_type_parameters()
            };
            let variant_name = clean_string_for_rust(type_name.as_str(), Case::Pascal);
            enum_values.insert(
                variant_name.clone(),
                Value::Number(
                    Number::from_u128(type_settings.value as u128)
                        .expect("encountered unparseable enum value"),
                ),
            );
            polymorphic_macro_lines.push(format!(
                "\t(\n\t\t{},\n\t\t{},\n\t\t{},\n\t\t{}\n\t),\n",
                type_settings.value, variant_name, reader, typ
            ));
        }

        polymorphic_macro_lines.push("\tUnknownLookupField\n);\n".to_string());

        let mut polymorphic_file_bytes: Vec<u8> = Vec::new();

        for line in polymorphic_macro_lines.drain(0..) {
            let mut byte_vec = line.as_bytes().to_vec();
            polymorphic_file_bytes.append(&mut byte_vec);
        }

        polymorphic_types_file
            .write_all(&polymorphic_file_bytes)
            .unwrap_or_else(|_| panic!("failed to write polymorphic type {}", enum_name));

        let _ = polymorphic_lookups.insert(enum_type_enum_name.clone(), Value::Object(enum_values));
    }

    write_basic_enums(&polymorphic_lookups, enums_file);
}

fn write_fieldset_struct(
    fieldset_name: String,
    field_objs: Iter<'_, FieldParametersJson>,
) -> Result<Vec<u8>, String> {
    let mut fieldset_struct_bytes: Vec<u8> = Vec::new();

    let mut derive_tag_bytes = b"#[derive(FieldsetDerive, Clone, Debug)]\n".to_vec();
    fieldset_struct_bytes.append(&mut derive_tag_bytes);
    let mut struct_entry = format!("pub struct {} {{\n", fieldset_name)
        .as_bytes()
        .to_vec();
    fieldset_struct_bytes.append(&mut struct_entry);
    write_field_segments(
        &mut fieldset_struct_bytes,
        field_objs,
        0,
        0,
        0,
        0,
        "".to_string(),
        "".to_string(),
    )?;
    fieldset_struct_bytes.append(&mut b"}\n".to_vec());

    Ok(fieldset_struct_bytes)
}

#[allow(clippy::too_many_arguments)]
fn write_field_segments(
    message_struct_bytes: &mut Vec<u8>,
    field_objs: Iter<'_, FieldParametersJson>,
    a_start: usize,
    a_end: usize,
    b_start: usize,
    b_end: usize,
    fieldset_a_name: String,
    fieldset_b_name: String,
) -> Result<(), String> {
    let mut polymorphic_params: Option<PolymorphicParameters> = None;
    let mut previous_field = "".to_string();
    let mut offset = 0;
    for (i, obj) in field_objs.enumerate() {
        if (a_start != 0) && (a_start == i) {
            let mut fieldset_tag = b"\t#[fieldset]\n".to_vec();
            let mut length_field_tag = format!("\t#[length_field = \"{}\"]\n", previous_field)
                .as_bytes()
                .to_vec();
            let mut field = format!("\tfieldset_a: Vec<{}>,\n", fieldset_a_name)
                .as_bytes()
                .to_vec();
            message_struct_bytes.append(&mut fieldset_tag);
            message_struct_bytes.append(&mut length_field_tag);
            message_struct_bytes.append(&mut field);
        } else if (b_start != 0) && (b_start == i) {
            let mut fieldset_tag = b"\t#[fieldset]\n".to_vec();
            let mut length_field_tag = format!("\t#[length_field = \"{}\"]\n", previous_field)
                .as_bytes()
                .to_vec();
            let mut field = format!("\tfieldset_a: Vec<{}>,\n", fieldset_b_name)
                .as_bytes()
                .to_vec();
            message_struct_bytes.append(&mut fieldset_tag);
            message_struct_bytes.append(&mut length_field_tag);
            message_struct_bytes.append(&mut field);
        } else if ((i < a_end) && (i > a_start)) || ((i < b_end) && (i > b_start)) {
            continue;
        } else {
            let field_name = &obj.name;
            let field_name_cleaned = clean_string_for_rust(field_name, Case::Snake);
            if previous_field != *"n_items" {
                previous_field = field_name_cleaned.clone();
            }
            let field = if obj.field_type == *"FIELDTYPE_LOOKUP" {
                polymorphic_params = Some(PolymorphicParameters {
                    key_field: field_name_cleaned,
                    lookup_name: obj.lookup.name.clone(),
                });
                Field::try_from(obj)?
            } else if obj.field_type == "KEY_VALUE" {
                if let Some(params) = polymorphic_params.take() {
                    Field::from_polymorphic_parameters(obj.name.clone(), params)
                } else {
                    panic!("encountered KEY_VALUE field without previous key field")
                }
            } else {
                Field::try_from(obj)?
            };

            if let FieldParameters::ReservedField(params) = &field.params {
                offset = params.size
            } else {
                writeln!(message_struct_bytes, "{}", field.into_field_string(offset))
                    .expect("failed to write field to struct");
                offset = 0;
            }
        }
    }
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=definitions.json");
    println!("cargo::rustc-check-cfg=cfg(generate_nmea_definitions)");

    let out_dir = std::env::var("OUT_DIR").expect("no OUT_DIR defined");

    let gen_path = format!("{}/nmea_gen", out_dir);

    create_dir_all(&gen_path).expect("failed to create nmea_gen directory");

    let enums_path = format!("{}/enums.rs", gen_path);

    let polymorphic_file_path = format!("{}/polymorphic_types.rs", gen_path);

    let messages_path = format!("{}/messages.rs", gen_path);

    let mut enums_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(true)
        .open(enums_path)
        .expect("could not create new enums.rs");

    enums_file
        .write_all("// AUTO-GENERATED CODE; DO NOT DELETE OR EDIT\n".as_bytes())
        .expect("failed to write warning statement");

    let mut polymorphic_types_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(true)
        .open(polymorphic_file_path)
        .expect("could not create new polymorphic_types.rs");

    polymorphic_types_file
        .write_all("// AUTO-GENERATED CODE; DO NOT DELETE OR EDIT\n".as_bytes())
        .expect("failed to write warning statement");

    let mut messages_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(true)
        .open(messages_path)
        .expect("could not create new messages.rs");

    messages_file
        .write_all("// AUTO-GENERATED CODE; DO NOT DELETE OR EDIT\n".as_bytes())
        .expect("failed to write warning statement");

    let file_path = "definitions.json";
    if !Path::new(file_path).exists() {
        println!("No definitions file, skipping auto-generation...");
        return;
    } else {
        println!("cargo:rustc-cfg=generate_nmea_definitions");
    }
    let file = File::open(file_path).expect("Failed to open file");
    let reader = BufReader::new(file);

    let data: Value =
        serde_json::from_reader(reader).expect("failed to parse JSON as initial Value");

    let mut object = match data {
        Value::Object(obj) => obj,
        _ => {
            panic!("failed to parse JSON as initial object");
        }
    };

    let enum_import_line_1 = "use crate::parse_helpers::enums::NmeaEnumeratedField;\n";
    let enum_import_line_2 = "use crate::define_nmea_enum;\n\n";

    enums_file
        .write_all(enum_import_line_1.as_bytes())
        .expect("failed to write import statement");
    enums_file
        .write_all(enum_import_line_2.as_bytes())
        .expect("failed to write import statement");

    let polymorphic_type_import_line =
        "use crate::{polymorphic_type, parse_helpers::parsers::*};\n";
    let polymorphic_type_import_line2 = "use super::enums::*;\n\n";

    polymorphic_types_file
        .write_all(polymorphic_type_import_line.as_bytes())
        .expect("failed to write import statement");

    polymorphic_types_file
        .write_all(polymorphic_type_import_line2.as_bytes())
        .expect("failed to write import statement");

    let messages_imports_1 = "use super::enums::*;\n#[allow(unused_imports)]\nuse micro_rdk_nmea_macros::{PgnMessageDerive, FieldsetDerive};\n";
    messages_file
        .write_all(messages_imports_1.as_bytes())
        .expect("failed to write import statement for messages.rs");

    let messages_imports_2 =
        "use crate::{parse_helpers::parsers::*, messages::message::Message, define_pgns};\n";
    messages_file
        .write_all(messages_imports_2.as_bytes())
        .expect("failed to write import statement for messages.rs");

    let messages_imports_3 = "use micro_rdk::{common::sensor::GenericReadingsResult, google::protobuf::{value::Kind, Struct, Value}};\n\n";
    messages_file
        .write_all(messages_imports_3.as_bytes())
        .expect("failed to write import statement for messages.rs");

    let lookups = match object.get("lookups") {
        Some(val) => match val {
            Value::Object(obj) => obj,
            _ => {
                panic!("value for 'lookups' key was improperly formatted");
            }
        },
        None => {
            panic!("missing 'lookups' key");
        }
    };

    write_basic_enums(lookups, &mut enums_file);

    let polymorphisms = match object.get("lookup_field_types") {
        Some(val) => match val {
            Value::Object(obj) => obj,
            _ => {
                panic!("value for 'lookup_field_types' key was improperly formatted");
            }
        },
        None => {
            panic!("missing 'lookup_field_types' key");
        }
    };

    write_polymorphic_types(polymorphisms, &mut enums_file, &mut polymorphic_types_file);

    let mut message_structs: Vec<Vec<u8>> = Vec::new();

    let mut pgn_struct_names: Vec<String> = Vec::new();

    let messages_value = object
        .remove("messages")
        .expect("JSON missing messages key");
    let message_formats: Vec<MessageJson> =
        serde_json::from_value(messages_value).expect("could not parse messages");
    for msg_json in message_formats {
        let pgn = msg_json.pgn;
        match msg_json.into_bytes() {
            Ok(Some((struct_name, struct_bytes))) => {
                pgn_struct_names.push(struct_name);
                message_structs.push(struct_bytes);
            }
            Err(err) => {
                let err_msg = format!(
                    "// Unable to parse format for PGN {}, error was '{}'\n\n",
                    pgn, err
                );
                message_structs.push(err_msg.into_bytes());
            }
            _ => {}
        };
    }

    for struct_bytes in message_structs.iter_mut() {
        if let Err(err) = messages_file.write_all(struct_bytes.as_mut_slice()) {
            panic!("failed to write message text: {:?}", err)
        }
    }

    messages_file
        .write_all("\n\ndefine_pgns!(\n".as_bytes())
        .expect("failed to write define_pgns macro");

    for (idx, name) in pgn_struct_names.iter().enumerate() {
        if idx == 0 {
            messages_file
                .write_all(format!("\t{}", name).as_bytes())
                .expect("failed to write define_pgns macro");
        } else {
            messages_file
                .write_all(format!(",\n\t{}", name).as_bytes())
                .expect("failed to write define_pgns macro");
        }
    }

    messages_file.write_all("\n);".as_bytes()).unwrap()
}
