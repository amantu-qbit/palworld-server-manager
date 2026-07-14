use psm_save::save::{decompress::decompress_sav, gvas::{parse_gvas, default_skip_set}};

fn level() -> Vec<u8> {
    decompress_sav(&std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/saves/world1/Level.sav")).unwrap()).unwrap()
}

#[test]
fn parses_level_root_keys() {
    let g = parse_gvas(&level(), &default_skip_set()).expect("parse gvas");
    // worldSaveData is the root struct; these maps must be present.
    let wsd = g.root.get("worldSaveData").expect("worldSaveData present");
    for key in ["CharacterSaveParameterMap", "GroupSaveDataMap"] {
        assert!(wsd.has_child(key), "missing {key}");
    }
}
