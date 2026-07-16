//! Surgical save editing (Phase 2).
//!
//! The read pipeline is deliberately lossy (scalar type names widened,
//! per-property GUIDs dropped, `BTreeMap` ordering), so a parse→mutate→
//! re-serialize round trip could never be byte-faithful. This module edits the
//! **decompressed byte buffer directly** instead: a targeted locator finds the
//! exact spans to change ([`locate`]), a splice plan applies them with
//! automatic size/count fixups ([`plan`]), and every untouched byte survives
//! verbatim by construction. High-level operations live in [`ops`]; container
//! re-packing, backups, and atomic file replacement in [`write`].
//!
//! Safety ladder, applied to every edit:
//! 1. every declared size the edit depends on is validated against the walked
//!    bytes during location;
//! 2. the edited buffer is re-parsed with the strict production reader and the
//!    relevant domain decoder, and the expected change is asserted, before any
//!    bytes are returned;
//! 3. the original file is copied to a timestamped backup before replacement;
//! 4. the replacement itself is temp-file + rename.
//!
//! Callers (the psm-bridge HTTP layer) add the operational guards: the
//! `allow_writes` config gate and refusing to edit while the game server
//! process is running.

pub mod enc;
pub mod locate;
pub mod ops;
pub mod plan;
pub mod write;

use std::path::{Path, PathBuf};

use super::decompress::{decompress_sav_with_type, SaveError};

/// Receipt of a committed save edit.
#[derive(Debug, Clone)]
pub struct EditReceipt {
    /// Where the pre-edit file was backed up.
    pub backup: PathBuf,
    /// Size of the new `.sav` container written.
    pub bytes_written: usize,
}

/// Read `path`, run `op` over its decompressed GVAS bytes, and commit the
/// result: re-pack (always zlib `PlZ`, preserving the save-type byte), back up
/// the original, and atomically replace the file.
pub fn edit_sav_file<F>(path: &Path, op: F) -> Result<EditReceipt, SaveError>
where
    F: FnOnce(&[u8]) -> Result<Vec<u8>, SaveError>,
{
    let original = std::fs::read(path)
        .map_err(|e| SaveError::Io(format!("{}: {e}", path.display())))?;
    let (gvas, save_type) = decompress_sav_with_type(&original)?;
    let new_gvas = op(&gvas)?;
    let packed = write::pack_sav(&new_gvas, save_type)?;

    let backup = write::backup_file(path)?;
    write::atomic_replace(path, &packed)?;
    Ok(EditReceipt {
        backup,
        bytes_written: packed.len(),
    })
}
