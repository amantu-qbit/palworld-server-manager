//! psm-bridge: local HTTP bridge for the Palworld Server Manager desktop
//! app — read endpoints plus opt-in, backup-guarded save-write endpoints.

pub mod config;
pub mod gui;
pub mod runtime;
pub mod server;
pub mod state;
pub mod supervisor;

/// Install a panic hook that silences `Reader underrun` messages.
///
/// The GVAS decoder's speculative probes (e.g. the dynamic-item egg-body
/// classifier) intentionally parse-and-catch: a rejection panics inside a
/// `catch_unwind` and is handled as a normal negative result. Every decode
/// path in this crate runs inside such a quarantine, so these messages are
/// pure console noise for server operators — several fire on every world
/// decode. Real (non-speculative) underruns are still surfaced: the caught
/// panic becomes a `StateError::Decode`/HTTP error either way; only the
/// stderr spam is suppressed. All other panics print via the default hook.
pub fn install_quiet_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or("");
        if msg.starts_with("Reader underrun") {
            return;
        }
        default_hook(info);
    }));
}
