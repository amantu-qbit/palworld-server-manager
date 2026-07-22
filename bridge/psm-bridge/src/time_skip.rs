//! Server-clock **time skip**: briefly jump the host system clock forward, hold
//! for a few seconds so the *running* Palworld server ticks its real-time timers
//! (egg hatching, cooldowns, time-gated missions) to completion, then restore.
//!
//! This is deliberately the most guarded thing the bridge does:
//!
//! - **Opt-in**: only runs when `[safety] allow_time_skip = true`.
//! - **Admin-only**: setting the system clock needs `SeSystemtimePrivilege`;
//!   without it the OS refuses and we report a clear "run as Administrator" error
//!   *before* the clock is ever touched.
//! - **Relative adjust**: we shift by `Set-Date -Adjust` (a *relative* offset), so
//!   restoring with the opposite offset lands back on true time automatically —
//!   the real seconds that elapsed during the hold are preserved, no absolute
//!   timestamp math, no timezone/DST edge cases.
//! - **Guaranteed restore**: a [`RestoreGuard`] restores on any early return or
//!   panic; the marker below covers a hard process kill.
//! - **Crash recovery**: a marker file is written for the duration of the skip.
//!   If the process is killed mid-window, [`recover_if_needed`] on the next start
//!   force-resyncs the clock from NTP and clears the marker.
//! - **Serialized**: one skip at a time ([`SKIP_LOCK`]).
//!
//! Windows only (the deployment target); other platforms return [`TimeSkipError::Unsupported`].

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;

/// How long the clock is held forward so the running server can process the
/// completions before we restore.
pub const HOLD: Duration = Duration::from_secs(10);

/// Only whole-hour jumps in this (small, safety-capped) range are allowed.
pub const MIN_HOURS: u32 = 1;
pub const MAX_HOURS: u32 = 4;

/// Serializes skips: a second request while one is mid-flight is rejected rather
/// than stacking clock adjustments.
static SKIP_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Error)]
pub enum TimeSkipError {
    #[error("hours must be between {MIN_HOURS} and {MAX_HOURS}")]
    BadHours,
    #[error("a time skip is already in progress")]
    InProgress,
    #[error("changing the system clock needs Administrator rights — run psm-bridge as Administrator")]
    NotElevated,
    #[error("failed to change the system clock: {0}")]
    SetClock(String),
    #[error("time skip is only supported on Windows")]
    Unsupported,
}

/// Outcome of a completed skip.
#[derive(Debug, Serialize)]
pub struct SkipReceipt {
    /// Hours the clock was jumped forward.
    pub hours: u32,
    /// Seconds the clock was held forward before restoring.
    pub held_secs: u64,
    /// True once the clock has been restored to true time.
    pub restored: bool,
}

/// Perform a forward-then-restore clock skip. Blocks for [`HOLD`]; call from a
/// blocking context (`spawn_blocking`).
pub fn skip(hours: u32) -> Result<SkipReceipt, TimeSkipError> {
    if !(MIN_HOURS..=MAX_HOURS).contains(&hours) {
        return Err(TimeSkipError::BadHours);
    }
    #[cfg(not(windows))]
    {
        let _ = hours;
        Err(TimeSkipError::Unsupported)
    }
    #[cfg(windows)]
    {
        windows_impl::skip(hours)
    }
}

/// On startup, if a marker shows a previous skip did not restore cleanly (the
/// process was killed mid-window), force the clock back to true time via NTP and
/// clear the marker. Safe to call unconditionally; a no-op when no marker exists.
pub fn recover_if_needed() {
    #[cfg(windows)]
    {
        windows_impl::recover_if_needed();
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::path::PathBuf;
    use std::process::Command;

    use super::{SkipReceipt, TimeSkipError, HOLD, MAX_HOURS, MIN_HOURS, SKIP_LOCK};

    fn marker_path() -> PathBuf {
        std::env::temp_dir().join("psm-bridge-timeskip.marker")
    }

    fn write_marker(hours: u32) {
        let _ = std::fs::write(marker_path(), hours.to_string());
    }

    /// Remove the crash-recovery marker. Absent is success (idempotent).
    fn remove_marker() -> std::io::Result<()> {
        match std::fs::remove_file(marker_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Shift the system clock by a **relative** whole-hour offset via
    /// `Set-Date -Adjust`. `$ErrorActionPreference='Stop'` makes a privilege
    /// failure a non-zero exit *before* any change, so a failed forward jump
    /// leaves the clock untouched.
    fn adjust_hours(hours: i64) -> Result<(), TimeSkipError> {
        let script = format!(
            "$ErrorActionPreference='Stop'; Set-Date -Adjust ([TimeSpan]::FromHours({hours})) | Out-Null"
        );
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|e| TimeSkipError::SetClock(e.to_string()))?;
        if out.status.success() {
            return Ok(());
        }
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = msg.trim();
        // "A required privilege is not held by the client" / "Access is denied".
        if msg.to_lowercase().contains("privilege") || msg.to_lowercase().contains("denied") {
            return Err(TimeSkipError::NotElevated);
        }
        Err(TimeSkipError::SetClock(if msg.is_empty() {
            format!("exit {:?}", out.status.code())
        } else {
            msg.to_string()
        }))
    }

    /// Best-effort NTP resync — snaps the clock to true time. Returns whether it
    /// actually succeeded (no network / no time service ⇒ `false`); never fails
    /// the caller. Used only as a fallback: the primary restore/recovery path is
    /// the network-free relative reverse.
    fn resync() -> bool {
        Command::new("w32tm")
            .args(["/resync", "/force"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Restores the clock on drop unless disarmed. The panic/early-return
    /// backstop for the happy-path restore; the marker is cleared **only** once
    /// a restore adjust has actually succeeded, so a still-wrong clock always
    /// leaves the marker for startup recovery.
    struct RestoreGuard {
        hours: i64,
        armed: bool,
    }

    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            if !self.armed {
                return;
            }
            match adjust_hours(-self.hours) {
                Ok(()) => {
                    let _ = resync();
                    let _ = remove_marker();
                }
                // Could not restore — leave the marker so the next startup
                // recovers (via the recorded offset). Still try a resync now as a
                // best effort.
                Err(_) => {
                    let _ = resync();
                }
            }
        }
    }

    pub fn skip(hours: u32) -> Result<SkipReceipt, TimeSkipError> {
        let _serialized = SKIP_LOCK.try_lock().map_err(|_| TimeSkipError::InProgress)?;

        // Marker first, so a kill in the tiny window before the jump still
        // triggers recovery.
        write_marker(hours);
        if let Err(e) = adjust_hours(hours as i64) {
            // A privilege denial fails *before* the clock moves, so it's
            // untouched — just clear the marker. Any other error leaves the clock
            // state unknown (e.g. the child was killed right after applying the
            // change), so best-effort resync in case it did move, then clear.
            if !matches!(e, TimeSkipError::NotElevated) {
                let _ = resync();
            }
            let _ = remove_marker();
            return Err(e);
        }

        // From here the clock is forward; it MUST be restored.
        let mut guard = RestoreGuard {
            hours: hours as i64,
            armed: true,
        };
        std::thread::sleep(HOLD);

        // Explicit restore; on failure the still-armed guard retries on drop.
        adjust_hours(-(hours as i64))?;
        guard.armed = false;
        let _ = resync();
        let _ = remove_marker();

        Ok(SkipReceipt {
            hours,
            held_secs: HOLD.as_secs(),
            restored: true,
        })
    }

    pub fn recover_if_needed() {
        let Ok(contents) = std::fs::read_to_string(marker_path()) else {
            return; // no interrupted skip (or an unreadable marker)
        };
        // The clock is almost certainly still forward by the recorded offset — the
        // marker outlives the forward jump and is cleared only on a clean restore.
        // A *relative* reverse by that offset is exact (the RTC kept ticking in
        // real time, so the +N skew is constant) AND needs no network, so it
        // recovers even on an offline/LAN host. NTP is only a fallback.
        if let Ok(hours) = contents.trim().parse::<i64>() {
            if (MIN_HOURS as i64..=MAX_HOURS as i64).contains(&hours)
                && adjust_hours(-hours).is_ok()
            {
                let _ = resync(); // best-effort snap to true time
                let _ = remove_marker();
                eprintln!(
                    "psm-bridge: recovered from an interrupted time skip — rolled the clock back {hours}h"
                );
                return;
            }
        }
        // Local reverse unavailable (unreadable/out-of-range offset, or setting
        // the clock is denied): fall back to NTP, and clear the marker ONLY if the
        // resync actually corrected the clock — otherwise leave it so a later
        // start retries instead of stranding a forwarded clock with no marker.
        if resync() {
            let _ = remove_marker();
            eprintln!("psm-bridge: recovered from an interrupted time skip — forced a clock resync");
        } else {
            eprintln!(
                "psm-bridge: interrupted time skip detected but the clock could not be corrected yet — will retry on next start"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_out_of_range_hours() {
        // These never touch the clock — validation happens before any OS call.
        assert!(matches!(skip(0), Err(TimeSkipError::BadHours)));
        assert!(matches!(skip(5), Err(TimeSkipError::BadHours)));
        assert!(matches!(skip(100), Err(TimeSkipError::BadHours)));
    }

    #[test]
    fn range_constants_cover_the_offered_offsets() {
        for h in [2u32, 3, 4] {
            assert!((MIN_HOURS..=MAX_HOURS).contains(&h));
        }
        assert_eq!(HOLD.as_secs(), 10);
    }
}
