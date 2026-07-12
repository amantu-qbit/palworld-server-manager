use psm_save::save::reference::load_reference;

#[test]
fn resolves_a_known_item_name() {
    let r = load_reference();
    // Palworld internal id -> display name; "Wood" is a real static id from the
    // vendored catalog (bridge/data/items.json), confirmed against the source
    // data before writing this assertion.
    assert!(r.item_name("Wood").is_some());
    assert_eq!(r.item_name("Wood"), Some("Wood"));
}

#[test]
fn resolves_a_known_active_skill_name() {
    let r = load_reference();
    // Active (attack) skill ids are stored in the vendored catalog with the
    // `EPalWazaID::` prefix, matching the raw save-file `EquipWaza` values.
    assert_eq!(r.active_skill_name("EPalWazaID::AirBlade"), Some("Air Blade"));
}

#[test]
fn resolves_a_known_passive_skill_name() {
    let r = load_reference();
    assert_eq!(r.passive_skill_name("AirDash_1"), Some("Aerial Dash +1"));
}

#[test]
fn resolves_a_known_element_name() {
    let r = load_reference();
    assert_eq!(r.element_name("Fire"), Some("Fire"));
}

#[test]
fn unknown_ids_resolve_to_none() {
    let r = load_reference();
    assert!(r.item_name("NotARealItemId").is_none());
    assert!(r.active_skill_name("NotARealSkillId").is_none());
}
