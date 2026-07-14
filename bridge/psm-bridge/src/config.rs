//! Bridge server configuration.
//!
//! Resolves settings from (in descending priority) CLI flags, environment
//! variables, an optional `bridge.toml` file, and hard-coded defaults. The
//! auth token is generated automatically when not configured, and is never
//! logged or printed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Resolved bridge server configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub port: u16,
    pub token: String,
    pub save_dir: PathBuf,
    pub allow_writes: bool,
    /// Process-supervision config; `None` unless `[server_process]` (with an
    /// `exe`) is set, in which case the server-control endpoints are enabled.
    pub server_process: Option<ServerProcessConfig>,
}

/// How the supervisor launches PalServer.exe. Present only when the owner
/// opts in via `[server_process]` in bridge.toml.
#[derive(Debug, Clone)]
pub struct ServerProcessConfig {
    /// Path to the server executable (e.g. `.../PalServer.exe`).
    pub exe: PathBuf,
    /// Launch arguments passed to the executable.
    pub args: Vec<String>,
}

/// Errors that can occur while loading configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

const DEFAULT_BIND: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8213;
const DEFAULT_SAVE_DIR: &str = ".";
const PORT_ENV_VAR: &str = "PSM_BRIDGE_PORT";

/// Raw, partially-populated TOML shape. Every field is optional so that a
/// missing file, a missing section, or a missing key all fall back to
/// defaults during resolution.
#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    server: Option<RawServer>,
    auth: Option<RawAuth>,
    paths: Option<RawPaths>,
    safety: Option<RawSafety>,
    server_process: Option<RawServerProcess>,
}

#[derive(Debug, Default, Deserialize)]
struct RawServerProcess {
    exe: Option<String>,
    args: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawServer {
    bind: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Default, Deserialize)]
struct RawAuth {
    token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawPaths {
    save_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSafety {
    allow_writes: Option<bool>,
}

/// Load configuration for the bridge server.
///
/// Precedence for the port: `cli_port` > `PSM_BRIDGE_PORT` env var > the
/// `[server].port` value from `file_path` > the built-in default (8213).
///
/// `file_path` is read as TOML if it exists; if it does not exist, every
/// setting falls back to its default. If the resolved auth token is empty
/// (unset in the file), a random one is generated. The token is never
/// printed or logged by this function.
pub fn load(cli_port: Option<u16>, file_path: &Path) -> Result<Config, ConfigError> {
    let raw = read_raw_config(file_path)?;
    let env_port = std::env::var(PORT_ENV_VAR).ok();
    Ok(resolve(raw, cli_port, env_port))
}

fn read_raw_config(file_path: &Path) -> Result<RawConfig, ConfigError> {
    if !file_path.exists() {
        return Ok(RawConfig::default());
    }
    let contents = std::fs::read_to_string(file_path).map_err(|source| ConfigError::Io {
        path: file_path.to_path_buf(),
        source,
    })?;
    parse_raw_config(&contents, file_path)
}

fn parse_raw_config(contents: &str, file_path: &Path) -> Result<RawConfig, ConfigError> {
    toml::from_str(contents).map_err(|source| ConfigError::Parse {
        path: file_path.to_path_buf(),
        source: Box::new(source),
    })
}

fn resolve(raw: RawConfig, cli_port: Option<u16>, env_port: Option<String>) -> Config {
    let server = raw.server.unwrap_or_default();
    let auth = raw.auth.unwrap_or_default();
    let paths = raw.paths.unwrap_or_default();
    let safety = raw.safety.unwrap_or_default();

    // Enabled only when [server_process] provides an `exe`.
    let server_process = raw.server_process.and_then(|sp| {
        sp.exe.filter(|e| !e.is_empty()).map(|exe| ServerProcessConfig {
            exe: PathBuf::from(exe),
            args: sp.args.unwrap_or_default(),
        })
    });

    let port = cli_port
        .or_else(|| env_port.and_then(|v| v.parse::<u16>().ok()))
        .or(server.port)
        .unwrap_or(DEFAULT_PORT);

    let bind = server.bind.unwrap_or_else(|| DEFAULT_BIND.to_string());

    let token = match auth.token {
        Some(token) if !token.is_empty() => token,
        _ => generate_token(),
    };

    let save_dir = paths
        .save_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SAVE_DIR));

    let allow_writes = safety.allow_writes.unwrap_or(false);

    Config {
        bind,
        port,
        token,
        save_dir,
        allow_writes,
        server_process,
    }
}

/// Generate a random token of 64 hex characters (two concatenated UUIDv4s in
/// their simple/hyphen-free form), comfortably over the 32-char minimum.
fn generate_token() -> String {
    let a = uuid::Uuid::new_v4().simple().to_string();
    let b = uuid::Uuid::new_v4().simple().to_string();
    format!("{a}{b}")
}

/// Public helper for the GUI's "Generate token" button.
pub fn new_token() -> String {
    generate_token()
}

// --- Writing bridge.toml ----------------------------------------------------

/// Serializable mirror of the TOML file. Paths are normalized to forward
/// slashes so the written file is always valid TOML on Windows (a raw `\`
/// would otherwise be read as an escape on the next load).
#[derive(Serialize)]
struct WritableConfig {
    server: WritableServer,
    auth: WritableAuth,
    paths: WritablePaths,
    safety: WritableSafety,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_process: Option<WritableServerProcess>,
}

#[derive(Serialize)]
struct WritableServer {
    bind: String,
    port: u16,
}

#[derive(Serialize)]
struct WritableAuth {
    token: String,
}

#[derive(Serialize)]
struct WritablePaths {
    save_dir: String,
}

#[derive(Serialize)]
struct WritableSafety {
    allow_writes: bool,
}

#[derive(Serialize)]
struct WritableServerProcess {
    exe: String,
    args: Vec<String>,
}

/// Normalize a path to forward slashes (valid + clean in TOML on any OS).
fn norm_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Write `config` to `path` as TOML. Backslashes in paths are normalized to
/// forward slashes, so a file produced here never triggers the "invalid
/// escape" parse error that a hand-typed Windows path can.
pub fn write(config: &Config, path: &Path) -> Result<(), ConfigError> {
    let raw = WritableConfig {
        server: WritableServer {
            bind: config.bind.clone(),
            port: config.port,
        },
        auth: WritableAuth {
            token: config.token.clone(),
        },
        paths: WritablePaths {
            save_dir: norm_path(&config.save_dir),
        },
        safety: WritableSafety {
            allow_writes: config.allow_writes,
        },
        server_process: config.server_process.as_ref().map(|sp| WritableServerProcess {
            exe: norm_path(&sp.exe),
            args: sp.args.clone(),
        }),
    };
    let text = toml::to_string_pretty(&raw)?;
    std::fs::write(path, text).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    /// `PSM_BRIDGE_PORT` is process-global state. Rust runs `#[test]`
    /// functions in parallel on separate threads within the same process, so
    /// every test that calls `load` (which reads that env var) must hold
    /// this lock — otherwise a test that temporarily sets the var can leak
    /// into an unrelated test running concurrently. Only one test actually
    /// mutates the env var, but all of them must serialize against it.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Write `contents` to a fresh temp file and return its path. The caller
    /// is responsible for cleanup via `cleanup`.
    fn write_temp_toml(contents: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = format!(
            "{}-{}-{}.toml",
            uuid::Uuid::new_v4().simple(),
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!("psm-bridge-config-test-{unique}"));
        std::fs::write(&path, contents).expect("write temp config fixture");
        path
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn port_from_file_is_used_when_no_cli_or_env_override() {
        let _guard = lock_env();
        let path = write_temp_toml("[server]\nport = 9000\n");

        let config = load(None, &path).expect("load should succeed");

        cleanup(&path);
        assert_eq!(config.port, 9000);
    }

    #[test]
    fn cli_port_overrides_file_port() {
        let _guard = lock_env();
        let path = write_temp_toml("[server]\nport = 9000\n");

        let config = load(Some(7777), &path).expect("load should succeed");

        cleanup(&path);
        assert_eq!(config.port, 7777);
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        let _guard = lock_env();
        let path = std::env::temp_dir().join(format!(
            "psm-bridge-config-test-does-not-exist-{}.toml",
            uuid::Uuid::new_v4().simple()
        ));

        let config = load(None, &path).expect("load should succeed for a missing file");

        assert_eq!(config.bind, DEFAULT_BIND);
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.save_dir, PathBuf::from(DEFAULT_SAVE_DIR));
        assert!(!config.allow_writes);
        assert!(config.token.len() >= 32);
    }

    #[test]
    fn empty_token_in_file_generates_a_token_at_least_32_chars() {
        let _guard = lock_env();
        let path = write_temp_toml("[auth]\ntoken = \"\"\n");

        let config = load(None, &path).expect("load should succeed");

        cleanup(&path);
        assert!(!config.token.is_empty());
        assert!(config.token.len() >= 32);
    }

    #[test]
    fn non_empty_token_in_file_is_preserved() {
        let _guard = lock_env();
        let path = write_temp_toml("[auth]\ntoken = \"my-fixed-token\"\n");

        let config = load(None, &path).expect("load should succeed");

        cleanup(&path);
        assert_eq!(config.token, "my-fixed-token");
    }

    /// Env-var precedence is asserted in one test (rather than split across
    /// several) because `PSM_BRIDGE_PORT` is process-global state; running
    /// separate set/unset tests in parallel with other tests that touch the
    /// same variable would race. This is the only test in the module that
    /// touches `PSM_BRIDGE_PORT`.
    #[test]
    fn env_port_beats_file_but_cli_beats_env() {
        let _guard = lock_env();
        let path = write_temp_toml("[server]\nport = 9000\n");

        // This is the only test in the module that mutates PSM_BRIDGE_PORT,
        // and it always removes it before returning (see below). Holding
        // `_guard` for the duration keeps every other test (which also
        // reads this env var via `load`) from observing it mid-mutation.
        // SAFETY: serialized by ENV_GUARD above; no other test can be
        // reading or writing this env var concurrently.
        unsafe { std::env::set_var(PORT_ENV_VAR, "8500") };

        let env_wins = load(None, &path).expect("load should succeed");
        assert_eq!(env_wins.port, 8500, "env var should beat file value");

        let cli_wins = load(Some(7777), &path).expect("load should succeed");
        assert_eq!(cli_wins.port, 7777, "CLI should beat env var");

        // SAFETY: see above.
        unsafe { std::env::remove_var(PORT_ENV_VAR) };

        let file_wins_after_removal = load(None, &path).expect("load should succeed");
        assert_eq!(
            file_wins_after_removal.port, 9000,
            "file value applies once the env var is gone"
        );

        cleanup(&path);
    }

    #[test]
    fn invalid_toml_returns_parse_error() {
        let _guard = lock_env();
        let path = write_temp_toml("not valid toml [[[");

        let result = load(None, &path);

        cleanup(&path);
        assert!(matches!(result, Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn save_dir_and_allow_writes_are_read_from_file() {
        let _guard = lock_env();
        let path = write_temp_toml(
            "[paths]\nsave_dir = \"/tmp/saves\"\n\n[safety]\nallow_writes = true\n",
        );

        let config = load(None, &path).expect("load should succeed");

        cleanup(&path);
        assert_eq!(config.save_dir, PathBuf::from("/tmp/saves"));
        assert!(config.allow_writes);
    }
}
