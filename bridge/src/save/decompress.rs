//! Decompress a Palworld `.sav` container to raw GVAS bytes.
//!
//! Container layout (ported from `palworld_save_tools/palsav.py::decompress_sav_to_gvas`
//! + `compressor/oozlib.py` + `compressor/zlib.py`):
//! - `u32 uncompressed_len` (LE)
//! - `u32 compressed_len` (LE)
//! - 3-byte magic: `PlZ` (zlib) or `PlM` (Oodle/Mermaid)
//! - 1-byte save type: `0x30` = uncompressed, `0x31` = single zlib, `0x32` = double zlib
//!   (only meaningful for the `PlZ` path; Oodle payloads are always a single Mermaid stream)
//! - remaining bytes = payload, compressed per magic/save type
//!
//! The magic byte determines the codec: `PlM` payloads are Oodle (Mermaid variant)
//! compressed and are decompressed with the `oozextract` crate; `PlZ` payloads are
//! zlib-compressed and are decompressed with `flate2`.

use std::io::Read;

use flate2::read::ZlibDecoder;
use oozextract::Extractor;
use thiserror::Error;

const HEADER_LEN: usize = 12;
const MAGIC_ZLIB: [u8; 3] = *b"PlZ";
const MAGIC_OODLE: [u8; 3] = *b"PlM";

/// Sane ceiling on a declared decompressed size (`.sav` header) or an inner
/// GVAS array/set/map element count. No legitimate Palworld world approaches
/// this; anything above it is a crafted or corrupt input and is rejected
/// before the corresponding allocation is attempted (Phase-1b hardening,
/// review finding S2).
pub(crate) const MAX_DECOMPRESSED: usize = 512 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("bad magic bytes in .sav header")]
    BadMagic,
    /// A file-controlled declared size (the `.sav` header's
    /// `uncompressed_len`, or an inner GVAS array/set/map element count)
    /// exceeds [`MAX_DECOMPRESSED`] or what the remaining input bytes could
    /// plausibly contain. Rejected before allocating/looping on it.
    #[error("declared size exceeds sane limit")]
    TooLarge,
    /// The magic bytes were valid `PlZ` (zlib), but the following save-type
    /// byte did not match any known variant (`0x30`/`0x31`/`0x32`).
    #[error("unknown save type: 0x{0:02x}")]
    UnknownSaveType(u8),
    #[error("truncated .sav file")]
    Truncated,
    #[error("oodle decompression error: {0}")]
    Oodle(String),
    #[error("zlib error: {0}")]
    Zlib(String),
    /// The GVAS envelope did not start with the `GVAS` file-type tag.
    #[error("invalid GVAS magic")]
    BadGvasMagic,
    /// A property/array/struct/set type was encountered that the generic reader
    /// does not (yet) decode. Carries the type name and the dotted path.
    #[error("unhandled GVAS type `{0}` at `{1}`")]
    UnhandledType(String, String),
    /// A filesystem error while reading a save file (path/message).
    #[error("io error: {0}")]
    Io(String),
    /// A character `RawData` blob did not match the expected layout
    /// (property stream + trailing group/unknown bytes). Carries a description.
    #[error("malformed character RawData: {0}")]
    CharacterData(String),
    /// An item/character container (or a slot's `RawData`) did not match the
    /// expected layout. Carries a description.
    #[error("malformed container data: {0}")]
    ContainerData(String),
    /// A `GroupSaveDataMap` entry (or a guild `RawData` blob) did not match the
    /// expected layout. Carries a description.
    #[error("malformed group data: {0}")]
    GroupData(String),
}

pub fn decompress_sav(bytes: &[u8]) -> Result<Vec<u8>, SaveError> {
    if bytes.len() < HEADER_LEN {
        return Err(SaveError::Truncated);
    }

    let uncompressed_len = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let compressed_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let magic: [u8; 3] = bytes[8..11].try_into().unwrap();
    let save_type = bytes[11];

    let body = &bytes[HEADER_LEN..];

    let decompressed = if magic == MAGIC_OODLE {
        oodle_decompress(body, compressed_len, uncompressed_len)?
    } else if magic == MAGIC_ZLIB {
        if body.len() < compressed_len {
            return Err(SaveError::Truncated);
        }
        let compressed = &body[..compressed_len];
        match save_type {
            0x30 => compressed.to_vec(),
            0x31 => zlib_decompress(compressed)?,
            0x32 => {
                let once = zlib_decompress(compressed)?;
                zlib_decompress(&once)?
            }
            _ => return Err(SaveError::UnknownSaveType(save_type)),
        }
    } else {
        return Err(SaveError::BadMagic);
    };

    if decompressed.len() != uncompressed_len {
        return Err(SaveError::Truncated);
    }

    Ok(decompressed)
}

fn oodle_decompress(
    body: &[u8],
    compressed_len: usize,
    uncompressed_len: usize,
) -> Result<Vec<u8>, SaveError> {
    if body.len() < compressed_len {
        return Err(SaveError::Truncated);
    }
    if uncompressed_len > MAX_DECOMPRESSED {
        return Err(SaveError::TooLarge);
    }
    let compressed = &body[..compressed_len];
    let mut output = vec![0u8; uncompressed_len];
    let written = Extractor::new()
        .read_from_slice(compressed, &mut output)
        .map_err(|e| SaveError::Oodle(e.to_string()))?;
    if written != uncompressed_len {
        return Err(SaveError::Truncated);
    }
    Ok(output)
}

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, SaveError> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| SaveError::Zlib(e.to_string()))?;
    Ok(out)
}
