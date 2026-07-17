//! Re-pack edited GVAS bytes into a `.sav` container and write it to disk
//! safely: timestamped backup first, then an atomic same-directory
//! temp-file-and-rename replace.

use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::ZlibEncoder;
use flate2::Compression;

use super::super::decompress::SaveError;

fn io_err(e: std::io::Error, what: &str) -> SaveError {
    SaveError::Io(format!("{what}: {e}"))
}

/// Pack raw GVAS bytes into a `.sav` container, always as zlib (`PlZ`).
///
/// Mirrors `palsav.py::compress_gvas_to_sav`: `compressed_len` is the length
/// after the FIRST compression stage even for double-zlib (`0x32`), where the
/// on-disk body is the twice-compressed stream. Oodle (`PlM`) inputs must be
/// re-emitted as `PlZ` (there is no Oodle compressor in the dependency tree);
/// the game loads both, and every save written by the palworld-save-tools
/// ecosystem is `PlZ`. Save types: `0x30` = raw body, `0x31` = single zlib,
/// `0x32` = double zlib.
pub fn pack_sav(gvas: &[u8], save_type: u8) -> Result<Vec<u8>, SaveError> {
    let (body, compressed_len) = match save_type {
        0x30 => (gvas.to_vec(), gvas.len()),
        0x31 => {
            let once = zlib_compress(gvas)?;
            let len = once.len();
            (once, len)
        }
        0x32 => {
            let once = zlib_compress(gvas)?;
            let len = once.len();
            (zlib_compress(&once)?, len)
        }
        other => return Err(SaveError::UnknownSaveType(other)),
    };

    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(&(gvas.len() as u32).to_le_bytes());
    out.extend_from_slice(&(compressed_len as u32).to_le_bytes());
    out.extend_from_slice(b"PlZ");
    out.push(save_type);
    out.extend_from_slice(&body);
    Ok(out)
}

fn zlib_compress(data: &[u8]) -> Result<Vec<u8>, SaveError> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data)
        .and_then(|_| enc.finish())
        .map_err(|e| SaveError::Zlib(e.to_string()))
}

/// How many timestamped backups to keep per file name.
const BACKUP_KEEP: usize = 20;

/// Copy `path` into `<parent>/psm-backups/<stem>.<UTC timestamp>.sav` before
/// it is overwritten, pruning old backups beyond [`BACKUP_KEEP`] per stem.
/// Returns the backup path. The `psm-backups` directory name is not a save
/// file, so the game ignores it.
pub fn backup_file(path: &Path) -> Result<PathBuf, SaveError> {
    let parent = path
        .parent()
        .ok_or_else(|| SaveError::Io(format!("{}: no parent dir", path.display())))?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| SaveError::Io(format!("{}: no file stem", path.display())))?;

    let dir = parent.join("psm-backups");
    std::fs::create_dir_all(&dir).map_err(|e| io_err(e, "create backup dir"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| SaveError::Io(format!("clock error: {e}")))?;
    // UTC compact stamp with millis: unique per write, sortable by name.
    let backup = dir.join(format!("{stem}.{}.{:03}.sav", now.as_secs(), now.subsec_millis()));
    std::fs::copy(path, &backup).map_err(|e| io_err(e, "copy backup"))?;

    prune_backups(&dir, stem);
    Ok(backup)
}

/// Best-effort prune: keep the newest [`BACKUP_KEEP`] `<stem>.*.sav` backups.
/// Failures are ignored — pruning must never block a save write.
fn prune_backups(dir: &Path, stem: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let prefix = format!("{stem}.");
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(&prefix) && n.ends_with(".sav"))
        .collect();
    // Timestamped names sort chronologically; oldest first.
    names.sort();
    if names.len() > BACKUP_KEEP {
        let excess = names.len() - BACKUP_KEEP;
        for n in &names[..excess] {
            let _ = std::fs::remove_file(dir.join(n));
        }
    }
}

/// List existing backups for `path` (newest first): `(backup_path, size)`.
pub fn list_backups(path: &Path) -> Vec<(PathBuf, u64)> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let dir = parent.join("psm-backups");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let prefix = format!("{stem}.");
    let mut out: Vec<(PathBuf, u64)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(&prefix) && n.ends_with(".sav"))
                .unwrap_or(false)
        })
        .map(|e| {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            (e.path(), size)
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

/// How many one-shot full-folder snapshots to keep.
const FULL_SNAPSHOT_KEEP: usize = 5;

/// Take a one-shot **full snapshot of the entire save directory** into
/// `<save_dir>/psm-backups/full-<UTC secs>.<millis>/`, recursively copying every
/// file and subdirectory except the `psm-backups` folder itself (so the
/// snapshot never contains prior backups or recurses into its own destination).
/// Older snapshots beyond [`FULL_SNAPSHOT_KEEP`] are pruned.
///
/// This is the belt-and-suspenders companion to the per-file [`backup_file`]:
/// callers take it once, before the first edit of a session, so the whole
/// pre-edit world (Level.sav + every player `.sav` + metadata) can be restored
/// wholesale even if a per-file backup is somehow missed.
pub fn snapshot_save_dir(save_dir: &Path) -> Result<PathBuf, SaveError> {
    let backups = save_dir.join("psm-backups");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| SaveError::Io(format!("clock error: {e}")))?;
    let final_name = format!("full-{}.{:03}", now.as_secs(), now.subsec_millis());
    let dest = backups.join(&final_name);

    // Build the snapshot in a hidden staging dir and rename it to `full-<ts>/`
    // only once the copy FULLY succeeds — so a `full-*` directory is always a
    // complete snapshot. A copy that fails partway (disk full, a
    // permission-denied file) leaves no `full-*` artifact and is not counted by
    // the keep-window prune; the edit is refused upstream.
    let staging = backups.join(format!(".{final_name}.partial"));
    let _ = std::fs::remove_dir_all(&staging); // clear any leftover from a crash
    std::fs::create_dir_all(&staging).map_err(|e| io_err(e, "create full-backup staging dir"))?;

    if let Err(e) = copy_tree(save_dir, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    std::fs::rename(&staging, &dest).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging);
        io_err(e, "finalize full-backup snapshot")
    })?;

    prune_full_snapshots(&backups);
    Ok(dest)
}

/// Recursively copy `src` into `dst`. Skips any `psm-backups` directory **at any
/// depth** — per-player edits create their own `Players/psm-backups` — so the
/// snapshot never contains prior backups or its own staging dir. Symlinks are
/// **followed** (classified by their target via `fs::metadata`), so a relocated
/// `Level.sav` or a bind-mounted `Players/` is captured rather than silently
/// omitted; a broken symlink surfaces as an error that refuses the edit rather
/// than yielding an incomplete "successful" snapshot. Transient `.psm-tmp`
/// files are skipped.
fn copy_tree(src: &Path, dst: &Path) -> Result<(), SaveError> {
    let entries = std::fs::read_dir(src).map_err(|e| io_err(e, "read save dir"))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err(e, "read dir entry"))?;
        let name = entry.file_name();
        // Exclude every psm-backups folder (top-level and nested) and the
        // atomic-replace temp file.
        if name.to_str() == Some("psm-backups") {
            continue;
        }
        if name.to_str().is_some_and(|n| n.ends_with(".psm-tmp")) {
            continue;
        }
        let path = entry.path();
        // Classify by the symlink *target* (`metadata` follows; `file_type`
        // does not) so symlinked files/dirs are captured, not skipped.
        let meta = std::fs::metadata(&path).map_err(|e| io_err(e, "stat save entry"))?;
        let target = dst.join(&name);
        if meta.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| io_err(e, "create snapshot subdir"))?;
            copy_tree(&path, &target)?;
        } else if meta.is_file() {
            std::fs::copy(&path, &target).map_err(|e| io_err(e, "copy save file"))?;
        }
    }
    Ok(())
}

/// Keep only the newest [`FULL_SNAPSHOT_KEEP`] `full-*` snapshot directories.
/// Best-effort — failures never block a save write.
fn prune_full_snapshots(backups: &Path) {
    let Ok(entries) = std::fs::read_dir(backups) else {
        return;
    };
    let mut dirs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("full-"))
        .collect();
    dirs.sort(); // full-<secs>.<millis> sorts chronologically (fixed-width fields)
    if dirs.len() > FULL_SNAPSHOT_KEEP {
        for n in &dirs[..dirs.len() - FULL_SNAPSHOT_KEEP] {
            let _ = std::fs::remove_dir_all(backups.join(n));
        }
    }
}

/// Replace `path` with `bytes` via a same-directory temp file + rename.
///
/// The temp file is flushed and synced before the rename. On Windows,
/// renaming onto an existing file fails, so the destination is removed first
/// — safe because [`backup_file`] has already preserved the original, and the
/// temp file (holding the complete new contents) survives any crash in the
/// gap. If the rename itself *returns an error* after the original was removed
/// (e.g. a transient Windows sharing violation from an AV/indexer briefly
/// holding the temp), the new bytes are written directly to `path` as a
/// fallback so the save is never left missing from its canonical location; only
/// if that recovery also fails does the error propagate (the original still
/// lives in the backup either way).
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let parent = path
        .parent()
        .ok_or_else(|| SaveError::Io(format!("{}: no parent dir", path.display())))?;
    let tmp = parent.join(format!(
        "{}.psm-tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("save")
    ));

    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| io_err(e, "create temp file"))?;
        f.write_all(bytes).map_err(|e| io_err(e, "write temp file"))?;
        f.sync_all().map_err(|e| io_err(e, "sync temp file"))?;
    }

    if path.exists() {
        std::fs::remove_file(path).map_err(|e| io_err(e, "remove old save"))?;
    }
    if let Err(rename_err) = std::fs::rename(&tmp, path) {
        // The original is already gone, so a failed rename must not leave the
        // canonical path with no file. Write the new (already-validated) bytes
        // directly — non-atomic, but the pre-edit contents survive in the
        // backup and the save stays present rather than vanishing from the
        // app's view. Report the original rename error only if this also fails.
        std::fs::write(path, bytes).map_err(|_| io_err(rename_err, "rename temp into place"))?;
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::decompress::decompress_sav_with_type;

    #[test]
    fn pack_round_trips_single_and_double_zlib() {
        let data = b"GVAS-not-really-but-payload-bytes-0123456789".repeat(50);
        for save_type in [0x31u8, 0x32u8] {
            let packed = pack_sav(&data, save_type).unwrap();
            let (unpacked, st) = decompress_sav_with_type(&packed).unwrap();
            assert_eq!(st, save_type);
            assert_eq!(unpacked, data, "round-trip mismatch for type {save_type:#x}");
        }
    }

    #[test]
    fn pack_raw_type_keeps_bytes() {
        let data = b"raw-body".to_vec();
        let packed = pack_sav(&data, 0x30).unwrap();
        let (unpacked, st) = decompress_sav_with_type(&packed).unwrap();
        assert_eq!(st, 0x30);
        assert_eq!(unpacked, data);
    }

    #[test]
    fn snapshot_copies_whole_dir_excluding_backups() {
        let dir = std::env::temp_dir().join(format!("psm-snap-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Players")).unwrap();
        std::fs::write(dir.join("Level.sav"), b"level").unwrap();
        std::fs::write(dir.join("Players").join("AAA.sav"), b"player").unwrap();
        // Pre-existing per-file backups that must NOT be copied into the
        // snapshot — top-level AND the nested Players/psm-backups a per-player
        // edit creates.
        std::fs::create_dir_all(dir.join("psm-backups")).unwrap();
        std::fs::write(dir.join("psm-backups").join("old.sav"), b"stale").unwrap();
        std::fs::create_dir_all(dir.join("Players").join("psm-backups")).unwrap();
        std::fs::write(dir.join("Players").join("psm-backups").join("p.sav"), b"nested").unwrap();

        let snap = snapshot_save_dir(&dir).unwrap();
        assert!(snap.starts_with(dir.join("psm-backups")), "snapshot lives under psm-backups");
        assert!(
            snap.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("full-")),
            "finalized snapshot is a full-* dir, not a staging dir"
        );
        assert_eq!(std::fs::read(snap.join("Level.sav")).unwrap(), b"level");
        assert_eq!(std::fs::read(snap.join("Players").join("AAA.sav")).unwrap(), b"player");
        // No psm-backups folder (top-level or nested) is captured, and the
        // snapshot never recurses into its own destination.
        assert!(!snap.join("psm-backups").exists(), "top-level psm-backups excluded");
        assert!(
            !snap.join("Players").join("psm-backups").exists(),
            "nested Players/psm-backups excluded"
        );
        // No leftover `.partial` staging dir remains after a successful snapshot.
        let leftover = std::fs::read_dir(dir.join("psm-backups"))
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_str().is_some_and(|n| n.contains(".partial")));
        assert!(!leftover, "staging dir renamed away, no .partial leftover");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_and_atomic_replace() {
        let dir = std::env::temp_dir().join(format!("psm-edit-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("Level.sav");
        std::fs::write(&f, b"original").unwrap();

        let backup = backup_file(&f).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), b"original");

        atomic_replace(&f, b"edited").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"edited");
        assert!(!list_backups(&f).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }
}
