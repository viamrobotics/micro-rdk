fn main() -> anyhow::Result<()> {
    if std::env::var("TARGET").unwrap() == "xtensa-esp32-espidf" {
        embuild::espidf::sysenv::output();
    }

    Ok(())
}
