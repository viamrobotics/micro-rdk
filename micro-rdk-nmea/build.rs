use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::Path;

use convert_case::{Case, Casing};

// This is to compute a valid name for a variable or struct/enum member. We
// want to remove special characters and introduce a default convention for
// when a name starts with a number.
fn clean_string_for_rust(input: &str, counter: usize) -> String {
    let filtered: String = input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                '_'
            }
        })
        .collect();
    if filtered
        .chars()
        .position(|c| (c.is_numeric() || (c == '_')))
        .map(|idx| idx == 0)
        .unwrap_or_default()
    {
        // We want to use a prefix with an alphabetical suffix (i.e. "UnformattableVariantA")
        // when the cleaned string still starts with a number. We map however many variants we've
        // seen so far to an ASCII code in range 65-90 (A-Z). If there are more than 25 variants
        // we want to use a second letter (i.e. "UnformattableVariantAB")
        let suffix = if counter <= 25 {
            ((counter + 65) as u8 as char).to_string()
        } else {
            // we assume that no enum has a number of variants > 25 * 25 = 625 (Note (GV) - I checked,
            // this is true so far)
            let first_letter_ascii = (((counter / 25) - 1) + 65) as u8;
            let second_letter_ascii = ((counter % 25) + 65) as u8;
            format!(
                "{}{}",
                first_letter_ascii as char, second_letter_ascii as char
            )
        };
        format!("UnformattableVariant{}", suffix)
    } else {
        filtered
    }
}

#[derive(Serialize, Deserialize)]
struct EnumValueTypeSettings {
    value: u32,
    field_type: String,
    lookup_name: String,
    lookup_size: usize,
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
}

struct NumberFieldParameters {
    num_type: SimplifiedNumberType,
    scale: f64,
    #[allow(dead_code)]
    unit: String,
    size: usize,
}

impl NumberFieldParameters {
    fn to_polymorphic_type_parameters(&self) -> (String, String) {
        let mut type_str = match self.num_type {
            SimplifiedNumberType::Int8 => "i8",
            SimplifiedNumberType::Uint8 => "u8",
            SimplifiedNumberType::Int16 => "i16",
            SimplifiedNumberType::Uint16 => "u16",
            SimplifiedNumberType::Int32 => "i32",
            SimplifiedNumberType::Uint32 => "u32",
            SimplifiedNumberType::Int64 => "i64",
            SimplifiedNumberType::Uint64 => "u64",
        };
        let reader_instance = if self.scale == 1.0 {
            format!("NumberField::<{}>::new({})?", type_str, self.size)
        } else {
            let res = if self.scale.fract() == 0.0 {
                format!(
                    "NumberFieldWithScale::<{}>::new({}, {:.1})?",
                    type_str, self.size, self.scale
                )
            } else {
                format!(
                    "NumberFieldWithScale::<{}>::new({}, {})?",
                    type_str, self.size, self.scale
                )
            };
            type_str = "f64";
            res
        };
        (reader_instance, type_str.to_string())
    }
}

impl TryFrom<&EnumValueTypeSettings> for NumberFieldParameters {
    type Error = String;
    fn try_from(value: &EnumValueTypeSettings) -> Result<NumberFieldParameters, Self::Error> {
        Ok(match value.field_type.as_str() {
            "INT8" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int8,
                scale: 1.0,
                unit: "".to_string(),
                size: 8,
            },
            "FIX8" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int8,
                scale: 1.0,
                unit: "".to_string(),
                size: 8,
            },
            "UINT8" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint8,
                scale: 1.0,
                unit: "".to_string(),
                size: 8,
            },
            "UFIX8" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint8,
                scale: 1.0,
                unit: "".to_string(),
                size: 8,
            },
            "INT16" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 1.0,
                unit: "".to_string(),
                size: 16,
            },
            "FIX16" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 1.0,
                unit: "".to_string(),
                size: 16,
            },
            "FIX16_1" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 0.1,
                unit: "".to_string(),
                size: 16,
            },
            "UINT16" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 1.0,
                unit: "".to_string(),
                size: 16,
            },
            "UFIX16" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 1.0,
                unit: "".to_string(),
                size: 16,
            },
            "UFIX16_3" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 0.001,
                unit: "".to_string(),
                size: 16,
            },
            "INT32" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int32,
                scale: 1.0,
                unit: "".to_string(),
                size: 32,
            },
            "FIX32" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int32,
                scale: 1.0,
                unit: "".to_string(),
                size: 32,
            },
            "UINT32" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint32,
                scale: 1.0,
                unit: "".to_string(),
                size: 32,
            },
            "UFIX32" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint32,
                scale: 1.0,
                unit: "".to_string(),
                size: 32,
            },
            "INT64" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int64,
                scale: 1.0,
                unit: "".to_string(),
                size: 64,
            },
            "FIX64" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int64,
                scale: 1.0,
                unit: "".to_string(),
                size: 64,
            },
            "UINT64" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint64,
                scale: 1.0,
                unit: "".to_string(),
                size: 64,
            },
            "UFIX64" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint64,
                scale: 1.0,
                unit: "".to_string(),
                size: 64,
            },
            "UFIX32_2" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint32,
                scale: 0.001,
                unit: "".to_string(),
                size: 32,
            },
            "FIX32_2" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int32,
                scale: 0.01,
                unit: "".to_string(),
                size: 32,
            },
            "ANGLE_FIX16" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 0.1,
                unit: "rad".to_string(),
                size: 16,
            },
            "ANGLE_UFIX16" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 0.1,
                unit: "rad".to_string(),
                size: 16,
            },
            "LENGTH_UFIX32_CM" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint32,
                scale: 0.01,
                unit: "m".to_string(),
                size: 32,
            },
            "PRESSURE_UFIX16_HPA" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 100.0,
                unit: "Pa".to_string(),
                size: 16,
            },
            "GEO_FIX32" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int32,
                scale: 1.0e-7,
                unit: "deg".to_string(),
                size: 32,
            },
            "SPEED_UFIX16_CM" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 0.01,
                unit: "m/s".to_string(),
                size: 32,
            },
            "SPEED_FIX16_CM" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 0.01,
                unit: "m/s".to_string(),
                size: 32,
            },
            "DISTANCE_FIX32_CM" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int32,
                scale: 0.01,
                unit: "m".to_string(),
                size: 32,
            },
            "DISTANCE_FIX16_CM" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 0.01,
                unit: "m".to_string(),
                size: 32,
            },
            "TEMPERATURE" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint16,
                scale: 0.01,
                unit: "K".to_string(),
                size: 16,
            },
            "PERCENTAGE_FIX16_D" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 0.1,
                unit: "%".to_string(),
                size: 16,
            },
            "TIME_FIX32_MS" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int32,
                scale: 0.001,
                unit: "sec".to_string(),
                size: 32,
            },
            "TIME_UFIX32_MS" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Uint32,
                scale: 0.001,
                unit: "sec".to_string(),
                size: 32,
            },
            "TIME_FIX16_MIN" => NumberFieldParameters {
                num_type: SimplifiedNumberType::Int16,
                scale: 60.0,
                unit: "sec".to_string(),
                size: 16,
            },
            x => {
                return Err(format!("encountered unsupported field type {}", x));
            }
        })
    }
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
        for (counter, (enum_str, num_val_val)) in value_map.iter().enumerate() {
            let enum_val =
                clean_string_for_rust(enum_str.clone().as_str(), counter).to_case(Case::Pascal);
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

        for (counter, (type_name, type_settings)) in types_unwrapped.iter().enumerate() {
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
            let variant_name = clean_string_for_rust(type_name.as_str(), counter)
                .as_str()
                .to_case(Case::Pascal);
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

fn main() {
    println!("cargo:rerun-if-changed=definitions.json");
    println!("cargo::rustc-check-cfg=cfg(generate_nmea_definitions)");

    let out_dir = std::env::var("OUT_DIR").expect("no OUT_DIR defined");

    let gen_path = format!("{}/nmea_gen", out_dir);

    create_dir_all(&gen_path).expect("failed to create nmea_gen directory");

    let enums_path = format!("{}/enums.rs", gen_path);

    let polymorphic_file_path = format!("{}/polymorphic_types.rs", gen_path);

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

    let object = match data {
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
}
