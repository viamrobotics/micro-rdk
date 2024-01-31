fn main() -> anyhow::Result<()> {
    if std::env::var("TARGET").unwrap() == "xtensa-esp32-espidf" {
        let cfg_args = embuild::build::CfgArgs::try_from_env("ESP_IDF_SVC")?;
        cfg_args.output();
        cfg_args.propagate();

        let link_args = embuild::build::LinkArgs::try_from_env("ESP_IDF_SVC")?;
        link_args.output();
        link_args.propagate();
    }

    Ok(())
}
