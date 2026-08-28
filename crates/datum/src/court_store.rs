//! Durable court store — RewardBook across process restart (diamond ♦2 / D-L2).
//!
//! ```text
//! RewardBook.acts  ──encode──►  bytes / file
//!        ▲                         │
//!        └──────── decode ─────────┘
//! ```
//!
//! **Source of truth is the act log.** Seen set and cumulative totals
//! are rebuilt from acts. Υ is never stored (non-wire). Edge/quay are
//! not involved — this is court (C) only; live venue remains the edge
//! implementor's lane.
//!
//! Format (version 1):
//!
//! ```text
//! magic "XDCT" ‖ ver u8=1 ‖ n_acts u32 LE
//!   ‖ for each act:
//!        tag u8=1 (Credited)
//!        transport u64 LE
//!        classes u8   bit0=game bit1=edge
//!        work_id_len u32 LE ‖ work_id bytes
//!        n_axes u32 LE ‖ axes u128 LE × n
//! ```

use std::fs;
use std::path::Path;

use assay::credit_event::{ClaimClasses, CreditEvent};
use assay::work::WorkId;

use crate::reward::{RewardAct, RewardBook};

/// Magic for xylarium datum court store.
pub const MAGIC: &[u8; 4] = b"XDCT";

/// Current store version. Version 2 added a per-Credited O1 payout
/// field; version 1 snapshots still load (payout defaults to 0 — an
/// old record simply never recorded its rebate), so a court does not
/// have to discard a pre-payout book to upgrade.
pub const VERSION: u8 = 2;

/// The last version whose Credited records carried no payout field.
const VERSION_NO_PAYOUT: u8 = 1;

/// Act tag: successful credit.
const TAG_CREDITED: u8 = 1;
/// Act tag: epoch opened (D-L7).
const TAG_EPOCH_OPEN: u8 = 2;
/// Act tag: epoch closed (D-L7).
const TAG_EPOCH_CLOSED: u8 = 3;
/// O3 — a settled refinement: old ≈ new, with the measured savings.
const TAG_EQUIVALENT: u8 = 4;

/// Why encode/decode/load refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreBroken {
    /// Buffer ended mid-field.
    Truncated,
    /// Wrong magic.
    Magic,
    /// Unsupported version.
    Version(u8),
    /// Unknown act tag.
    Tag(u8),
    /// I/O failed (path ops).
    Io,
    /// Empty axes on a stored act.
    EmptyAxes,
    /// Snapshot contained a duplicate work_id.
    DuplicateWork,
    /// Bytes remain after a complete snapshot.
    Trailing,
}

impl From<std::io::Error> for StoreBroken {
    fn from(_: std::io::Error) -> Self {
        StoreBroken::Io
    }
}

/// Encode a [`RewardBook`] to durable bytes (acts only).
pub fn encode(book: &RewardBook) -> Vec<u8> {
    let acts = book.acts();
    let mut out = Vec::with_capacity(16 + acts.len().saturating_mul(64));
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    put_u32(&mut out, acts.len() as u32);
    for act in acts {
        match act {
            RewardAct::Credited { credit, event } => {
                out.push(TAG_CREDITED);
                put_u64(&mut out, event.transport);
                out.push(classes_byte(event.classes));
                let id = event.work_id.as_bytes();
                put_u32(&mut out, id.len() as u32);
                out.extend_from_slice(id);
                put_u32(&mut out, event.axes.len() as u32);
                for a in &event.axes {
                    put_u128(&mut out, *a);
                }
                // version 2: the O1 yield rebate, durable at last.
                put_u128(&mut out, credit.payout);
            }
            RewardAct::EpochOpened { epoch, label } => {
                out.push(TAG_EPOCH_OPEN);
                put_u64(&mut out, *epoch);
                let b = label.as_bytes();
                put_u32(&mut out, b.len() as u32);
                out.extend_from_slice(b);
            }
            RewardAct::EpochClosed {
                epoch,
                credits_in_epoch,
            } => {
                out.push(TAG_EPOCH_CLOSED);
                put_u64(&mut out, *epoch);
                put_u64(&mut out, *credits_in_epoch);
            }
            RewardAct::Equivalent {
                old,
                new,
                saved_fuel,
                saved_bytes,
            } => {
                out.push(TAG_EQUIVALENT);
                let ob = old.as_bytes();
                put_u32(&mut out, u32::try_from(ob.len()).unwrap_or(u32::MAX));
                out.extend_from_slice(ob);
                let nb = new.as_bytes();
                put_u32(&mut out, u32::try_from(nb.len()).unwrap_or(u32::MAX));
                out.extend_from_slice(nb);
                put_u64(&mut out, *saved_fuel);
                put_u64(&mut out, *saved_bytes);
            }
        }
    }
    out
}

/// Decode a durable snapshot into a [`RewardBook`].
pub fn decode(bytes: &[u8]) -> Result<RewardBook, StoreBroken> {
    if bytes.len() < 5 {
        return Err(StoreBroken::Truncated);
    }
    if bytes.get(0..4) != Some(MAGIC.as_slice()) {
        return Err(StoreBroken::Magic);
    }
    let mut i = 4usize;
    let ver = *bytes.get(i).ok_or(StoreBroken::Truncated)?;
    i += 1;
    if ver != VERSION && ver != VERSION_NO_PAYOUT {
        return Err(StoreBroken::Version(ver));
    }
    let has_payout = ver >= VERSION;
    let n = take_u32(bytes, &mut i)?;
    let mut book = RewardBook::new();
    for _ in 0..n {
        let tag = *bytes.get(i).ok_or(StoreBroken::Truncated)?;
        i += 1;
        match tag {
            TAG_CREDITED => {
                let transport = take_u64(bytes, &mut i)?;
                let classes = classes_from_byte(*bytes.get(i).ok_or(StoreBroken::Truncated)?);
                i += 1;
                let id_len = take_u32(bytes, &mut i)? as usize;
                let id_bytes = bytes
                    .get(i..i.saturating_add(id_len))
                    .ok_or(StoreBroken::Truncated)?;
                let work_id = WorkId::from_bytes(id_bytes.to_vec());
                i += id_len;
                let n_axes = take_u32(bytes, &mut i)? as usize;
                if n_axes == 0 {
                    return Err(StoreBroken::EmptyAxes);
                }
                let mut axes = Vec::with_capacity(n_axes);
                for _ in 0..n_axes {
                    axes.push(take_u128(bytes, &mut i)?);
                }
                let payout = if has_payout {
                    take_u128(bytes, &mut i)?
                } else {
                    0
                };
                let event = CreditEvent::with_classes(work_id, transport, axes, classes);
                book.admit_event_priced(event, payout).map_err(|e| match e {
                    crate::reward::RewardRefused::Replay { .. } => StoreBroken::DuplicateWork,
                    crate::reward::RewardRefused::OpenWork => StoreBroken::EmptyAxes,
                    _ => StoreBroken::EmptyAxes,
                })?;
            }
            TAG_EPOCH_OPEN => {
                let epoch = take_u64(bytes, &mut i)?;
                let lab_len = take_u32(bytes, &mut i)? as usize;
                let lab_bytes = bytes
                    .get(i..i.saturating_add(lab_len))
                    .ok_or(StoreBroken::Truncated)?;
                let label = std::str::from_utf8(lab_bytes)
                    .map_err(|_| StoreBroken::Truncated)?
                    .to_string();
                i += lab_len;
                book.restore_epoch_opened(epoch, label);
            }
            TAG_EPOCH_CLOSED => {
                let epoch = take_u64(bytes, &mut i)?;
                let credits_in_epoch = take_u64(bytes, &mut i)?;
                book.restore_epoch_closed(epoch, credits_in_epoch);
            }
            TAG_EQUIVALENT => {
                let ol = take_u32(bytes, &mut i)? as usize;
                let ob = bytes
                    .get(i..i.saturating_add(ol))
                    .ok_or(StoreBroken::Truncated)?;
                let old = WorkId::from_bytes(ob.to_vec());
                i = i.saturating_add(ol);
                let nl = take_u32(bytes, &mut i)? as usize;
                let nb = bytes
                    .get(i..i.saturating_add(nl))
                    .ok_or(StoreBroken::Truncated)?;
                let new = WorkId::from_bytes(nb.to_vec());
                i = i.saturating_add(nl);
                let saved_fuel = take_u64(bytes, &mut i)?;
                let saved_bytes = take_u64(bytes, &mut i)?;
                let _ = book.record_equivalence(old, new, saved_fuel, saved_bytes);
            }
            other => return Err(StoreBroken::Tag(other)),
        }
    }
    if i != bytes.len() {
        return Err(StoreBroken::Trailing);
    }
    Ok(book)
}

/// Write book to a path (create/overwrite).
pub fn write(path: &Path, book: &RewardBook) -> Result<(), StoreBroken> {
    fs::write(path, encode(book))?;
    Ok(())
}

/// Load book from a path.
pub fn load(path: &Path) -> Result<RewardBook, StoreBroken> {
    let bytes = fs::read(path)?;
    decode(&bytes)
}

fn classes_byte(c: ClaimClasses) -> u8 {
    let mut b = 0u8;
    if c.game_capacity {
        b |= 1;
    }
    if c.edge_payout {
        b |= 2;
    }
    b
}

fn classes_from_byte(b: u8) -> ClaimClasses {
    ClaimClasses {
        game_capacity: b & 1 != 0,
        edge_payout: b & 2 != 0,
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u128(out: &mut Vec<u8>, v: u128) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn take_u32(bytes: &[u8], i: &mut usize) -> Result<u32, StoreBroken> {
    let end = i.saturating_add(4);
    let slice = bytes.get(*i..end).ok_or(StoreBroken::Truncated)?;
    *i = end;
    let mut arr = [0u8; 4];
    arr.copy_from_slice(slice);
    Ok(u32::from_le_bytes(arr))
}

fn take_u64(bytes: &[u8], i: &mut usize) -> Result<u64, StoreBroken> {
    let end = i.saturating_add(8);
    let slice = bytes.get(*i..end).ok_or(StoreBroken::Truncated)?;
    *i = end;
    let mut arr = [0u8; 8];
    arr.copy_from_slice(slice);
    Ok(u64::from_le_bytes(arr))
}

fn take_u128(bytes: &[u8], i: &mut usize) -> Result<u128, StoreBroken> {
    let end = i.saturating_add(16);
    let slice = bytes.get(*i..end).ok_or(StoreBroken::Truncated)?;
    *i = end;
    let mut arr = [0u8; 16];
    arr.copy_from_slice(slice);
    Ok(u128::from_le_bytes(arr))
}

#[cfg(test)]
mod tests {
    // Tests are allowed to panic: a test that cannot reach its subject
    // must say so loudly rather than pass quietly.
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::reward::{triangle_claim, RewardRefused};

    #[test]
    fn round_trip_preserves_replay() {
        let mut book = RewardBook::new();
        let body = triangle_claim(3).encode();
        book.credit_claim(&body).expect("credit");
        let bytes = encode(&book);
        let loaded = decode(&bytes).expect("decode");
        assert_eq!(loaded.act_len(), 1);
        assert_eq!(loaded.total().components(), book.total().components());
        assert!(matches!(
            loaded.clone().credit_claim(&triangle_claim(9).encode()),
            Err(RewardRefused::Replay { .. })
        ));
    }

    #[test]
    fn bad_magic_refuses() {
        assert!(matches!(decode(b"XXXX\x01"), Err(StoreBroken::Magic)));
    }
}
