use std::env;
fn main() {
    if env::var("TARGET").unwrap() == "xtensa-esp32-espidf" {
        if std::env::var_os("IDF_PATH").is_none() {
            panic!("You need to run IDF's export.sh before building");
        }
        embuild::build::CfgArgs::output_propagated("MICRO_RDK").unwrap();
        embuild::build::LinkArgs::output_propagated("MICRO_RDK").unwrap();
    }

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("Cargo.toml")
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let root_package_id = metadata
        .root_package()
        .ok_or(anyhow::anyhow!("Failed to get ID of root package"))
        .unwrap()
        .id
        .clone();

    let viam_modules: Vec<_> = metadata
        // Obtain the dependency graph from the metadata and iterate its nodes
        .resolve
        .as_ref()
        .ok_or(anyhow::anyhow!("Dependencies were not resolved"))
        .unwrap()
        .nodes
        .iter()
        // Until we find the root node..
        .find(|node| node.id == root_package_id)
        .ok_or(anyhow::anyhow!("Root package not found in dependencies"))
        .unwrap()
        // Then iterate the root node's dependencies, selecting only those
        // that are normal dependencies.
        .deps
        .iter()
        .filter(|dep| {
            dep.dep_kinds
                .iter()
                .any(|dk| dk.kind == cargo_metadata::DependencyKind::Normal)
        })
        // And which have a populated `package.metadata.com.viam` section in their Cargo.toml
        // which has `module = true`
        .filter(|dep| {
            metadata[&dep.pkg].metadata["com"]["viam"]["module"]
                .as_bool()
                .unwrap_or(false)
        })
        .collect();
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let mut modules_rs_content = String::new();
    let module_name_seq = viam_modules
        .iter()
        .map(|m| m.name.replace('-', "_"))
        .collect::<Vec<_>>()
        .join(", \n\t");
    modules_rs_content.push_str(&format!(
        "generate_register_modules!(\n\t{module_name_seq}\n);\n"
    ));
    let dest_path = std::path::Path::new(&out_dir).join("modules.rs");
    std::fs::write(dest_path, modules_rs_content).unwrap();
}
