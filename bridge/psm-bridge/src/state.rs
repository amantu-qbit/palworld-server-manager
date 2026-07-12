//! Decoded-`World` cache.
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
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

use psm_save::save::decompress::SaveError;
use psm_save::save::load_world;
use psm_save::save::model::World;
use thiserror::Error;

/// Cache key: `(mtime_secs, size_bytes)` of `<save_dir>/Level.sav`. Either
/// component changing (a new write, a restore from backup, etc.)
/// invalidates the cached decode.
type CacheKey = (u64, u64);

/// A previously-decoded `World`, tagged with the file key it was decoded
/// from.
struct Cached {
    key: CacheKey,
    world: Arc<World>,
}

/// Shared server state: the save directory to read from, plus a
/// lazily-populated, mtime/size-invalidated decode cache.
pub struct AppState {
    save_dir: PathBuf,
    cached: RwLock<Option<Cached>>,
}

/// Errors from [`AppState::world`].
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
}

impl AppState {
    /// Build state pointed at `save_dir` (the directory containing
    /// `Level.sav`), with an empty cache.
    pub fn new(save_dir: PathBuf) -> Self {
        Self {
            save_dir,
            cached: RwLock::new(None),
        }
    }

    /// Return the decoded `World` for the current `Level.sav`.
    ///
    /// Stats `<save_dir>/Level.sav` for `(mtime, size)`; if that key matches
    /// what's cached, returns the cached `Arc` without touching the
    /// decoder. Otherwise decodes off-thread (see module docs for the
    /// panic-safety rationale), caches the result under the new key, and
    /// returns it.
    pub async fn world(&self) -> Result<Arc<World>, StateError> {
        let key = self.current_key()?;

        if let Some(world) = self.cached_if_fresh(key) {
            return Ok(world);
        }

        let world = decode_off_thread(self.save_dir.clone()).await?;

        let mut guard = self.cached.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(Cached {
            key,
            world: Arc::clone(&world),
        });
        Ok(world)
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

    /// Return the cached `World` iff its key matches `key` exactly.
    fn cached_if_fresh(&self, key: CacheKey) -> Option<Arc<World>> {
        let guard = self.cached.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .as_ref()
            .filter(|cached| cached.key == key)
            .map(|cached| Arc::clone(&cached.world))
    }

    /// The directory this state reads `Level.sav` (and future save files)
    /// from.
    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    /// Whether `<save_dir>/Level.sav` currently exists on disk. Cheap
    /// existence check (no decode) used for the `/v1/health` response.
    pub fn level_sav_exists(&self) -> bool {
        self.save_dir.join("Level.sav").is_file()
    }
}

/// Decode `<dir>/Level.sav` on a blocking-pool thread, catching any panic
/// from the decoder so it surfaces as [`StateError::Decode`] instead of
/// unwinding into the async runtime (which would abort the process).
async fn decode_off_thread(dir: PathBuf) -> Result<Arc<World>, StateError> {
    let outcome = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(AssertUnwindSafe(|| load_world(&dir)))
    })
    .await;

    match outcome {
        // Decoded cleanly.
        Ok(Ok(Ok(world))) => Ok(Arc::new(world)),
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
