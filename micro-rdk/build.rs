use regex::Regex;

fn main() {
    if Regex::new(r"\w+-esp3?2?s?\d?-espidf")
        .unwrap()
        .is_match(&std::env::var("TARGET").unwrap())
    {
        println!("cargo::rustc-check-cfg=cfg(esp32)");
        println!("cargo::rustc-check-cfg=cfg(esp32s2)");
        println!("cargo::rustc-check-cfg=cfg(esp32s3)");
        println!("cargo::rustc-check-cfg=cfg(esp32c2)");
        println!("cargo::rustc-check-cfg=cfg(esp32c3)");
        println!("cargo::rustc-check-cfg=cfg(esp32c6)");

        let cfg_args = embuild::build::CfgArgs::try_from_env("ESP_IDF_SVC").unwrap();
        cfg_args.output();
        cfg_args.propagate();

        let link_args = embuild::build::LinkArgs::try_from_env("ESP_IDF_SVC").unwrap();
        link_args.output();
        link_args.propagate();
    }
}
