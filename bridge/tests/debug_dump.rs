//! Integration tests for the Raw Save JSON projection (`save::debug`).
//!
//! The committed test runs against the `world1` `Level.sav` fixture. A second,
//! `#[ignore]`d test dumps the top-level structure of any `.sav` pointed to by
//! the `PSM_SAV` env var — used locally to explore files (e.g. `LocalData.sav`)
//! that live only under the gitignored `SavFileExample/`.

use std::path::Path;

use psm_save::save::debug::{node_to_json, resolve, DumpOpts};
use psm_save::save::decompress::decompress_sav;
use psm_save::save::gvas::{default_skip_set, parse_gvas};

const WORLD1_LEVEL: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/saves/world1/Level.sav");

fn tree(path: &str) -> serde_json::Value {
    let bytes = std::fs::read(path).expect("read sav");
    let raw = decompress_sav(&bytes).expect("decompress");
    let gvas = parse_gvas(&raw, &default_skip_set()).expect("parse");
    let node = resolve(&gvas.root, "").expect("root");
    node_to_json(&node, &DumpOpts::default())
}

#[test]
fn level_sav_exposes_world_save_data_and_summarizes_blobs() {
    let v = tree(WORLD1_LEVEL);
    // The world container is the top-level key in Level.sav.
    let world = &v["worldSaveData"];
    assert!(world.is_object(), "worldSaveData present as an object");

    // CharacterSaveParameterMap is a (large) MapProperty; at default depth it is
    // collapsed with a count, never expanded in full.
    let node = resolve_json(&v, ["worldSaveData", "CharacterSaveParameterMap"]);
    assert!(
        node["_type"].as_str().unwrap_or("").starts_with("Map<"),
        "CharacterSaveParameterMap is a Map, got {:?}",
        node["_type"]
    );
    assert!(node["_count"].as_u64().unwrap_or(0) >= 1, "map has entries");

    // No raw byte arrays leak: any `_bytes` node is just a length.
    assert!(no_raw_bytes(&v), "byte blobs must be summarized, not dumped");
}

fn resolve_json<'a>(v: &'a serde_json::Value, path: [&str; 2]) -> &'a serde_json::Value {
    &v[path[0]][path[1]]
}

/// Assert every `_bytes` field is a number (a length), never an array of bytes.
fn no_raw_bytes(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(m) => m.iter().all(|(k, val)| {
            if k == "_bytes" {
                val.is_number()
            } else {
                no_raw_bytes(val)
            }
        }),
        serde_json::Value::Array(a) => a.iter().all(no_raw_bytes),
        _ => true,
    }
}

/// Local-only: `PSM_SAV=<path> cargo test -p psm-save --test debug_dump -- --ignored --nocapture`
/// Prints the top-level keys + node types of any `.sav` (e.g. `LocalData.sav`).
#[test]
#[ignore]
fn dump_sav_structure_from_env() {
    let path = std::env::var("PSM_SAV").expect("set PSM_SAV=<path to .sav>");
    let sub = std::env::var("PSM_PATH").unwrap_or_default();
    assert!(Path::new(&path).is_file(), "PSM_SAV must point at a file");
    let bytes = std::fs::read(&path).expect("read sav");
    let raw = decompress_sav(&bytes).expect("decompress");
    let gvas = parse_gvas(&raw, &default_skip_set()).expect("parse");
    let node = resolve(&gvas.root, &sub).expect("path exists");
    let v = node_to_json(&node, &DumpOpts { page: Some(60), depth: 2 });

    println!("=== structure of {path} @ '{sub}' ===");
    if let serde_json::Value::Object(m) = &v {
        for (k, val) in m {
            if k == "_type" {
                continue;
            }
            let t = val
                .get("_type")
                .and_then(|t| t.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| short_kind(val));
            match val.get("_count").and_then(|c| c.as_u64()) {
                Some(n) => println!("  {k}: {t} ({n})"),
                None => println!("  {k}: {t}"),
            }
        }
    }
}

fn short_kind(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
    .to_string()
}
