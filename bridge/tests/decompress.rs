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
