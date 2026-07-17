//! Headless bridge runner for development and CI.
//!
//! Serves the same authenticated REST API as the GUI app, configured purely
//! from environment variables — no window, no `bridge.toml`, no persisted
//! state. Never ship this to server operators as the primary entry point;
//! it exists so the frontend and integration harnesses can stand up a real
//! bridge against a fixture save directory.
//!
//! ```text
//! PSM_SAVE_DIR   (required) directory containing Level.sav
//! PSM_TOKEN      (default "dev-token") Bearer token
//! PSM_PORT       (default 8213) 127.0.0.1 port
//! PSM_ALLOW_WRITES  set to "1" to enable the save-write endpoints
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use psm_bridge::server;
use psm_bridge::state::AppState;
use psm_bridge::supervisor::Supervisor;

#[tokio::main]
async fn main() {
    psm_bridge::install_quiet_panic_hook();
    let save_dir = std::env::var("PSM_SAVE_DIR").unwrap_or_else(|_| {
        eprintln!("PSM_SAVE_DIR is required (directory containing Level.sav)");
        std::process::exit(2);
    });
    let token = std::env::var("PSM_TOKEN").unwrap_or_else(|_| "dev-token".to_string());
    let port: u16 = std::env::var("PSM_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8213);
    let allow_writes = std::env::var("PSM_ALLOW_WRITES").is_ok_and(|v| v == "1");
    let settings_ini = std::env::var("PSM_SETTINGS_INI")
        .ok()
        .map(PathBuf::from)
        .or_else(|| psm_bridge::config::derive_settings_ini(&PathBuf::from(&save_dir)));

    let state = Arc::new(AppState::new(PathBuf::from(&save_dir)));
    let router = server::router(
        state,
        Arc::new(token),
        Arc::new(Supervisor::new(None)),
        allow_writes,
        settings_ini,
    );

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("bind {addr} failed: {e}");
            std::process::exit(1);
        });
    println!("psm-bridge-headless listening on {addr} (save_dir={save_dir}, writes={allow_writes})");
    if let Err(e) = axum::serve(listener, router).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
