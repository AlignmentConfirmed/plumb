//! Transport replay hygiene — **secondary** to useful-work identity (H7 / IS-2 §6).
//!
//! ## What this is not
//!
//! - **Not a session seen-set.** `IS-2` §6.1: the session owes no sequence
//!   number, no window, and no seen-set. Claims re-derive; replaying a
//!   claim changes nothing.
//! - **Not PoUW identity.** Credit keys on [`crate::reward::RewardBook`]
//!   use `work_id` (structure). A different transport field with the same
//!   structure still refuses at the book — that is H1, not this module.
//!
//! ## What this is
//!
//! **Exact wire-byte re-injection** of an **effectful** payload at the
//! authority/adapter: the same bytes applied twice as an effect. The
//! carrier may deliver them again; the court may choose to drop the
//! second delivery without re-entering the credit path.
//!
//! ```text
//! claim frame     re-derive always        no hygiene required
//! effect payload  work_id (primary)     + optional wire digest (secondary)
//! ```
//!
//! Digests are FNV-1a over the opaque body (or whole frame). **Not a
//! recommendation for production collision resistance** — same role as
//! the uplink example's toy digest: deterministic, dependency-free,
//! sufficient to pin identical-byte replay in tests.

use std::collections::HashSet;

/// Why transport hygiene refused a delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HygieneRefused {
    /// These exact bytes were already admitted as an effect on this filter.
    WireReplay {
        /// FNV-1a of the payload.
        digest: u64,
    },
}

/// Secondary filter: identical effect payloads only once per filter lifetime.
#[derive(Debug, Default, Clone)]
pub struct WireHygiene {
    seen: HashSet<u64>,
}

impl WireHygiene {
    /// Empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many distinct effect payloads have been admitted.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether the filter has admitted nothing.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Digest used for identity of a wire payload (stable, pure).
    pub fn digest(bytes: &[u8]) -> u64 {
        fnv1a64(bytes)
    }

    /// Admit an **effectful** payload once. Second identical byte string refuses.
    ///
    /// Does **not** inspect structure. Two encodings of the same useful
    /// work with different transport fields have different digests here
    /// and both pass — the book still refuses the second on `work_id`.
    pub fn admit_effect(&mut self, payload: &[u8]) -> Result<u64, HygieneRefused> {
        let digest = Self::digest(payload);
        if !self.seen.insert(digest) {
            return Err(HygieneRefused::WireReplay { digest });
        }
        Ok(digest)
    }

    /// Whether this exact payload would be a wire replay (without inserting).
    pub fn would_replay(&self, payload: &[u8]) -> bool {
        self.seen.contains(&Self::digest(payload))
    }
}

/// Credit path with secondary wire hygiene in front of the book.
///
/// Order: wire digest once → structure `work_id` once.
pub fn credit_effect(
    hygiene: &mut WireHygiene,
    book: &mut crate::reward::RewardBook,
    body: &[u8],
) -> Result<crate::reward::Credit, CreditEffectRefused> {
    hygiene
        .admit_effect(body)
        .map_err(CreditEffectRefused::Wire)?;
    book.credit_claim(body).map_err(CreditEffectRefused::Work)
}

/// Refusal from the dual credit path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditEffectRefused {
    /// Identical wire bytes already applied.
    Wire(HygieneRefused),
    /// Structure credit path refused (open work, work_id replay, …).
    Work(crate::reward::RewardRefused),
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::reward::{closed_box_claim, RewardBook, RewardRefused};

    #[test]
    fn identical_wire_effect_replays_at_hygiene() {
        let mut h = WireHygiene::new();
        let body = closed_box_claim(1, 1).encode();
        h.admit_effect(&body).expect("first");
        match h.admit_effect(&body) {
            Err(HygieneRefused::WireReplay { digest }) => {
                assert_eq!(digest, WireHygiene::digest(&body));
            }
            other => panic!("expected wire replay, got {other:?}"),
        }
    }

    #[test]
    fn different_transport_passes_hygiene_but_work_id_catches() {
        let mut h = WireHygiene::new();
        let mut book = RewardBook::new();
        let a = closed_box_claim(1, 1).encode();
        let b = closed_box_claim(99, 1).encode();
        credit_effect(&mut h, &mut book, &a).expect("first");
        // Different bytes → hygiene admits
        match credit_effect(&mut h, &mut book, &b) {
            Err(CreditEffectRefused::Work(RewardRefused::Replay { .. })) => {}
            other => panic!("expected work_id replay after hygiene pass, got {other:?}"),
        }
        assert_eq!(h.len(), 2, "two wire digests admitted");
    }

    #[test]
    fn claim_rederive_needs_no_hygiene() {
        // Documented stance: claims re-derive; filter stays empty.
        let h = WireHygiene::new();
        assert!(h.is_empty());
    }
}
