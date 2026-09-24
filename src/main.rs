mod server;

use leptos::prelude::get_configuration;
use server::App;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use webdash_rs::Service;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listen_addr: SocketAddr = env::var("DASHBOARD_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:3000".into())
        .parse()?;
    let docker_socket = env::var("DOCKER_SOCKET").unwrap_or_else(|_| "/var/run/docker.sock".into());
    let extra_services: Vec<Service> =
        serde_json::from_str(&env::var("DASHBOARD_LINKS").unwrap_or_else(|_| "[]".into()))?;
    let leptos_options = get_configuration(None)?.leptos_options;

    let app = Arc::new(App::new(docker_socket, leptos_options, extra_services)?);
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, server::router(app)).await?;
    Ok(())
}
