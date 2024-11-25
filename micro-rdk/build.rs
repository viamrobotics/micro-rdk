use regex::Regex;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(esp32)");
    if Regex::new(r"\w+-esp3?2?s?\d?-espidf")
        .unwrap()
        .is_match(&std::env::var("TARGET").unwrap())
    {
        let cfg_args = embuild::build::CfgArgs::try_from_env("ESP_IDF_SVC").unwrap();
        cfg_args.output();
        cfg_args.propagate();

        let link_args = embuild::build::LinkArgs::try_from_env("ESP_IDF_SVC").unwrap();
        link_args.output();
        link_args.propagate();
    }
}
