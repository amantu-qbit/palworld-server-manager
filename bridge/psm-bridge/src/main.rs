// Not yet wired into `main` — a later task bootstraps the server from this
// module's `load`. Suppressed here rather than leaving it unused so the
// module can be built and tested standalone in this task.
#[allow(dead_code)]
mod config;

fn main() {
    println!("psm-bridge");
}
