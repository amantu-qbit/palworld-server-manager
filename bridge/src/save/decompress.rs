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

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("bad magic bytes in .sav header")]
    BadMagic,
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
            _ => return Err(SaveError::BadMagic),
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
