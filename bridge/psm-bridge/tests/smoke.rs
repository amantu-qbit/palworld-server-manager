#[test]
fn psm_save_dependency_links() {
    // The bridge crate can call into the decoder it depends on.
    let skip = psm_save::save::gvas::default_skip_set();
    assert!(!format!("{skip:?}").is_empty());
}
