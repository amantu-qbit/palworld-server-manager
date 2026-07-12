//! Phase-1b Task 1: crafted-input hardening regression tests.
//!
//! Covers review finding S2 — file-controlled `u32` lengths/counts must be
//! rejected with a `SaveError` *before* the corresponding allocation is
//! attempted, not after (which, for a multi-GB request, aborts the process
//! rather than returning an `Err` a caller could catch).

use psm_save::save::decompress::{decompress_sav, SaveError};
use psm_save::save::gvas::{default_skip_set, parse_gvas};

/// A .sav header claiming a 4 GB uncompressed_len must error, not attempt the allocation.
#[test]
fn absurd_uncompressed_len_errors() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // uncompressed_len ~4GB
    bytes.extend_from_slice(&4u32.to_le_bytes()); // compressed_len
    bytes.extend_from_slice(b"PlM"); // Oodle magic
    bytes.push(0x31);
    bytes.extend_from_slice(&[0u8; 4]);
    assert!(matches!(
        decompress_sav(&bytes),
        Err(SaveError::TooLarge) | Err(SaveError::Truncated) | Err(SaveError::Oodle(_))
    ));
}

/// Encode a UE `fstring` in its positive-length ASCII form: an `i32` byte
/// count (including the trailing NUL), then the ASCII bytes plus NUL.
/// Mirrors `Reader::fstring`'s decode of the positive-length branch.
fn fstring(s: &str) -> Vec<u8> {
    let mut body = s.as_bytes().to_vec();
    body.push(0); // NUL terminator
    let mut out = (body.len() as i32).to_le_bytes().to_vec();
    out.extend_from_slice(&body);
    out
}

/// A minimal-but-valid GVAS envelope header (magic through save-game class
/// name) — enough for `parse_gvas`'s `read_header` to advance past it and
/// start reading the root property set. Mirrors `gvas.rs::read_header`'s
/// field order exactly.
fn gvas_header() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&0x5341_5647i32.to_le_bytes()); // "GVAS" magic
    b.extend_from_slice(&0i32.to_le_bytes()); // save_game_version
    b.extend_from_slice(&0i32.to_le_bytes()); // package_file_version_ue4
    b.extend_from_slice(&0i32.to_le_bytes()); // package_file_version_ue5
    b.extend_from_slice(&0u16.to_le_bytes()); // engine_major
    b.extend_from_slice(&0u16.to_le_bytes()); // engine_minor
    b.extend_from_slice(&0u16.to_le_bytes()); // engine_patch
    b.extend_from_slice(&0u32.to_le_bytes()); // engine_changelist
    b.extend_from_slice(&fstring("")); // engine_branch
    b.extend_from_slice(&0i32.to_le_bytes()); // custom_version_format
    b.extend_from_slice(&0u32.to_le_bytes()); // custom_version_count
    b.extend_from_slice(&fstring("")); // save_game_class_name
    b
}

/// A crafted `ArrayProperty<Guid>` that declares an absurd element count
/// (~4 billion) but supplies zero trailing bytes must error out of the
/// count guard in `gvas.rs::read_array`'s `Guid` branch, rather than
/// looping/pre-allocating against the fabricated count.
#[test]
fn crafted_array_count_errors_without_oom() {
    let mut b = gvas_header();

    b.extend_from_slice(&fstring("Arr")); // property name
    b.extend_from_slice(&fstring("ArrayProperty")); // type name
    b.extend_from_slice(&0u64.to_le_bytes()); // size (unused by the Guid branch)
    b.extend_from_slice(&fstring("Guid")); // array_type
    b.push(0x00); // optional_guid: absent
    b.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // count: ~4 billion
    // No further bytes: a real file would need count * 16 bytes here.

    let result = parse_gvas(&b, &default_skip_set());
    assert!(
        result.is_err(),
        "crafted huge array count must error, not hang/OOM"
    );
}

/// Same shape as above but for a `MapProperty<IntProperty, IntProperty>` —
/// covers the `read_map` count guard independently of `read_array`'s.
#[test]
fn crafted_map_count_errors_without_oom() {
    let mut b = gvas_header();

    b.extend_from_slice(&fstring("Map")); // property name
    b.extend_from_slice(&fstring("MapProperty")); // type name
    b.extend_from_slice(&0u64.to_le_bytes()); // size (unused by read_map)
    b.extend_from_slice(&fstring("IntProperty")); // key_type
    b.extend_from_slice(&fstring("IntProperty")); // value_type
    b.push(0x00); // optional_guid: absent
    b.extend_from_slice(&0u32.to_le_bytes()); // padding
    b.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // count: ~4 billion
    // No further bytes: a real file would need count * 8 bytes here.

    let result = parse_gvas(&b, &default_skip_set());
    assert!(
        result.is_err(),
        "crafted huge map count must error, not hang/OOM"
    );
}
