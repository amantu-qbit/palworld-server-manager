#[test]
fn fixtures_present_and_nonempty() {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/saves/world1/Level.sav");
    let meta = std::fs::metadata(p).expect("world1/Level.sav fixture must exist");
    assert!(meta.len() > 1024, "Level.sav should be a real save, not a stub");
}
