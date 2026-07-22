//! psm-bridge: a native settings/control window (egui) plus a background,
//! re-bindable REST API for the Palworld Server Manager desktop app.
//!
//! The console is kept alongside the window for raw logs.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use psm_bridge::config;
use psm_bridge::gui::BridgeApp;
use psm_bridge::runtime::Runtime;
use psm_bridge::server;
use psm_bridge::state::AppState;

const CONFIG_FILE: &str = "bridge.toml";

fn main() {
    psm_bridge::install_quiet_panic_hook();
    // If a previous run was killed mid clock time-skip, snap the clock back to
    // true time before doing anything else.
    psm_bridge::time_skip::recover_if_needed();
    let config_path = PathBuf::from(CONFIG_FILE);
    let first_run = !config_path.exists();

    let config = match config::load(None, &config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\nCould not read {}:\n  {e}\n", config_path.display());
            eprintln!("Tip: use forward slashes in paths (C:/Palworld/... not C:\\Palworld\\...),");
            eprintln!("or delete {} and let the app recreate it.\n", config_path.display());
            eprint!("Press Enter to exit… ");
            let _ = std::io::stdout().flush();
            let _ = std::io::stdin().read_line(&mut String::new());
            std::process::exit(1);
        }
    };

    // First run: persist the generated defaults so the auth token is stable
    // across restarts (instead of regenerating every launch).
    if first_run {
        match config::write(&config, &config_path) {
            Ok(()) => println!("Created {} with a fresh auth token.", config_path.display()),
            Err(e) => eprintln!("Warning: couldn't write {}: {e}", config_path.display()),
        }
    }

    let runtime = Runtime::new(config, config_path);

    // Background API server: its own tokio runtime on a worker thread.
    let server_runtime = runtime.clone();
    std::thread::spawn(move || match tokio::runtime::Runtime::new() {
        Ok(tokio_rt) => tokio_rt.block_on(serve_loop(server_runtime)),
        Err(e) => server_runtime.log(format!("failed to start async runtime: {e}")),
    });

    // Native settings/control window on the main thread.
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([560.0, 680.0])
            .with_min_inner_size([460.0, 520.0])
            .with_title("PSM Bridge"),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "PSM Bridge",
        native_options,
        Box::new(|_cc| Ok(Box::new(BridgeApp::new(runtime)))),
    ) {
        eprintln!("GUI error: {e}");
    }
}

/// Serve the REST API, re-binding whenever settings change. Each iteration
/// reads the current bind/port/token/save-dir, builds a fresh router, and
/// serves until [`Runtime::reconfigure`] fires. The supervisor is shared, so a
/// running game process survives re-binds.
async fn serve_loop(runtime: Arc<Runtime>) {
    loop {
        let bind = runtime.bind();
        let port = runtime.port();
        let addr = format!("{bind}:{port}");

        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                runtime.set_bind_status(format!("Bind failed on {addr}: {e}"));
                runtime.log(format!("bind failed on {addr}: {e} — waiting for a settings change"));
                runtime.reconfigure.notified().await;
                continue;
            }
        };

        runtime.set_bind_status(format!("Listening on {addr}"));
        runtime.log(format!("API listening on {addr}"));

        let app = Arc::new(AppState::new(runtime.save_dir()));
        let token = Arc::new(runtime.token());
        // `allow_writes` is sampled per bind; toggling it in the GUI fires
        // `reconfigure`, which re-binds with the new value.
        // Explicit ini path, or a fallback derived from the CURRENT save dir —
        // computed here on each (re)bind so it always tracks save_dir.
        let settings_ini = runtime
            .settings_ini()
            .or_else(|| config::derive_settings_ini(&runtime.save_dir()));
        let router = server::router(
            app,
            token,
            runtime.supervisor.clone(),
            runtime.allow_writes(),
            runtime.allow_time_skip(),
            settings_ini,
        );

        let shutdown_rt = runtime.clone();
        let shutdown = async move { shutdown_rt.reconfigure.notified().await };

        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
        {
            runtime.log(format!("server error: {e}"));
        }
        // reconfigure fired → loop and re-bind with the new settings.
    }
}
