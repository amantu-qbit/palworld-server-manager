//! psm-bridge binary: loads config, builds server state, and serves the
//! Bearer-authenticated HTTP API.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use psm_bridge::{config, server, state};

#[tokio::main]
async fn main() {
    let config = config::load(None, Path::new("bridge.toml")).expect("failed to load config");

    let addr: SocketAddr = format!("{}:{}", config.bind, config.port)
        .parse()
        .expect("configured bind address and port must form a valid socket address");

    let app_state = Arc::new(state::AppState::new(config.save_dir.clone()));
    let router = server::router(app_state, Arc::new(config.token.clone()));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind listener");

    println!("psm-bridge listening on {addr}");
    println!("auth token required for all requests");

    axum::serve(listener, router)
        .await
        .expect("server error");
}
