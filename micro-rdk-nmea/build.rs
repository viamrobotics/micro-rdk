use serde_json::Value;
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

fn main() {
    println!("cargo:rerun-if-changed=definitions.json");
    println!("cargo::rustc-check-cfg=cfg(generate_nmea_definitions)");

    let out_dir = std::env::var("OUT_DIR").expect("no OUT_DIR defined");

    let gen_path = format!("{}/nmea_gen", out_dir);

    create_dir_all(&gen_path).expect("failed to create nmea_gen directory");

    let enums_path = format!("{gen_path}/enums.rs");

    let mut enums_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(true)
        .open(enums_path)
        .expect("could not create new enums_gen.rs");

    enums_file
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
