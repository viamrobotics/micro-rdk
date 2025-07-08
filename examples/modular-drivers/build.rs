use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use embuild::build;
use embuild::espidf::ulp_fsm::SystemIncludes;

fn find_ulp_sources<P: AsRef<Path>>(dir: P) -> io::Result<Vec<PathBuf>> {
    let mut ulp_sources = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "S" {
                    println!(
                        "cargo:rerun-if-changed={}",
                        path.as_os_str().to_str().unwrap()
                    );
                    ulp_sources.push(path);
                }
            }
        }
    }

    Ok(ulp_sources)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if Regex::new(r"\w+-esp3?2?s?\d?-espidf")
        .unwrap()
        .is_match(&std::env::var("TARGET").unwrap())
    {
        embuild::espidf::sysenv::output();
        embuild::espidf::sysenv::env_path();
        embuild::espidf::sysenv::idf_path();

        println!(
            "cargo::rustc-check-cfg=cfg(esp_idf_ulp_coproc_type_fsm, esp_idf_ulp_coproc_enabled)"
        );

        let esp_idf_env =
            PathBuf::from(std::env::var("DEP_MICRO_RDK_EMBUILD_ESP_IDF_PATH").unwrap());
        let sys_includes = SystemIncludes::CInclArgs(build::CInclArgs::try_from_env("MICRO_RDK")?);
        let env_path = std::env::var_os("DEP_MICRO_RDK_EMBUILD_ENV_PATH");

        let ulp_builder = embuild::espidf::ulp_fsm::Builder::new(
            esp_idf_env,
            sys_includes,
            vec![],
            None,
            env_path,
        );

        ulp_builder.build(
            find_ulp_sources("bme280-ulp")?.iter().map(PathBuf::as_path),
            embuild::cargo::out_dir(),
        )?;
    }
    Ok(())
}
