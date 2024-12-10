use axum::Router;
use local_ip_address::local_ip;
use std::net::SocketAddr;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const MICRO_RDK_OTA_BIN: &str = "micro-rdk-server-esp32-ota.bin";
const TARGET_DIR: &str = "../../target/xtensa-esp32-espidf";

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tokio::join!(serve(using_serve_dir(), 3001),);
}

fn using_serve_dir() -> Router {
    Router::new().nest_service(
        "/",
        ServeDir::new(TARGET_DIR),
    )
}

async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    // get private address
    let local = local_ip().unwrap();
    tracing::info!("serving ota partition: \n\n\thttp://{local}:{port}/{MICRO_RDK_OTA_BIN}");
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}
