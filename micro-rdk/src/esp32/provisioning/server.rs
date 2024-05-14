use bytes::Bytes;
use esp_idf_svc::wifi::{ClientConfiguration, Configuration};

use crate::{
    common::{
        app_client::{AppClientBuilder, AppClientConfig},
        conn::mdns::Mdns,
        grpc::ServerError,
        grpc_client::GrpcClient,
        provisioning::{
            server::{
                ProvisioningInfo, ProvisioningService, ProvisioningServiceBuilder,
                ProvisoningServer, WifiManager,
            },
            storage::{RobotCredentialStorage, WifiCredentialStorage},
        },
    },
    esp32::{
        conn::mdns::Esp32Mdns,
        exec::Esp32Executor,
        provisioning::wifi_provisioning::{esp32_get_wifi, Esp32WifiProvisioning},
        tcp::Esp32Stream,
        tls::{Esp32TLS, Esp32TLSServerConfig},
    },
};

use async_io::{Async, Timer};
use std::{
    ffi::CString,
    fmt::Debug,
    net::{Ipv4Addr, TcpListener, UdpSocket},
};

async fn dns_server() {
    let socket = async_io::Async::<UdpSocket>::bind(([0, 0, 0, 0], 53)).unwrap();
    loop {
        let mut buf = [0_u8; 512];
        let len = socket.recv_from(&mut buf).await.unwrap();
        let buf = Bytes::copy_from_slice(&buf[..len.0]);
        let mut ans = dns_message_parser::Dns::decode(buf);
        if let Ok(ref mut msg) = ans {
            if let Some(q) = msg.questions.first() {
                if q.domain_name.to_string().contains("viam.setup") {
                    let rr = dns_message_parser::rr::RR::A(dns_message_parser::rr::A {
                        domain_name: q.domain_name.clone(),
                        ttl: 3600,
                        ipv4_addr: Ipv4Addr::new(192, 168, 71, 1),
                    });

                    msg.answers.push(rr);
                    msg.flags.qr = true;

                    let buf = msg.encode().unwrap();
                    socket.send_to(&buf, len.1).await.unwrap();
                } else {
                    msg.flags.qr = true;
                    msg.flags.rcode = dns_message_parser::RCode::ServFail;
                }
            }
        }
        drop(ans);
    }
}
async fn accept_connections<S, Wifi>(
    listener: Async<TcpListener>,
    service: ProvisioningService<S, Wifi>,
    exec: Esp32Executor,
) where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    Wifi: WifiManager + 'static,
{
    // Annoyingly VIAM app creates a new HTTP2 connection for each provisioning request
    loop {
        let incoming = listener.accept().await;

        if let Ok((stream, _)) = incoming {
            // The provisioning server is exposed over unencrypted HTTP2
            let stream = Esp32Stream::LocalPlain(stream);
            let cloned_srv = service.clone();
            let cloned_exec = exec.clone();
            exec.spawn(async {
                if let Err(e) = ProvisoningServer::new(cloned_srv, cloned_exec, stream).await {
                    log::error!("provisioning error {:?}", e);
                }
            })
            .detach();
        } else {
            break;
        }
    }
}

pub(crate) async fn serve_provisioning_async<S>(
    ip: Ipv4Addr,
    exec: Esp32Executor,
    info: ProvisioningInfo,
    storage: S,
    last_error: Option<String>,
) -> Result<(AppClientConfig, Esp32TLSServerConfig), Box<dyn std::error::Error>>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
{
    let _ = Timer::after(std::time::Duration::from_millis(150)).await;

    // Start the WiFi in AP + STA mode
    let wifi = Esp32WifiProvisioning::new(storage.clone()).await.unwrap();

    // Provisioning relies on DNS query to find the IP of the server. Specifically it will
    // make a request for viam.setup. All other queries are answered failed to express the lack off
    // internet
    let dns_answerer = exec.spawn(dns_server());

    let hostname = format!(
        "provisioning-{}-{}",
        info.get_model(),
        info.get_manufacturer()
    );
    let mut mdns = Esp32Mdns::new(hostname)?;

    let srv = ProvisioningServiceBuilder::new()
        .with_provisioning_info(info)
        .with_wifi_manager(wifi);

    let srv = if let Some(error) = last_error {
        srv.with_last_error(error)
    } else {
        srv
    };

    let srv = srv.build(storage.clone());
    let listen = TcpListener::bind("0.0.0.0:4772")?; // VIAM app expects the server to be at 4772
    let listen: Async<TcpListener> = listen.try_into()?;

    let port = listen.get_ref().local_addr()?.port();

    mdns.add_service(
        "provisioning",
        "_rpc",
        "_tcp",
        port,
        &[("provisioning", "")],
    )?;

    let credential_ready = srv.get_credential_ready();

    let cloned_exec = exec.clone();

    let provisioning_server_task = exec.spawn(accept_connections(listen, srv, cloned_exec));

    // Future will complete when either robot credentials have been transmitted when WiFi provisioning is disabled
    // or when both robot credentials and WiFi credentials have been transmitted.
    // wait for provisioning completion
    credential_ready.await;

    dns_answerer.cancel().await;
    provisioning_server_task.cancel().await;
    // Test if the WiFi credentials are correct
    {
        let mut wifi = esp32_get_wifi()?.lock().await;
        let wifi_creds = storage.get_wifi_credentials()?;
        let conf = ClientConfiguration {
            ssid: wifi_creds.ssid.as_str().try_into().unwrap(),
            password: wifi_creds.pwd.as_str().try_into().unwrap(),
            ..Default::default()
        };
        wifi.set_configuration(&Configuration::Client(conf))?;
        wifi.start().await?;
        wifi.connect().await?;
    }

    //We are connected let's validate the robot's credentials

    let creds = storage.get_robot_credentials()?;

    let app_config = AppClientConfig::new(
        creds.robot_secret().to_owned(),
        creds.robot_id().to_owned(),
        ip,
        "".to_owned(),
    );

    let conn = Esp32Stream::TLSStream(Box::new(Esp32TLS::new_client().open_ssl_context(None)?));
    let client = GrpcClient::new(conn, exec.clone(), "https://app.viam.com:443").await?;
    let builder = AppClientBuilder::new(Box::new(client), app_config.clone());

    let mut client = builder.build().await?;
    let certs = client.get_certificates().await?;

    let serv_key = CString::new(certs.tls_private_key).unwrap();
    let serv_key_len = serv_key.as_bytes_with_nul().len() as u32;
    let serv_key: *const u8 = serv_key.into_raw() as *const u8;

    let tls_certs = CString::new(certs.tls_certificate)
        .unwrap()
        .into_bytes_with_nul();

    Ok((
        app_config,
        Esp32TLSServerConfig::new(tls_certs, serv_key, serv_key_len),
    ))
}
