// bridge/tests/model_serde.rs
use psm_save::save::model::Pal;

#[test]
fn pal_roundtrips_json_with_expected_fields() {
    let p = Pal { character_id: "SheepBall".into(), level: 5, talent_hp: 42, rank_defense: 3, ..Pal::default() };
    let j = serde_json::to_value(&p).unwrap();
    assert_eq!(j["character_id"], "SheepBall");
    assert_eq!(j["talent_hp"], 42);
    assert_eq!(j["rank_defense"], 3); // JSON stays snake_case; GVAS spelling handled in the decoder
}
