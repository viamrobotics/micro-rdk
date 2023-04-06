#[allow(dead_code)]
#[cfg(feature = "qemu")]
use esp_idf_svc::eth::*;
#[cfg(feature = "qemu")]
use esp_idf_svc::eth::{EspEth, EthWait};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::netif::{EspNetif, EspNetifWait};
use esp_idf_sys as _;

use anyhow::bail;
use futures_lite::future::block_on;
use log::*;
use micro_rdk::common::sctp::Sctp2;
use micro_rdk::esp32::exec::Esp32Executor;
use smol::io::AsyncReadExt;
use smol::net::UdpSocket;
use std::time::Duration;

use esp_idf_hal::prelude::Peripherals;

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    let periph = Peripherals::take().unwrap();
    log::info!("Hello world ");
    #[cfg(feature = "qemu")]
    let (ip, _eth) = {
        use std::net::Ipv4Addr;
        info!("creating eth object");
        let eth = eth_configure(
            &sys_loop_stack,
            Box::new(esp_idf_svc::eth::EspEth::wrap(EthDriver::new_openeth(
                periph.mac,
                sys_loop_stack.clone(),
            )?)?),
        )?;
        (Ipv4Addr::new(0, 0, 0, 0), eth)
    };
    {
        esp_idf_sys::esp!(unsafe {
            esp_idf_sys::esp_vfs_eventfd_register(&esp_idf_sys::esp_vfs_eventfd_config_t {
                max_fds: 5,
            })
        })?;
    }

    let exec = Esp32Executor::new();
    let cloned = exec.clone();
    log::info!("Starting sctp ");
    block_on(exec.run(async move { run_server(cloned).await }));
    Ok(())
}

async fn run_server(exec: Esp32Executor<'_>) {
    log::info!("Starting sctp ");
    let socket = UdpSocket::bind("0.0.0.0:63332").await.unwrap();

    let (c_tx, c_rx) = async_channel::unbounded();
    let mut srv = Box::new(Sctp2::new(socket, exec.clone(), c_tx));

    log::info!("awaiting conneciton ");

    srv.listen().await.unwrap();
    exec.spawn(async move {
        srv.run().await;
    })
    .detach();
    let mut c = c_rx.recv().await.unwrap();
    loop {
        let mut buf = [0; 512];
        log::info!("Reading from server");
        let ret = c.read(&mut buf).await;
        match ret {
            Err(e) => log::info!("error echoing {:?}", e),
            Ok(len) => {
                log::info!("Echoing from server");
                let buf = &buf[..len];
                //Timer::after(Duration::from_millis(100)).await;
                c.write(buf).await;
            }
        }
    }
}

#[cfg(feature = "qemu")]
fn eth_configure(
    sl_stack: &EspSystemEventLoop,
    mut eth: Box<EspEth<'static>>,
) -> anyhow::Result<Box<EspEth<'static>>> {
    use std::net::Ipv4Addr;

    eth.start()?;

    if !EthWait::new(eth.driver(), sl_stack)?
        .wait_with_timeout(Duration::from_secs(30), || eth.is_started().unwrap())
    {
        bail!("couldn't start eth driver")
    }

    if !EspNetifWait::new::<EspNetif>(eth.netif(), sl_stack)?
        .wait_with_timeout(Duration::from_secs(20), || {
            eth.netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        })
    {
        bail!("didn't get an ip")
    }
    let ip_info = eth.netif().get_ip_info()?;
    info!("ETH IP {:?}", ip_info);
    Ok(eth)
}
