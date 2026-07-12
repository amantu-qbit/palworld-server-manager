use std::process::Command;

#[test]
fn dump_world1_prints_players_json() {
    let out = Command::new(env!("CARGO_BIN_EXE_psm-save-dump"))
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/saves/world1"))
        .output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("\"players\""));
    assert!(s.to_lowercase().contains("8c2f1930"));
}
