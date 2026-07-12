//! UE property-archive primitive reader.
//!
//! A faithful port of the primitive readers in
//! `palworld_save_tools/archive.py::FArchiveReader`. These are exact,
//! well-defined Unreal Engine serialization primitives; every later decode
//! task is built on top of them, so a subtly wrong `fstring` or `guid` here
//! would silently corrupt all downstream decoding.
//!
//! Conventions:
//! - All integers and floats are little-endian (UE saves are produced on
//!   little-endian platforms; `struct.Struct("i")` etc. in archive.py use the
//!   native little-endian layout).
//! - The readers have infallible signatures and **panic** on a buffer underrun
//!   (or, for `fstring`, fall back to a lossy decode of malformed text). This
//!   mirrors archive.py, where a short `BytesIO.read` feeds `struct.unpack`
//!   and raises. Callers treat a corrupt archive as a hard error, so panicking
//!   at the primitive layer surfaces corruption immediately rather than
//!   letting it propagate as plausible-looking garbage.

use uuid::Uuid;

/// Cursor over a byte slice exposing UE archive primitives.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Create a reader positioned at the start of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Advance the cursor by `n` bytes and return the consumed slice.
    ///
    /// Panics on underrun (fewer than `n` bytes remaining).
    fn take(&mut self, n: usize) -> &'a [u8] {
        let end = self.pos + n;
        assert!(
            end <= self.data.len(),
            "Reader underrun: need {n} bytes at pos {} but only {} remain",
            self.pos,
            self.data.len() - self.pos
        );
        let slice = &self.data[self.pos..end];
        self.pos = end;
        slice
    }

    // --- cursor accessors -------------------------------------------------

    /// Current cursor position (bytes consumed so far).
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Total length of the underlying buffer.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Bytes remaining after the cursor.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// True once the cursor has reached the end of the buffer.
    pub fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    // --- raw byte access --------------------------------------------------

    /// Read exactly `n` bytes, advancing the cursor. Panics on underrun.
    pub fn read(&mut self, n: usize) -> &'a [u8] {
        self.take(n)
    }

    /// Consume and return all remaining bytes.
    pub fn read_to_end(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos..];
        self.pos = self.data.len();
        slice
    }

    /// Advance the cursor by `n` bytes, discarding them. Panics on underrun.
    pub fn skip(&mut self, n: usize) {
        let _ = self.take(n);
    }

    // --- little-endian scalars -------------------------------------------

    /// Read a single unsigned byte (archive.py `byte`).
    pub fn read_u8(&mut self) -> u8 {
        self.take(1)[0]
    }

    /// Read a little-endian `i16` (archive.py `i16`).
    pub fn read_i16(&mut self) -> i16 {
        i16::from_le_bytes(self.take(2).try_into().unwrap())
    }

    /// Read a little-endian `u16` (archive.py `u16`).
    pub fn read_u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }

    /// Read a little-endian `i32` (archive.py `i32`).
    pub fn read_i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    /// Read a little-endian `u32` (archive.py `u32`).
    pub fn read_u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    /// Read a little-endian `i64` (archive.py `i64`).
    pub fn read_i64(&mut self) -> i64 {
        i64::from_le_bytes(self.take(8).try_into().unwrap())
    }

    /// Read a little-endian `u64` (archive.py `u64`).
    pub fn read_u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }

    /// Read a little-endian `f32` (archive.py `float`).
    pub fn read_f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    /// Read a little-endian `f64` (archive.py `double`).
    pub fn read_f64(&mut self) -> f64 {
        f64::from_le_bytes(self.take(8).try_into().unwrap())
    }

    /// Read a 1-byte boolean; `true` iff the byte is non-zero
    /// (archive.py `bool` = `byte() > 0`).
    pub fn read_bool(&mut self) -> bool {
        self.read_u8() != 0
    }

    // --- strings ----------------------------------------------------------

    /// Read a UE length-prefixed string (archive.py `fstring`).
    ///
    /// Layout: an `i32` length prefix, then the string body followed by a NUL
    /// terminator that is dropped:
    /// - `len == 0` → empty string (no body follows).
    /// - `len > 0`  → `len` bytes of ASCII/UTF-8 (the last of which is the NUL
    ///   terminator and is discarded).
    /// - `len < 0`  → `2 * (-len)` bytes of UTF-16LE (the final 2 bytes being
    ///   the NUL terminator, discarded); `-len` counts UTF-16 code units
    ///   including the terminator.
    ///
    /// Well-formed Palworld strings are pure ASCII (positive path) or UTF-16
    /// (negative path); on the rare chance of malformed text we fall back to a
    /// lossy decode rather than panic, mirroring archive.py's non-crashing
    /// `surrogatepass` fallback (Rust's `String` cannot hold unpaired
    /// surrogates, so U+FFFD replacement is used instead).
    pub fn fstring(&mut self) -> String {
        let size = self.read_i32();
        if size == 0 {
            return String::new();
        }

        if size < 0 {
            // UTF-16LE: -size code units incl. the trailing NUL.
            let units = (-(size as i64)) as usize;
            let bytes = self.take(units * 2);
            let body = &bytes[..bytes.len() - 2]; // drop 2-byte NUL
            let u16s: Vec<u16> = body
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&u16s).unwrap_or_else(|_| String::from_utf16_lossy(&u16s))
        } else {
            // ASCII / UTF-8: size bytes incl. the trailing NUL.
            let bytes = self.take(size as usize);
            let body = &bytes[..bytes.len() - 1]; // drop 1-byte NUL
            match std::str::from_utf8(body) {
                Ok(s) => s.to_owned(),
                Err(_) => String::from_utf8_lossy(body).into_owned(),
            }
        }
    }

    // --- GUIDs ------------------------------------------------------------

    /// Read a 16-byte UE GUID (archive.py `guid`).
    ///
    /// UE stores a GUID on disk as four little-endian `u32` words (`A B C D`).
    /// The canonical RFC-4122 byte order (what `Uuid` prints) is each 4-byte
    /// word in big-endian order, i.e. every group of 4 on-disk bytes reversed.
    /// This matches `UUID.__str__` in archive.py, and makes the canonical
    /// string (uppercased, dashes stripped) equal the Palworld player-UID file
    /// name — e.g. on-disk `30 19 2F 8C 00…` → `8c2f1930-0000-…-000000000000`
    /// → `8C2F1930000000000000000000000000.sav`.
    pub fn guid(&mut self) -> Uuid {
        let r = self.take(16);
        let canonical = [
            r[3], r[2], r[1], r[0], // word A (LE) -> BE
            r[7], r[6], r[5], r[4], // word B
            r[11], r[10], r[9], r[8], // word C
            r[15], r[14], r[13], r[12], // word D
        ];
        Uuid::from_bytes(canonical)
    }

    /// Read an optional GUID (archive.py `optional_guid`): a 1-byte present
    /// flag, followed by a `guid()` iff the flag is non-zero.
    pub fn optional_guid(&mut self) -> Option<Uuid> {
        if self.read_u8() != 0 {
            Some(self.guid())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_scalars_round_trip() {
        let mut buf = Vec::new();
        buf.push(0xFEu8); // u8
        buf.extend_from_slice(&(-2i16).to_le_bytes());
        buf.extend_from_slice(&0xBEEFu16.to_le_bytes());
        buf.extend_from_slice(&(-123456i32).to_le_bytes());
        buf.extend_from_slice(&0xDEADBEEFu32.to_le_bytes());
        buf.extend_from_slice(&(-9_000_000_000i64).to_le_bytes());
        buf.extend_from_slice(&0x1122334455667788u64.to_le_bytes());
        buf.extend_from_slice(&1.5f32.to_le_bytes());
        buf.extend_from_slice(&(-2.5f64).to_le_bytes());
        buf.push(0x00); // bool false
        buf.push(0x07); // bool true (non-zero)

        let mut r = Reader::new(&buf);
        assert_eq!(r.read_u8(), 0xFE);
        assert_eq!(r.read_i16(), -2);
        assert_eq!(r.read_u16(), 0xBEEF);
        assert_eq!(r.read_i32(), -123456);
        assert_eq!(r.read_u32(), 0xDEADBEEF);
        assert_eq!(r.read_i64(), -9_000_000_000);
        assert_eq!(r.read_u64(), 0x1122334455667788);
        assert_eq!(r.read_f32(), 1.5);
        assert_eq!(r.read_f64(), -2.5);
        assert!(!r.read_bool());
        assert!(r.read_bool());
        assert!(r.eof());
    }

    #[test]
    fn fstring_empty() {
        let buf = 0i32.to_le_bytes();
        let mut r = Reader::new(&buf);
        assert_eq!(r.fstring(), "");
    }

    #[test]
    fn fstring_utf16_negative_length() {
        // "Pál" in UTF-16LE = 3 code units + a NUL terminator => 4 units,
        // so the length prefix is -4 and 8 bytes of body follow.
        let s = "Pál";
        let mut units: Vec<u16> = s.encode_utf16().collect();
        assert_eq!(units.len(), 3);
        units.push(0); // NUL terminator
        let prefix = -(units.len() as i32); // -4
        let mut buf = Vec::new();
        buf.extend_from_slice(&prefix.to_le_bytes());
        for u in &units {
            buf.extend_from_slice(&u.to_le_bytes());
        }
        let mut r = Reader::new(&buf);
        assert_eq!(r.fstring(), "Pál");
        assert!(r.eof());
    }

    #[test]
    fn optional_guid_absent_and_present() {
        // Absent: single 0x00 flag byte, no guid follows.
        let absent = [0u8];
        let mut r = Reader::new(&absent);
        assert_eq!(r.optional_guid(), None);
        assert_eq!(r.pos(), 1);

        // Present: 0x01 flag, then 16 on-disk GUID bytes. First word little
        // endian is `30 19 2F 8C`, matching the fixture player file name
        // `8C2F1930000000000000000000000000.sav`.
        let mut buf = vec![0x01u8];
        buf.extend_from_slice(&[0x30, 0x19, 0x2F, 0x8C]);
        buf.extend_from_slice(&[0u8; 12]);
        let mut r = Reader::new(&buf);
        let g = r.optional_guid().expect("present flag => Some");
        assert_eq!(g, Uuid::parse_str("8c2f1930-0000-0000-0000-000000000000").unwrap());
        // Uppercased, dash-stripped canonical form == the player-UID file name.
        assert_eq!(
            g.simple().to_string().to_uppercase(),
            "8C2F1930000000000000000000000000"
        );
        assert!(r.eof());
    }

    #[test]
    fn guid_word_byte_order() {
        // Distinct byte in every position verifies the per-word LE->BE swap.
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0x00, 0x01, 0x02, 0x03]); // word A
        buf.extend_from_slice(&[0x04, 0x05, 0x06, 0x07]); // word B
        buf.extend_from_slice(&[0x08, 0x09, 0x0A, 0x0B]); // word C
        buf.extend_from_slice(&[0x0C, 0x0D, 0x0E, 0x0F]); // word D
        let mut r = Reader::new(&buf);
        let g = r.guid();
        assert_eq!(
            g,
            Uuid::parse_str("03020100-0706-0504-0b0a-09080f0e0d0c").unwrap()
        );
    }
}
