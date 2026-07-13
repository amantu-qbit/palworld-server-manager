//! Integration test: the decoded-`World` cache in `AppState`.
//!
//! Fixture: the Phase-1a `world1` fixture, known (from
//! `bridge/tests/decode_world1.rs`) to decode to exactly 2 players.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use psm_bridge::state::{AppState, StateError};

const WORLD1_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/saves/world1");

/// A fresh, collision-proof scratch directory under the OS temp dir,
/// containing a copy of `world1/Level.sav`. `load_world` only ever reads
/// `<dir>/Level.sav` (never the sibling `LevelMeta.sav`/`Players/` files), so
/// copying just that one file is sufficient to reuse the fixture data while
/// letting each test freely mutate its own private copy.
fn copy_world1_into_scratch_dir(label: &str) -> PathBuf {
    let unique = format!(
        "{label}-{}-{}",
        uuid::Uuid::new_v4().simple(),
        std::process::id()
    );
    let dir = std::env::temp_dir().join(format!("psm-bridge-cache-test-{unique}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    std::fs::copy(
        Path::new(WORLD1_DIR).join("Level.sav"),
        dir.join("Level.sav"),
    )
    .expect("copy world1 Level.sav into scratch dir");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn bundle_decodes_two_players_from_fixture() {
    let state = AppState::new(PathBuf::from(WORLD1_DIR));

    let bundle = state.bundle().await.expect("bundle() should decode world1");

    assert_eq!(bundle.world.players.len(), 2, "world1 fixture has exactly 2 players");
    assert!(
        !bundle.item_containers.is_empty(),
        "world1 fixture's bundle has a non-empty item_containers index"
    );
}

#[tokio::test]
async fn second_call_returns_same_arc_without_redecoding() {
    let state = AppState::new(PathBuf::from(WORLD1_DIR));

    let a = state.bundle().await.expect("first bundle() call should decode");
    let b = state
        .bundle()
        .await
        .expect("second bundle() call should hit the cache");

    assert!(
        Arc::ptr_eq(&a, &b),
        "unchanged Level.sav must return the cached Arc, not decode again"
    );
}

#[tokio::test]
async fn cache_invalidates_and_redecodes_when_mtime_changes() {
    let dir = copy_world1_into_scratch_dir("mtime-invalidation");
    let state = AppState::new(dir.clone());

    let a = state.bundle().await.expect("first bundle() call should decode");

    // Bump the file's mtime forward without touching its contents/size, to
    // isolate the mtime half of the (mtime, size) cache key.
    let level_sav = dir.join("Level.sav");
    let file = std::fs::File::options()
        .write(true)
        .open(&level_sav)
        .expect("reopen Level.sav to bump mtime");
    file.set_modified(SystemTime::now() + Duration::from_secs(120))
        .expect("set_modified should succeed on a local scratch file");

    let b = state
        .bundle()
        .await
        .expect("bundle() after mtime bump should redecode successfully");

    assert!(
        !Arc::ptr_eq(&a, &b),
        "a changed mtime must invalidate the cache and produce a fresh Arc"
    );
    assert_eq!(b.world.players.len(), 2, "redecoded world still has 2 players");

    cleanup(&dir);
}

#[tokio::test]
async fn missing_save_file_is_an_io_error() {
    let dir = std::env::temp_dir().join(format!(
        "psm-bridge-cache-test-missing-{}-{}",
        uuid::Uuid::new_v4().simple(),
        std::process::id()
    ));
    let state = AppState::new(dir);

    let result = state.bundle().await;

    assert!(
        matches!(result, Err(StateError::Io(_))),
        "a save directory with no Level.sav must surface StateError::Io, got {result:?}"
    );
}

#[tokio::test]
async fn malformed_save_is_a_load_error_not_a_panic() {
    let dir = std::env::temp_dir().join(format!(
        "psm-bridge-cache-test-malformed-{}-{}",
        uuid::Uuid::new_v4().simple(),
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    // 12-byte header (uncompressed_len, compressed_len, magic, save_type)
    // followed by a few garbage bytes. Passes the "long enough to have a
    // header" check but fails to decompress -- proving a malformed save
    // comes back as a `Result` error rather than aborting the test process.
    std::fs::write(dir.join("Level.sav"), [0u8; 20]).expect("write malformed Level.sav");

    let state = AppState::new(dir.clone());
    let result = state.bundle().await;

    assert!(
        matches!(result, Err(StateError::Load(_))),
        "a malformed Level.sav must surface StateError::Load, got {result:?}"
    );

    cleanup(&dir);
}
