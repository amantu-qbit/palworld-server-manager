use psm_save::save::decompress::decompress_sav;

fn read(p: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/saves/{}", env!("CARGO_MANIFEST_DIR"), p)).unwrap()
}

#[test]
fn level_sav_decompresses_to_gvas() {
    let raw = decompress_sav(&read("world1/Level.sav")).expect("decompress");
    assert_eq!(&raw[0..4], b"GVAS", "decompressed payload must start with GVAS magic");
}

#[test]
fn player_sav_decompresses_to_gvas() {
    let raw = decompress_sav(&read("world1/Players/8C2F1930000000000000000000000000.sav")).unwrap();
    assert_eq!(&raw[0..4], b"GVAS");
}

/// A truncated Oodle (`PlM`) payload must return `Err`, not a zero-padded `Ok` buffer.
///
/// Takes the real Level.sav container, chops the compressed body short, and patches
/// the header's `compressed_len` to match — mirroring a corrupted/incomplete file on
/// disk. This is a general safety-net regression test for the truncation path.
///
/// Note: with `oozextract` 0.5.4, its `Slice` input reader bounds-checks every read
/// internally and itself returns `Err` on any underrun, so truncating this real
/// fixture doesn't reproduce the exact narrow condition the CRITICAL finding
/// described (`read_from_slice` returning `Ok(n)` with `n < uncompressed_len`) —
/// that was verified empirically by scanning ~800 truncation offsets, all of which
/// errored out of `oozextract` itself rather than returning a short `Ok`. The
/// `written != uncompressed_len` check added in `oodle_decompress` remains a
/// necessary backstop (defense in depth against a future `oozextract` version, or
/// any other decoder, that *does* return a short count instead of erroring), but
/// this crate version doesn't offer a code path to hit it from real/malformed
/// fixture bytes without hand-crafting a byte-exact malformed Oodle bitstream.
#[test]
fn truncated_oodle_payload_errors_rather_than_returning_zero_padded_ok() {
    let mut raw = read("world1/Level.sav");
    let compressed_len = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;

    // Chop the compressed body to half its length and patch the header to match,
    // so the `body.len() < compressed_len` bounds check still passes and the
    // truncated slice is handed straight to the Oodle decoder.
    let new_compressed_len = compressed_len / 2;
    raw.truncate(12 + new_compressed_len);
    raw[4..8].copy_from_slice(&(new_compressed_len as u32).to_le_bytes());

    let result = decompress_sav(&raw);
    assert!(
        result.is_err(),
        "truncated Oodle payload must not decompress successfully"
    );
}
