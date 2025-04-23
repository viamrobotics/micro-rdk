pub(crate) fn terminate() -> ! {
    #[cfg(feature = "native")]
    std::process::exit(0);
    #[cfg(feature = "esp32")]
    unsafe {
        crate::esp32::esp_idf_svc::sys::esp_restart();
    }
}
