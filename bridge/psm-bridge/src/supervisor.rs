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

use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use serde::Serialize;
use thiserror::Error;

use crate::config::ServerProcessConfig;

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
/// `Arc`; all mutation goes through the inner `Mutex`.
pub struct Supervisor {
    config: RwLock<Option<ServerProcessConfig>>,
    running: Mutex<Option<Running>>,
}

/// The `GET /v1/server/status` payload.
#[derive(Serialize)]
pub struct ServerStatus {
    /// Whether `[server_process]` is configured at all.
    pub configured: bool,
    /// Whether a supervised server process is currently alive.
    pub running: bool,
    /// PID of the running process, if any.
    pub pid: Option<u32>,
    /// Seconds since the supervised process was started, if running.
    pub uptime_secs: Option<u64>,
}

impl Supervisor {
    pub fn new(config: Option<ServerProcessConfig>) -> Self {
        Self {
            config: RwLock::new(config),
            running: Mutex::new(None),
        }
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
    pub fn status(&self) -> ServerStatus {
        let configured = self.is_configured();
        let mut slot = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let pid = Self::live_pid(&mut slot);
        let uptime_secs = pid.and(slot.as_ref().map(|r| r.started.elapsed().as_secs()));
        ServerStatus {
            configured,
            running: pid.is_some(),
            pid,
            uptime_secs,
        }
    }

    /// Launch the configured server if it is not already running.
    pub fn start(&self) -> Result<ServerStatus, SupervisorError> {
        let cfg = self.cfg()?;
        let mut slot = self.running.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(pid) = Self::live_pid(&mut slot) {
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

    /// Force-stop the supervised server (kills its whole process tree).
    pub fn stop(&self) -> Result<ServerStatus, SupervisorError> {
        if !self.is_configured() {
            return Err(SupervisorError::NotConfigured);
        }
        let mut slot = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let pid = Self::live_pid(&mut slot).ok_or(SupervisorError::NotRunning)?;
        kill_tree(pid).map_err(SupervisorError::Stop)?;
        // Reap our handle so the slot is clean and no zombie remains.
        if let Some(mut run) = slot.take() {
            let _ = run.child.kill();
            let _ = run.child.wait();
        }
        drop(slot);
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
