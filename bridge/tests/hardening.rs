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

/// The toothless-test gap: a `MapProperty<BoolProperty, BoolProperty>` whose
/// declared `count` is small enough to satisfy the old on-disk
/// `remaining()`-guard (`count <= remaining() / 2`, since a `BoolProperty` map
/// entry's smallest on-disk encoding is 1 byte per key/value) but whose *real*
/// in-memory allocation — `Vec::<MapEntry>::with_capacity(count)`, at
/// `size_of::<MapEntry>()` bytes per element, not 2 — would exceed the
/// `gvas.rs::MAX_ELEM_ALLOC_BYTES` cap by tens of megabytes. Unlike the
/// existing `crafted_map_count_errors_without_oom` test (which supplies zero
/// trailing bytes, so `remaining() ≈ 0` rejects trivially without ever
/// reaching the real failure mode), this test pads the buffer to several
/// megabytes of `remaining()` so the on-disk guard genuinely passes and only
/// the size-aware guard catches the oversized allocation.
#[test]
fn crafted_map_bool_bool_count_passes_on_disk_guard_but_exceeds_elem_alloc_cap() {
    use psm_save::save::props::MapEntry;

    const MAX_ELEM_ALLOC_BYTES: usize = 256 * 1024 * 1024;
    let elem_size = std::mem::size_of::<MapEntry>();
    let min_on_disk_entry = 2usize; // BoolProperty(1) + BoolProperty(1)

    // Smallest count whose *real* Vec<MapEntry> allocation exceeds the cap,
    // plus a comfortable margin so the assertion below isn't a coin flip.
    let count = MAX_ELEM_ALLOC_BYTES / elem_size + 100_000;
    // A fixed several-MB pad — enough remaining() bytes that the old
    // `count <= remaining() / min_on_disk_entry` on-disk guard passes for any
    // `count` in the low millions (it does not for the `MapEntry` sizes seen
    // in practice, verified by the assertion below).
    let padding_len = 8 * 1024 * 1024;

    // Sanity-check the setup's own assumptions before exercising the parser:
    // the old on-disk guard alone would have accepted this count...
    assert!(
        count <= padding_len / min_on_disk_entry,
        "test setup invalid: count {count} would fail even the old on-disk guard \
         (elem_size={elem_size}, adjust padding_len)"
    );
    // ...but the true in-memory allocation exceeds the cap.
    assert!(
        count.saturating_mul(elem_size) > MAX_ELEM_ALLOC_BYTES,
        "test setup invalid: count {count} * elem_size {elem_size} does not exceed the cap"
    );

    let mut b = gvas_header();
    b.extend_from_slice(&fstring("Map")); // property name
    b.extend_from_slice(&fstring("MapProperty")); // type name
    b.extend_from_slice(&0u64.to_le_bytes()); // size (unused by read_map)
    b.extend_from_slice(&fstring("BoolProperty")); // key_type
    b.extend_from_slice(&fstring("BoolProperty")); // value_type
    b.push(0x00); // optional_guid: absent
    b.extend_from_slice(&0u32.to_le_bytes()); // padding
    b.extend_from_slice(&(count as u32).to_le_bytes()); // count
    b.extend(std::iter::repeat_n(0u8, padding_len)); // remaining() is several MB, not ~0

    let result = parse_gvas(&b, &default_skip_set());
    assert!(
        matches!(result, Err(SaveError::TooLarge)),
        "expected Err(TooLarge) from the size-aware elem-alloc guard, got {result:?}"
    );
}
