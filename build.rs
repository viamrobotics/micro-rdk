fn main() -> anyhow::Result<()> {
    if std::env::var("TARGET").unwrap() == "xtensa-esp32-espidf" {
        embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
        embuild::build::LinkArgs::output_propagated("ESP_IDF")?;
        let cfg = embuild::build::CfgArgs::try_from_env("ESP_IDF")?;
        cfg.output();
    }

    Ok(())
}
