use bytes::Bytes;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};

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

use super::wifi_provisioning::Esp32WifiProvisioningBuilder;

pub(crate) async fn serve_provisioning_async<S, Wifi>(
    exec: Esp32Executor,
    info: ProvisioningInfo,
    storage: S,
    last_error: Option<Box<dyn std::error::Error>>,
    mut wifi_manager: Option<Wifi>,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: RobotCredentialStorage + WifiCredentialStorage + Clone + 'static,
    <S as RobotCredentialStorage>::Error: Debug,
    ServerError: From<<S as RobotCredentialStorage>::Error>,
    <S as WifiCredentialStorage>::Error: Sync + Send + 'static,
    Wifi: WifiManager + 'static,
{
    let _ = Timer::after(std::time::Duration::from_millis(150)).await;
    let hostname = format!(
        "provisioning-{}-{}",
        info.get_model(),
        info.get_manufacturer()
    );
    let srv = ProvisioningServiceBuilder::<Wifi>::new().with_provisioning_info(info);
    let mut dns_answerer = None;

    let srv = if let Some(wifi_manager) = wifi_manager.take() {
        // Provisioning relies on DNS query to find the IP of the server. Specifically it will
        // make a request for viam.setup. All other queries are answered failed to express the lack off
        // internet
        let _ = dns_answerer.insert(exec.spawn(dns_server((wifi_manager.get_ap_ip()))));
        srv.with_wifi_manager(wifi_manager)
    } else {
        srv
    };

    let mut mdns = Esp32Mdns::new(hostname)?;

    let srv = if let Some(error) = last_error {
        srv.with_last_error(error.to_string())
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

    if let Some(dns_answerer) = dns_answerer {
        dns_answerer.cancel().await;
    }
    provisioning_server_task.cancel().await;
    Ok(())
}
