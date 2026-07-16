//! Server process supervisor: launches, monitors, and force-stops the game
//! server (`PalServer.exe`).
//!
//! This is **process control, not save editing** — no save file is ever
//! written here. It is enabled only when the owner sets `[server_process]`
//! (with an `exe`) in bridge.toml; otherwise every operation returns
//! [`SupervisorError::NotConfigured`] and the desktop app shows a setup hint.
//!
//! Graceful shutdown (warn players + save) is handled by the desktop app via
//! Palworld's own REST `/shutdown`; the supervisor's `stop` is a **force**
//! stop that kills the whole process tree (`PalServer.exe` launches a child
//! `PalServer-Win64-Shipping.exe`, so killing only the parent would orphan the
//! real server). `restart` here is therefore a force restart; the app's
//! graceful restart orchestrates REST `/shutdown` → wait → `start`.

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use thiserror::Error;

use crate::config::ServerProcessConfig;

/// How long a by-name process scan stays fresh. The GUI polls status every
/// second and every health/write-guard request consults it too — without a
/// cache each of those spawns `tasklist` (twice).
const DETECT_TTL: Duration = Duration::from_secs(2);

/// Errors from supervisor operations.
#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("server process control is not configured (add a [server_process] section with an `exe` to bridge.toml)")]
    NotConfigured,
    #[error("the server is already running (pid {0})")]
    AlreadyRunning(u32),
    #[error("the server is not running")]
    NotRunning,
    #[error("failed to launch the server: {0}")]
    Launch(String),
    #[error("failed to stop the server: {0}")]
    Stop(String),
}

/// A launched child plus when it started (for uptime).
struct Running {
    child: Child,
    started: Instant,
}

/// Owns the (optionally) launched server process. Cheap to hold behind an
/// `Arc`; all mutation goes through the inner `Mutex`es.
pub struct Supervisor {
    config: RwLock<Option<ServerProcessConfig>>,
    running: Mutex<Option<Running>>,
    /// TTL cache of the by-name process scan (see [`DETECT_TTL`]).
    detected: Mutex<Option<(Instant, Vec<u32>)>>,
    /// OS-reported start time (Unix secs) per adopted PID, `None` cached for
    /// PIDs whose start time could not be read. Cleared when the PID dies.
    adopted_starts: Mutex<HashMap<u32, Option<u64>>>,
}

/// The `GET /v1/server/status` payload.
#[derive(Serialize)]
pub struct ServerStatus {
    /// Whether `[server_process]` is configured at all.
    pub configured: bool,
    /// Whether a server process is currently alive (launched by us or
    /// detected by image name).
    pub running: bool,
    /// PID of the running process, if any.
    pub pid: Option<u32>,
    /// Seconds since the server started, if known. For an adopted process
    /// this comes from the OS process table.
    pub uptime_secs: Option<u64>,
    /// True when the running server was NOT launched by this bridge instance
    /// (started manually, or the bridge was closed/updated and reopened) —
    /// detected by image name and fully controllable regardless.
    pub adopted: bool,
}

impl Supervisor {
    pub fn new(config: Option<ServerProcessConfig>) -> Self {
        Self {
            config: RwLock::new(config),
            running: Mutex::new(None),
            detected: Mutex::new(None),
            adopted_starts: Mutex::new(HashMap::new()),
        }
    }

    /// By-name process scan through the TTL cache. `fresh` bypasses (and
    /// refills) the cache — used by start/stop, where a stale answer could
    /// double-launch or report a just-killed server as alive.
    fn detected_pids(&self, fresh: bool) -> Vec<u32> {
        let mut cache = self.detected.lock().unwrap_or_else(|p| p.into_inner());
        if !fresh {
            if let Some((at, pids)) = cache.as_ref() {
                if at.elapsed() < DETECT_TTL {
                    return pids.clone();
                }
            }
        }
        let pids = find_server_pids();
        *cache = Some((Instant::now(), pids.clone()));
        pids
    }

    /// Uptime of an adopted process from its OS-reported start time, queried
    /// once per PID and cached. `None` when the start time can't be read.
    fn adopted_uptime(&self, pid: u32) -> Option<u64> {
        let mut starts = self.adopted_starts.lock().unwrap_or_else(|p| p.into_inner());
        // Drop entries for PIDs that are no longer the adopted server, so a
        // recycled PID can't inherit a stale start time.
        starts.retain(|p, _| *p == pid);
        let start = *starts.entry(pid).or_insert_with(|| process_start_epoch(pid));
        let start = start?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        Some(now.saturating_sub(start))
    }

    /// Replace the launch configuration; applied on the next `start`.
    pub fn set_config(&self, config: Option<ServerProcessConfig>) {
        *self.config.write().unwrap_or_else(|p| p.into_inner()) = config;
    }

    /// Whether a `[server_process]` is configured.
    pub fn is_configured(&self) -> bool {
        self.config.read().unwrap_or_else(|p| p.into_inner()).is_some()
    }

    /// A clone of the current launch configuration (for the settings form).
    pub fn config_snapshot(&self) -> Option<ServerProcessConfig> {
        self.config.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn cfg(&self) -> Result<ServerProcessConfig, SupervisorError> {
        self.config
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or(SupervisorError::NotConfigured)
    }

    /// Reap the tracked child if it has already exited, clearing the slot.
    /// Returns the pid if the process is still alive, `None` otherwise.
    fn live_pid(slot: &mut Option<Running>) -> Option<u32> {
        let run = slot.as_mut()?;
        // `Ok(None)` means still running; `Ok(Some(_))` (exited) or `Err(_)`
        // (can't determine) both mean the process is gone → clear the slot.
        if let Ok(None) = run.child.try_wait() {
            return Some(run.child.id());
        }
        *slot = None;
        None
    }

    /// Current status (reaps a dead child as a side effect).
    ///
    /// `PalServer.exe` is a thin launcher that spawns the real server
    /// (`PalServer-Win64-Shipping.exe`) and exits, so our tracked child dies
    /// almost immediately — and after a bridge restart/update there's no handle
    /// at all. So when the tracked child isn't alive, fall back to detecting the
    /// real server process by name; this also picks up a server started outside
    /// PSM. `uptime` is only known for a server we launched ourselves.
    pub fn status(&self) -> ServerStatus {
        let configured = self.is_configured();
        let mut slot = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let tracked = Self::live_pid(&mut slot);
        let tracked_uptime = tracked.and(slot.as_ref().map(|r| r.started.elapsed().as_secs()));
        drop(slot);

        let (running, pid, uptime_secs, adopted) = match tracked {
            Some(p) => (true, Some(p), tracked_uptime, false),
            None => match self.detected_pids(false).into_iter().next() {
                Some(p) => (true, Some(p), self.adopted_uptime(p), true),
                None => (false, None, None, false),
            },
        };
        ServerStatus {
            configured,
            running,
            pid,
            uptime_secs,
            adopted,
        }
    }

    /// Launch the configured server if it is not already running.
    pub fn start(&self) -> Result<ServerStatus, SupervisorError> {
        let cfg = self.cfg()?;
        let mut slot = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(pid) = Self::live_pid(&mut slot) {
            return Err(SupervisorError::AlreadyRunning(pid));
        }
        // Also refuse if a server is already running that we didn't launch (e.g.
        // started manually, or still alive from before a bridge restart) — a
        // second launch would fight over the game port. Fresh scan: a stale
        // cached "none" must not slip a double-launch through.
        if let Some(pid) = self.detected_pids(true).into_iter().next() {
            return Err(SupervisorError::AlreadyRunning(pid));
        }

        let mut cmd = Command::new(&cfg.exe);
        cmd.args(&cfg.args);
        // Launch from the executable's own directory so relative game paths resolve.
        if let Some(dir) = cfg.exe.parent() {
            if !dir.as_os_str().is_empty() {
                cmd.current_dir(dir);
            }
        }
        // Detach the server's stdio from the bridge console; the game writes to
        // its own log files under Pal/Saved/Logs regardless.
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: run the server without popping a console window.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| SupervisorError::Launch(e.to_string()))?;
        *slot = Some(Running {
            child,
            started: Instant::now(),
        });
        drop(slot);
        Ok(self.status())
    }

    /// Force-stop the server (kills its whole process tree). Works for a
    /// tracked child AND an adopted/detected server — stopping only needs a
    /// PID, so no `[server_process]` configuration is required (unlike
    /// `start`, which needs the exe path).
    pub fn stop(&self) -> Result<ServerStatus, SupervisorError> {
        let mut slot = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(pid) = Self::live_pid(&mut slot) {
            kill_tree(pid).map_err(SupervisorError::Stop)?;
            // Reap our handle so the slot is clean and no zombie remains.
            if let Some(mut run) = slot.take() {
                let _ = run.child.kill();
                let _ = run.child.wait();
            }
            drop(slot);
            *self.detected.lock().unwrap_or_else(|p| p.into_inner()) = None;
            return Ok(self.status());
        }
        drop(slot);

        // No tracked child: stop a detected/adopted server (the launcher already
        // exited, or the server was started outside PSM / before a restart).
        // Fresh scan, and the cache is cleared afterwards so status doesn't
        // report the just-killed server as alive for the cache TTL.
        let pids = self.detected_pids(true);
        if pids.is_empty() {
            return Err(SupervisorError::NotRunning);
        }
        for pid in pids {
            kill_tree(pid).map_err(SupervisorError::Stop)?;
        }
        *self.detected.lock().unwrap_or_else(|p| p.into_inner()) = None;
        self.adopted_starts.lock().unwrap_or_else(|p| p.into_inner()).clear();
        Ok(self.status())
    }

    /// Force restart: stop (if running), then start.
    pub fn restart(&self) -> Result<ServerStatus, SupervisorError> {
        match self.stop() {
            Ok(_) | Err(SupervisorError::NotRunning) => {}
            Err(e) => return Err(e),
        }
        self.start()
    }
}

/// Palworld's long-lived server process image names. `PalServer.exe` is only a
/// launcher that spawns `PalServer-Win64-Shipping.exe` and exits, so the
/// shipping exe is the real process to detect; the launcher is included for
/// setups where it stays resident.
#[cfg(windows)]
const SERVER_IMAGE_NAMES: &[&str] = &["PalServer-Win64-Shipping.exe", "PalServer.exe"];

/// PIDs of any running Palworld server process, by image name (Windows:
/// `tasklist`). This is what lets status/start/stop work for a server the
/// bridge didn't launch itself, or that outlived a bridge restart/update.
#[cfg(windows)]
fn find_server_pids() -> Vec<u32> {
    let mut pids = Vec::new();
    for name in SERVER_IMAGE_NAMES {
        if let Ok(out) = Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {name}"), "/FO", "CSV", "/NH"])
            .output()
        {
            pids.extend(parse_tasklist_pids(&String::from_utf8_lossy(&out.stdout)));
        }
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// OS-reported start time (Unix seconds) of `pid`, or `None` if unreadable
/// (process gone, access denied). Windows: one PowerShell query — callers
/// cache the result per PID, so this does not run per status poll.
#[cfg(windows)]
fn process_start_epoch(pid: u32) -> Option<u64> {
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-Process -Id {pid} -ErrorAction Stop).StartTime.ToUniversalTime()\
                 .Subtract([datetime]'1970-01-01').TotalSeconds"
            ),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .map(|s| s as u64)
}

#[cfg(not(windows))]
fn process_start_epoch(_pid: u32) -> Option<u64> {
    None
}

/// Parse PIDs from `tasklist /FO CSV /NH` output. Each data row is
/// `"Image","PID","Session","Session#","Mem"`; the "no tasks" info line has no
/// CSV PID field and is skipped.
#[cfg(windows)]
fn parse_tasklist_pids(csv: &str) -> Vec<u32> {
    csv.lines()
        .filter_map(|line| {
            let mut fields = line.split("\",\"");
            let _image = fields.next()?;
            fields.next()?.trim_matches('"').trim().parse::<u32>().ok()
        })
        .collect()
}

/// Non-Windows dev builds can't enumerate the server; detection is Windows-only
/// (the real deployment target).
#[cfg(not(windows))]
fn find_server_pids() -> Vec<u32> {
    Vec::new()
}

/// Kill a process and all of its descendants.
#[cfg(windows)]
fn kill_tree(pid: u32) -> Result<(), String> {
    let output = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    // taskkill exits non-zero (code 128) when the pid is already gone; a stop
    // that finds nothing to kill is a success (idempotent).
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code() == Some(128) || stderr.contains("not found") {
        Ok(())
    } else {
        Err(stderr.trim().to_string())
    }
}

/// Best-effort kill on non-Windows (dev builds only; the real target is Windows).
#[cfg(not(windows))]
fn kill_tree(pid: u32) -> Result<(), String> {
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::parse_tasklist_pids;

    #[test]
    fn parses_pids_from_tasklist_csv() {
        let csv = "\"PalServer-Win64-Shipping.exe\",\"12345\",\"Services\",\"0\",\"1,234,567 K\"\n\
                   \"PalServer-Win64-Shipping.exe\",\"6789\",\"Console\",\"1\",\"900,000 K\"";
        assert_eq!(parse_tasklist_pids(csv), vec![12345, 6789]);
    }

    #[test]
    fn ignores_no_match_info_line() {
        let csv = "INFO: No tasks are running which match the specified criteria.";
        assert!(parse_tasklist_pids(csv).is_empty());
    }
}
