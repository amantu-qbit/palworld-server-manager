fn main() {
    let dir = std::env::args().nth(1).expect("usage: psm-save-dump <save-dir>");
    let world = psm_save::save::load_world(std::path::Path::new(&dir)).expect("load");
    println!("{}", serde_json::to_string_pretty(&world).unwrap());
}
