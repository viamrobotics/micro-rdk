#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl esp_idf_hal::peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> anyhow::Result<AsyncWifi<EspWifi<'static>>> {
    let mut wifi = EspWifi::new(modem, sl_stack.clone(), None)?;

    info!("scanning");
    let aps = wifi.scan()?;
    let foundap = aps.into_iter().find(|x| x.ssid == SSID);

    let channel = if let Some(foundap) = foundap {
        info!("{} channel is {}", "Viam", foundap.channel);
        Some(foundap.channel)
    } else {
        None
    };
    let client_config = embedded_svc::wifi::ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        channel,
        ..Default::default()
    };
    wifi.set_configuration(&embedded_svc::wifi::Configuration::Client(client_config))?;

    wifi.start()?;

    let timer = EspTimerService::new()?;
    let mut async_wifi = AsyncWifi::wrap(wifi, sl_stack, timer)?;
    block_on(async {
        async_wifi
            .wifi_wait(|| async_wifi.wifi().is_started(), Some(Duration::from_secs(20)))
            .await
            .expect("couldn't start wifi");
            async_wifi.connect().await.unwrap();
    });

    block_on(async {
        async_wifi
            .ip_wait_while(
                || Ok(async_wifi.wifi().is_connected().unwrap() && async_wifi.wifi().sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0,0,0,0)), 
                Some(Duration::from_secs(20)))
            .await
            .expect("wifi couldn't connect");
            async_wifi.connect().await.unwrap();
    });

    let ip_info = async_wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;

    Ok(async_wifi)
}

/*
#[derive(Deserialize, Debug)]
struct Response {
    response: String,
}
// webhook
use embedded_svc::{
    http::{client::Client as HttpClient, Method, Status},
    io::{Read, Write},
    utils::io,
    wifi::{ClientConfiguration, Wifi},
};
use serde::Deserialize;
use serde_json::json;

/// Send a HTTP GET request.
fn get_request(client: &mut HttpClient<EspHttpConnection>) -> anyhow::Result<()> {
    // Prepare headers and URL
    //let content_length_header = format!("{}", payload.len());

    let payload = json!({
        /*
        "location": "<ROBOT LOCATION>",
        "secret": "<ROBOT SECRET>",
        "target": "<COMPONENT BOARD NAME>",
        "pin": pin-no
        */
        "delete": "this"
    })
    .to_string();
    let payload = payload.as_bytes();
    // Prepare headers and URL
    let content_length_header = format!("{}", payload.len());
    let headers = [
        ("accept", "text/plain"),
        ("content-type", "application/json"),
        ("connection", "close"),
        ("content-length", &*content_length_header),
    ];
    let url = "https://restless-shape-1762.fly.dev/esp";

    // Send request
    //let mut request = client.get(&url, &headers)?;
    let mut request = client.request(Method::Get, &url, &headers)?;
    request.write_all(payload)?;
    request.flush()?;
    info!("-> GET {}", url);
    let mut response = request.submit()?;

    // Process response
    let status = response.status();
    info!("<- {}", status);
    let (_headers, mut body) = response.split();
    let mut buf = [0u8; 4096];
    let bytes_read = io::try_read_full(&mut body, &mut buf).map_err(|e| e.0)?;
    info!("Read {} bytes", bytes_read);
    let response: Response = serde_json::from_slice(&buf[0..bytes_read])?;
    info!("Response body: {:?} bytes", response);

    // Drain the remaining response bytes
    while body.read(&mut buf)? > 0 {}

    //let bytes_read = io::try_read_full(&mut body, &mut buf).map_err(|e| e.0)?;
    //info!("Read {} bytes", bytes_read);

    // Drain the remaining response bytes
    //while body.read(&mut buf)? > 0 {}

    Ok(())
}
*/
