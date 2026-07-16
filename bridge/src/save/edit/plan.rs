//! Splice plan: byte patches + automatic length/count fixups.
//!
//! An edit is expressed entirely in *original-buffer* coordinates:
//!
//! - [`Patch`] — replace `range` with `bytes` (insertion when the range is
//!   empty, deletion when `bytes` is empty).
//! - [`LenScope`] — a declared-size field (`u32`/`u64`, little-endian byte
//!   count) whose value must grow/shrink by the net byte delta of every patch
//!   inside its `body`. Ops register one scope per enclosing size field on the
//!   descent path; the plan computes the deltas so no op ever hand-computes a
//!   size.
//! - [`CountFix`] — a `u32` element count adjusted by an explicit delta
//!   (element counts change per element, not per byte, so they cannot be
//!   derived from patch lengths).
//!
//! [`apply`] validates the plan (in-bounds, non-overlapping, no patch
//! straddling a scope boundary), converts fixups into same-length patches, and
//! splices everything in one pass. Because size-field patches never change
//! length, nesting order between scopes is irrelevant.

use std::ops::Range;

use super::super::decompress::SaveError;

fn edit_err(msg: impl Into<String>) -> SaveError {
    SaveError::Edit(msg.into())
}

/// Replace `range` of the original buffer with `bytes`.
#[derive(Debug, Clone)]
pub struct Patch {
    pub range: Range<usize>,
    pub bytes: Vec<u8>,
}

/// The width of a declared-length field.
#[derive(Debug, Clone, Copy)]
pub enum LenKind {
    U32,
    U64,
}

/// A declared byte-length field covering `body`.
#[derive(Debug, Clone)]
pub struct LenScope {
    pub kind: LenKind,
    /// Offset of the length field itself.
    pub offset: usize,
    /// The byte region the field's value measures.
    pub body: Range<usize>,
}

/// A `u32` element-count field adjusted by an explicit element delta.
#[derive(Debug, Clone)]
pub struct CountFix {
    pub offset: usize,
    pub delta: i64,
}

/// A complete edit: content patches plus the length/count fields they affect.
#[derive(Debug, Default)]
pub struct EditPlan {
    pub patches: Vec<Patch>,
    pub scopes: Vec<LenScope>,
    pub counts: Vec<CountFix>,
}

impl EditPlan {
    pub fn patch(&mut self, range: Range<usize>, bytes: Vec<u8>) {
        self.patches.push(Patch { range, bytes });
    }

    pub fn insert(&mut self, at: usize, bytes: Vec<u8>) {
        self.patches.push(Patch { range: at..at, bytes });
    }

    pub fn delete(&mut self, range: Range<usize>) {
        self.patches.push(Patch { range, bytes: Vec::new() });
    }

    pub fn scope_u64(&mut self, offset: usize, body: Range<usize>) {
        self.scopes.push(LenScope { kind: LenKind::U64, offset, body });
    }

    pub fn scope_u32(&mut self, offset: usize, body: Range<usize>) {
        self.scopes.push(LenScope { kind: LenKind::U32, offset, body });
    }

    pub fn count(&mut self, offset: usize, delta: i64) {
        self.counts.push(CountFix { offset, delta });
    }

    /// Net byte delta of the whole plan (patches only; fixups are
    /// length-preserving).
    pub fn net_delta(&self) -> i64 {
        self.patches
            .iter()
            .map(|p| p.bytes.len() as i64 - p.range.len() as i64)
            .sum()
    }
}

/// Apply `plan` to `buf`, returning the new buffer.
pub fn apply(buf: &[u8], plan: &EditPlan) -> Result<Vec<u8>, SaveError> {
    // --- validate content patches ------------------------------------------
    let mut sorted: Vec<&Patch> = plan.patches.iter().collect();
    sorted.sort_by_key(|p| (p.range.start, p.range.end));
    let mut prev_end = 0usize;
    for p in &sorted {
        if p.range.end > buf.len() || p.range.start > p.range.end {
            return Err(edit_err(format!("patch out of bounds: {:?}", p.range)));
        }
        if p.range.start < prev_end {
            return Err(edit_err(format!(
                "overlapping patches at {}..{}",
                p.range.start, p.range.end
            )));
        }
        prev_end = p.range.end.max(p.range.start);
    }

    // --- scope deltas --------------------------------------------------------
    let mut fixup_patches: Vec<Patch> = Vec::new();
    for scope in &plan.scopes {
        let mut delta = 0i64;
        for p in &plan.patches {
            let inside = p.range.start >= scope.body.start && p.range.end <= scope.body.end;
            let outside = p.range.end <= scope.body.start || p.range.start >= scope.body.end;
            if inside {
                delta += p.bytes.len() as i64 - p.range.len() as i64;
            } else if !outside {
                return Err(edit_err(format!(
                    "patch {:?} straddles scope body {:?}",
                    p.range, scope.body
                )));
            }
        }
        if delta == 0 {
            continue;
        }
        match scope.kind {
            LenKind::U64 => {
                let off = scope.offset;
                let old = u64::from_le_bytes(
                    buf.get(off..off + 8)
                        .ok_or_else(|| edit_err("scope field out of bounds"))?
                        .try_into()
                        .unwrap(),
                );
                let new = (old as i64)
                    .checked_add(delta)
                    .filter(|v| *v >= 0)
                    .ok_or_else(|| edit_err("scope underflow"))? as u64;
                fixup_patches.push(Patch {
                    range: off..off + 8,
                    bytes: new.to_le_bytes().to_vec(),
                });
            }
            LenKind::U32 => {
                let off = scope.offset;
                let old = u32::from_le_bytes(
                    buf.get(off..off + 4)
                        .ok_or_else(|| edit_err("scope field out of bounds"))?
                        .try_into()
                        .unwrap(),
                );
                let new = (old as i64)
                    .checked_add(delta)
                    .filter(|v| *v >= 0 && *v <= u32::MAX as i64)
                    .ok_or_else(|| edit_err("scope out of range"))? as u32;
                fixup_patches.push(Patch {
                    range: off..off + 4,
                    bytes: new.to_le_bytes().to_vec(),
                });
            }
        }
    }

    // --- count fixups --------------------------------------------------------
    for cf in &plan.counts {
        let off = cf.offset;
        let old = u32::from_le_bytes(
            buf.get(off..off + 4)
                .ok_or_else(|| edit_err("count field out of bounds"))?
                .try_into()
                .unwrap(),
        );
        let new = (old as i64)
            .checked_add(cf.delta)
            .filter(|v| *v >= 0 && *v <= u32::MAX as i64)
            .ok_or_else(|| edit_err("count out of range"))? as u32;
        fixup_patches.push(Patch {
            range: off..off + 4,
            bytes: new.to_le_bytes().to_vec(),
        });
    }

    // Fixup patches are same-length overwrites of size/count fields. They may
    // nest inside other scopes' bodies (harmless: zero delta) but must not
    // overlap content patches (a content patch replacing a region that
    // contains a field being fixed up would be contradictory). A scope
    // accidentally registered twice yields byte-identical fixups — dedup
    // those; different bytes at the same offset are a contradictory plan.
    fixup_patches.sort_by_key(|p| (p.range.start, p.range.end));
    fixup_patches.dedup_by(|a, b| a.range == b.range && a.bytes == b.bytes);
    for pair in fixup_patches.windows(2) {
        if pair[0].range.start < pair[1].range.end && pair[1].range.start < pair[0].range.end {
            return Err(edit_err(format!(
                "conflicting fixups at {:?} / {:?}",
                pair[0].range, pair[1].range
            )));
        }
    }
    for f in &fixup_patches {
        for p in &plan.patches {
            if f.range.start < p.range.end && p.range.start < f.range.end {
                return Err(edit_err(format!(
                    "fixup at {:?} overlaps content patch {:?}",
                    f.range, p.range
                )));
            }
        }
    }

    // --- splice ---------------------------------------------------------------
    let mut all: Vec<&Patch> = plan.patches.iter().chain(fixup_patches.iter()).collect();
    all.sort_by_key(|p| (p.range.start, p.range.end));

    let capacity = (buf.len() as i64 + plan.net_delta()) as usize;
    let mut out = Vec::with_capacity(capacity);
    let mut cursor = 0usize;
    for p in all {
        out.extend_from_slice(&buf[cursor..p.range.start]);
        out.extend_from_slice(&p.bytes);
        cursor = p.range.end;
    }
    out.extend_from_slice(&buf[cursor..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splice_insert_delete_replace() {
        let buf = b"0123456789".to_vec();
        let mut plan = EditPlan::default();
        plan.patch(2..4, b"AB".to_vec()); // replace "23"
        plan.insert(6, b"xyz".to_vec()); // insert before "6"
        plan.delete(8..9); // drop "8"
        let out = apply(&buf, &plan).unwrap();
        assert_eq!(out, b"01AB45xyz679".to_vec());
        assert_eq!(plan.net_delta(), 2);
    }

    #[test]
    fn scope_fixup_adjusts_len_field() {
        // [u32 len][body: 4 bytes] — grow body by 2.
        let mut buf = 4u32.to_le_bytes().to_vec();
        buf.extend_from_slice(b"asdf");
        let mut plan = EditPlan::default();
        plan.insert(6, b"xx".to_vec());
        plan.scope_u32(0, 4..8);
        let out = apply(&buf, &plan).unwrap();
        assert_eq!(&out[..4], &6u32.to_le_bytes());
        assert_eq!(&out[4..], b"asxxdf");
    }

    #[test]
    fn straddling_patch_rejected() {
        let buf = vec![0u8; 10];
        let mut plan = EditPlan::default();
        plan.patch(3..7, vec![1, 2]);
        plan.scope_u32(0, 4..8);
        assert!(apply(&buf, &plan).is_err());
    }

    #[test]
    fn overlapping_patches_rejected() {
        let buf = vec![0u8; 10];
        let mut plan = EditPlan::default();
        plan.patch(2..5, vec![]);
        plan.patch(4..6, vec![9]);
        assert!(apply(&buf, &plan).is_err());
    }

    #[test]
    fn count_fix_applies_delta() {
        let mut buf = 7u32.to_le_bytes().to_vec();
        buf.extend_from_slice(&[0u8; 4]);
        let mut plan = EditPlan::default();
        plan.count(0, -2);
        let out = apply(&buf, &plan).unwrap();
        assert_eq!(&out[..4], &5u32.to_le_bytes());
    }
}
