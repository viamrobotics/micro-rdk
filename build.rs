use anyhow::anyhow;

// Necessary because of this issue: https://github.com/rust-lang/cargo/issues/9641
fn main() -> anyhow::Result<()> {
    if std::env::var_os("IDF_PATH").is_none() {
        return Err(anyhow!("You need to run IDF's export.sh before building"));
    }
    if std::env::var_os("MINI_RDK_WIFI_SSID").is_none() {
        std::env::set_var("MINI_RDK_WIFI_SSID", "Viam-2G");
        println!("cargo:rustc-env=MINI_RDK_WIFI_SSID=Viam-2G");
    }
    if std::env::var_os("MINI_RDK_WIFI_PASSWORD").is_none() {
        return Err(anyhow!(
            "please set the password for WiFi {}",
            std::env::var_os("MINI_RDK_WIFI_SSID")
                .unwrap()
                .to_str()
                .unwrap()
        ));
    }
    embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
    embuild::build::LinkArgs::output_propagated("ESP_IDF")
}
