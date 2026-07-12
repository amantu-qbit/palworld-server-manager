// Not yet wired into `main` — a later task bootstraps the server from this
// module's `load`. Suppressed here rather than leaving it unused so the
// module can be built and tested standalone in this task.
#[allow(dead_code)]
mod config;

// Not yet wired into `main` — a later task (axum app + auth) builds an
// `AppState` and serves from it. Suppressed here rather than leaving it
// unused so the module can be built and tested standalone in this task.
#[allow(dead_code)]
mod state;

fn main() {
    println!("psm-bridge");
}
