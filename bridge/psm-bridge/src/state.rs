//! Decoded-save (`WorldBundle`) cache.
//!
//! Decoding `Level.sav` is CPU-bound and, per the Phase-1a decoder ledger,
//! not yet fully panic-free on malformed input (a known-deferred hardening
//! item: some inner reparse paths still use panic-on-underrun rather than a
//! fallible `Result`). This module is the boundary that makes that safe to
//! expose over a network API:
//!
//! - Every decode runs on a blocking-pool thread via
//!   [`tokio::task::spawn_blocking`], so it never stalls the async
//!   executor.
//! - Every decode is wrapped in [`std::panic::catch_unwind`], so a decoder
//!   panic on a crafted/corrupt save becomes a [`StateError::Decode`]
//!   result, never a crashed process.
//! - Results are cached by `(mtime, size)` of `Level.sav`, so repeated reads
//!   of an unchanged save are free (no re-decode, no re-allocation).

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

use psm_save::save::containers::{read_player_save, PlayerSave};
use psm_save::save::decompress::SaveError;
use psm_save::save::load_world_with_containers;
use psm_save::save::WorldBundle;
use thiserror::Error;

/// Cache key: `(mtime_secs, size_bytes)` of `<save_dir>/Level.sav`. Either
/// component changing (a new write, a restore from backup, etc.)
/// invalidates the cached decode.
type CacheKey = (u64, u64);

/// A previously-decoded [`WorldBundle`], tagged with the file key it was
/// decoded from.
struct Cached {
    key: CacheKey,
    bundle: Arc<WorldBundle>,
}

/// Shared server state: the save directory to read from, plus a
/// lazily-populated, mtime/size-invalidated decode cache.
pub struct AppState {
    save_dir: PathBuf,
    cached: RwLock<Option<Cached>>,
    /// Set once the one-shot full save-folder snapshot has been taken this
    /// bridge session (see [`AppState::ensure_full_backup`]).
    full_backup_done: AtomicBool,
}

/// Errors from [`AppState::bundle`].
#[derive(Debug, Error)]
pub enum StateError {
    /// Failed to stat (or read the mtime of) `<save_dir>/Level.sav`.
    #[error("failed to stat save file: {0}")]
    Io(#[from] std::io::Error),
    /// The decoder panicked while parsing a malformed save file. Caught via
    /// `catch_unwind` on a `spawn_blocking` task, so this is a `Result`
    /// error path — the process itself never crashes.
    #[error("save decoder panicked while parsing a malformed save file")]
    Decode,
    /// The decoder ran to completion but reported a normal error (e.g.
    /// `SaveError::TooLarge`, `SaveError::BadMagic`).
    #[error("failed to load world: {0}")]
    Load(#[from] SaveError),
    /// The one-shot full save-folder snapshot could not be written, so the
    /// edit was refused — no edit proceeds without a full pre-edit backup.
    #[error("full pre-edit backup failed: {0}")]
    Backup(String),
}

impl AppState {
    /// Build state pointed at `save_dir` (the directory containing
    /// `Level.sav`), with an empty cache.
    pub fn new(save_dir: PathBuf) -> Self {
        Self {
            save_dir,
            cached: RwLock::new(None),
            full_backup_done: AtomicBool::new(false),
        }
    }

    /// Take the one-shot full save-folder snapshot for this bridge session, if
    /// it hasn't been taken yet. Called before the first save edit (see the
    /// bridge's `run_edit`), on top of the per-file backup every edit makes.
    ///
    /// Returns the snapshot path on the run that creates it, `Ok(None)` when it
    /// was already taken this session, and an error (which aborts the edit) if
    /// the snapshot cannot be written — so no edit ever proceeds without a full
    /// pre-edit backup existing. Serialized by the caller's write lock, so the
    /// snapshot runs exactly once even under concurrent edit requests.
    pub async fn ensure_full_backup(&self) -> Result<Option<PathBuf>, StateError> {
        if self.full_backup_done.load(Ordering::Acquire) {
            return Ok(None);
        }
        let dir = self.save_dir.clone();
        let outcome =
            tokio::task::spawn_blocking(move || psm_save::save::edit::write::snapshot_save_dir(&dir))
                .await;
        match outcome {
            Ok(Ok(path)) => {
                // Only mark done once the snapshot actually exists, so a failed
                // attempt is retried on the next edit rather than skipped.
                self.full_backup_done.store(true, Ordering::Release);
                Ok(Some(path))
            }
            Ok(Err(save_error)) => Err(StateError::Backup(save_error.to_string())),
            Err(_join_error) => Err(StateError::Backup("snapshot task failed".to_string())),
        }
    }

    /// Return the decoded [`WorldBundle`] (world + item containers + dynamic
    /// items) for the current `Level.sav`.
    ///
    /// Stats `<save_dir>/Level.sav` for `(mtime, size)`; if that key matches
    /// what's cached, returns the cached `Arc` without touching the
    /// decoder. Otherwise decodes off-thread (see module docs for the
    /// panic-safety rationale), caches the result under the new key, and
    /// returns it.
    pub async fn bundle(&self) -> Result<Arc<WorldBundle>, StateError> {
        let key = self.current_key()?;

        if let Some(bundle) = self.cached_if_fresh(key) {
            return Ok(bundle);
        }

        let bundle = decode_off_thread(self.save_dir.clone()).await?;

        let mut guard = self.cached.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(Cached {
            key,
            bundle: Arc::clone(&bundle),
        });
        Ok(bundle)
    }

    /// Read `<save_dir>/Level.sav`'s (mtime seconds since the Unix epoch,
    /// size in bytes) as the cache key.
    fn current_key(&self) -> Result<CacheKey, StateError> {
        let level_path = self.save_dir.join("Level.sav");
        let meta = std::fs::metadata(&level_path)?;
        let mtime = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok((mtime, meta.len()))
    }

    /// Return the cached [`WorldBundle`] iff its key matches `key` exactly.
    fn cached_if_fresh(&self, key: CacheKey) -> Option<Arc<WorldBundle>> {
        let guard = self.cached.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .as_ref()
            .filter(|cached| cached.key == key)
            .map(|cached| Arc::clone(&cached.bundle))
    }

    /// The directory this state reads `Level.sav` (and future save files)
    /// from.
    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    /// Drop the cached decode unconditionally. Called after a save write:
    /// the `(mtime, size)` key normally self-invalidates, but mtime has
    /// whole-second granularity, so a write landing in the same second as
    /// the previous one with an identical byte size would otherwise serve
    /// stale pre-edit state indefinitely.
    pub fn invalidate(&self) {
        let mut guard = self.cached.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
    }

    /// Whether `<save_dir>/Level.sav` currently exists on disk. Cheap
    /// existence check (no decode) used for the `/v1/health` response.
    pub fn level_sav_exists(&self) -> bool {
        self.save_dir.join("Level.sav").is_file()
    }

    /// Read a player's five inventory + two character container ids from
    /// `<save_dir>/Players/<UPPERCASE-UID-NO-DASHES>.sav`.
    ///
    /// Runs on a blocking-pool thread with the same `catch_unwind` panic
    /// boundary as [`AppState::bundle`] (the underlying GVAS property reader
    /// panics on a malformed/crafted buffer by contract — see
    /// `psm_save::save::reader`'s doc comment), so a corrupt per-player save
    /// file cannot crash the server either. Not cached: per-player `.sav`
    /// files are small and read on demand.
    pub async fn player_save(&self, uid: &str) -> Result<PlayerSave, StateError> {
        let sav_path = self.save_dir.join("Players").join(player_sav_filename(uid));

        let outcome = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(AssertUnwindSafe(|| read_player_save(&sav_path)))
        })
        .await;

        match outcome {
            Ok(Ok(Ok(save))) => Ok(save),
            Ok(Ok(Err(save_error))) => Err(StateError::Load(save_error)),
            Ok(Err(_panic_payload)) => Err(StateError::Decode),
            Err(_join_error) => Err(StateError::Decode),
        }
    }
}

/// The per-player `.sav` filename Palworld uses for a given uid: the
/// canonical hyphenated uid with the dashes stripped and the hex uppercased,
/// e.g. `8c2f1930-0000-0000-0000-000000000000` ->
/// `8C2F1930000000000000000000000000.sav`. Lowercase orphans records on
/// Linux dedicated servers, so this must always uppercase.
fn player_sav_filename(uid: &str) -> String {
    format!("{}.sav", uid.replace('-', "").to_uppercase())
}

/// Decode `<dir>/Level.sav` (plus its item containers/dynamic items) on a
/// blocking-pool thread, catching any panic from the decoder so it surfaces
/// as [`StateError::Decode`] instead of unwinding into the async runtime
/// (which would abort the process).
async fn decode_off_thread(dir: PathBuf) -> Result<Arc<WorldBundle>, StateError> {
    let outcome = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(AssertUnwindSafe(|| load_world_with_containers(&dir)))
    })
    .await;

    match outcome {
        // Decoded cleanly.
        Ok(Ok(Ok(bundle))) => Ok(Arc::new(bundle)),
        // Decoder ran to completion but reported a normal error.
        Ok(Ok(Err(save_error))) => Err(StateError::Load(save_error)),
        // Decoder panicked — caught by `catch_unwind`, never reaches the caller as a panic.
        Ok(Err(_panic_payload)) => Err(StateError::Decode),
        // The blocking task itself was cancelled/aborted (not expected here,
        // since we never abort it), or panicked in a way `catch_unwind`
        // inside the closure could not intercept (also not expected, since
        // the closure catches its own panics). Treat as a decode failure
        // rather than propagating a `JoinError`, keeping `StateError`'s
        // surface small and consistently non-crashing.
        Err(_join_error) => Err(StateError::Decode),
    }
}
