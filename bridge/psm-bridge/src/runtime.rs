//! Shared runtime state for the GUI (main thread) and the background API
//! server (a worker thread).
//!
//! The GUI edits settings live; the server thread reads them each time it
//! (re)binds and rebuilds its router, so bind/port/token/save-dir changes take
//! effect on the next bind — triggered immediately via [`Runtime::apply`],
//! which persists `bridge.toml` and notifies [`Runtime::reconfigure`]. The
//! [`Supervisor`] is the one piece that must persist across rebinds (it holds
//! the running game process), so it lives here behind an `Arc`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::Notify;

use crate::config::{self, Config};
use crate::supervisor::Supervisor;

/// Max log lines retained for the in-window log panel.
const LOG_CAP: usize = 300;

pub struct Runtime {
    pub supervisor: Arc<Supervisor>,
    /// Notified when the API server should re-bind (settings were saved).
    pub reconfigure: Notify,
    bind: RwLock<String>,
    port: RwLock<u16>,
    token: RwLock<String>,
    save_dir: RwLock<PathBuf>,
    allow_writes: RwLock<bool>,
    config_path: PathBuf,
    log: Mutex<VecDeque<String>>,
    bind_status: RwLock<String>,
}

impl Runtime {
    pub fn new(config: Config, config_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            supervisor: Arc::new(Supervisor::new(config.server_process.clone())),
            reconfigure: Notify::new(),
            bind: RwLock::new(config.bind),
            port: RwLock::new(config.port),
            token: RwLock::new(config.token),
            save_dir: RwLock::new(config.save_dir),
            allow_writes: RwLock::new(config.allow_writes),
            config_path,
            log: Mutex::new(VecDeque::new()),
            bind_status: RwLock::new("Starting…".to_string()),
        })
    }

    // --- reads used by the server thread on each (re)bind ---
    pub fn bind(&self) -> String {
        self.bind.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn port(&self) -> u16 {
        *self.port.read().unwrap_or_else(|p| p.into_inner())
    }
    pub fn token(&self) -> String {
        self.token.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn save_dir(&self) -> PathBuf {
        self.save_dir.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn allow_writes(&self) -> bool {
        *self.allow_writes.read().unwrap_or_else(|p| p.into_inner())
    }

    /// Snapshot of all current settings (for the form and for writing to disk).
    pub fn snapshot(&self) -> Config {
        Config {
            bind: self.bind(),
            port: self.port(),
            token: self.token(),
            save_dir: self.save_dir(),
            allow_writes: self.allow_writes(),
            server_process: self.supervisor.config_snapshot(),
        }
    }

    /// Apply new settings: update the live values, push `server_process` to the
    /// supervisor, persist `bridge.toml`, and signal the server to re-bind.
    pub fn apply(&self, new: Config) -> Result<(), String> {
        *self.bind.write().unwrap_or_else(|p| p.into_inner()) = new.bind.clone();
        *self.port.write().unwrap_or_else(|p| p.into_inner()) = new.port;
        *self.token.write().unwrap_or_else(|p| p.into_inner()) = new.token.clone();
        *self.save_dir.write().unwrap_or_else(|p| p.into_inner()) = new.save_dir.clone();
        *self.allow_writes.write().unwrap_or_else(|p| p.into_inner()) = new.allow_writes;
        self.supervisor.set_config(new.server_process.clone());
        config::write(&new, &self.config_path).map_err(|e| e.to_string())?;
        // notify_one stores a permit if the server is momentarily between binds,
        // so the re-bind is never missed (at worst one harmless extra re-bind).
        self.reconfigure.notify_one();
        Ok(())
    }

    // --- bind status (server thread → GUI) ---
    pub fn set_bind_status(&self, s: impl Into<String>) {
        *self.bind_status.write().unwrap_or_else(|p| p.into_inner()) = s.into();
    }
    pub fn bind_status(&self) -> String {
        self.bind_status.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    // --- log (mirrored to the console and the in-window panel) ---
    pub fn log(&self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("{msg}");
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        log.push_back(msg);
        while log.len() > LOG_CAP {
            log.pop_front();
        }
    }
    pub fn log_lines(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
}
