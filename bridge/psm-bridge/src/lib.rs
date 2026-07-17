//! psm-bridge: local HTTP bridge for the Palworld Server Manager desktop
//! app — read endpoints plus opt-in, backup-guarded save-write endpoints.

pub mod config;
pub mod gui;
pub mod runtime;
pub mod server;
pub mod settings_ini;
pub mod state;
pub mod supervisor;

/// Install a panic hook that silences the decoder's *speculative-probe*
/// panics only.
///
/// The GVAS decoder's egg-body classifier intentionally parse-and-catches: a
/// rejection panics inside a `catch_unwind` and is handled as a normal
/// negative result — several fire on every world decode, spamming operator
/// consoles. The probe marks its extent via
/// [`psm_save::save::containers::speculative_probe_active`], so this hook
/// suppresses exactly those panics and nothing else: a genuine decode failure
/// on a corrupt save still prints its full panic message (and still surfaces
/// as an HTTP error through the `catch_unwind` quarantines either way).
pub fn install_quiet_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if psm_save::save::containers::speculative_probe_active() {
            return;
        }
        default_hook(info);
    }));
}
