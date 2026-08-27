//! Opaque POW++ claim envelopes on the superhighway.
//!
//! Tags sit in the assay band (`IS-3`: 80–127). This crate **frames and
//! forwards** only — it never calls a prover and never decides whether
//! a claim is true. Verification is the verifier node's job (via
//! `assay`); settlement is the authority's (`datum`).
//!
//! ```text
//! boundary claim  tag 80  ‖ body (domain=1 …)
//! receipt         tag 81  ‖ body opaque
//! shape claim     tag 82  ‖ body (domain=2 …)   PoUW Shape domain
//! ```

use crate::frame::{self, Malformed, Reader};
use crate::layout::{Layout, Tag};

/// Boundary / flux work claim body. In the assay grant (80–127).
pub const CLAIM_TAG: Tag = 80;

/// Watcher receipt body. In the assay grant (80–127).
pub const RECEIPT_TAG: Tag = 81;

/// Shape-domain PoUW claim body (orbs, edges, charges). Assay grant.
pub const SHAPE_CLAIM_TAG: Tag = 82;

/// Whether a tag is an isthmus-framed POW++ envelope (not a proof check).
#[must_use]
pub fn is_work_tag(tag: Tag) -> bool {
    tag == CLAIM_TAG || tag == RECEIPT_TAG || tag == SHAPE_CLAIM_TAG
}

/// Write a boundary claim as a length-prefixed frame. Body is opaque.
pub fn put_claim(body: &[u8], out: &mut Vec<u8>) -> Result<(), Malformed> {
    frame::put_frame(&Layout::founding(), CLAIM_TAG, body, out)
}

/// Write a shape claim as a length-prefixed frame. Body is opaque.
pub fn put_shape_claim(body: &[u8], out: &mut Vec<u8>) -> Result<(), Malformed> {
    frame::put_frame(&Layout::founding(), SHAPE_CLAIM_TAG, body, out)
}

/// Write a receipt as a length-prefixed frame. Body is opaque.
pub fn put_receipt(body: &[u8], out: &mut Vec<u8>) -> Result<(), Malformed> {
    frame::put_frame(&Layout::founding(), RECEIPT_TAG, body, out)
}

/// Read one founding-layout frame: `(tag, value)`.
pub fn take_frame(bytes: &[u8]) -> Result<(Tag, &[u8]), Malformed> {
    let mut reader = Reader::new(bytes);
    let (tag, value) = reader.frame(&Layout::founding())?;
    reader.finish()?;
    Ok((tag, value))
}

/// Carrier path: if the head frame is not a work tag this peer owns,
/// return the whole record for forwarding; if it is owned, return the
/// body for a higher layer (still without verifying it here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envelope<'a> {
    /// Tag this layer treats as work; body for verifier/producer code.
    Mine {
        /// Claim or receipt tag.
        tag: Tag,
        /// Opaque body.
        body: &'a [u8],
    },
    /// Not ours — forward these exact bytes (header + value).
    Forward {
        /// Whole record including header.
        whole: &'a [u8],
    },
}

/// Classify one founding-layout record at the head of `bytes`.
///
/// Does **not** re-derive work. A carrier that only ever returns
/// [`Envelope::Forward`] is a lawful linking mesh for work tags it does
/// not implement.
pub fn classify<'a>(bytes: &'a [u8]) -> Result<Envelope<'a>, Malformed> {
    let layout = Layout::founding();
    let mut reader = Reader::new(bytes);
    let (tag, value) = reader.frame(&layout)?;
    let header = layout.header();
    let whole_len = header.saturating_add(value.len());
    let whole = bytes.get(..whole_len).ok_or(Malformed::Truncated {
        want: whole_len,
        have: bytes.len(),
    })?;
    if is_work_tag(tag) {
        Ok(Envelope::Mine { tag, body: value })
    } else {
        Ok(Envelope::Forward { whole })
    }
}
