use psm_save::save::reader::Reader;

#[test]
fn reads_le_primitives_and_ue_string() {
    // u32 LE = 5, then an ASCII UE string "GVAS" (len prefix 5 incl. null terminator, "GVAS\0")
    let mut buf = Vec::new();
    buf.extend_from_slice(&5u32.to_le_bytes());
    buf.extend_from_slice(&5i32.to_le_bytes());
    buf.extend_from_slice(b"GVAS\0");
    let mut r = Reader::new(&buf);
    assert_eq!(r.read_u32(), 5);
    assert_eq!(r.fstring(), "GVAS");
}
