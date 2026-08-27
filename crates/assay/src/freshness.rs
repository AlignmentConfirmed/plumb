//! Work-identity freshness — **credit once per structure**.
//!
//! ## Theorem (work_id-primary reward)
//!
//! Useful work has intrinsic identity: the content address of its
//! structure ([`crate::WorkId`]). Transport fields (nonces, session
//! tags) are **not** part of that identity.
//!
//! Therefore a credit ledger keyed by [`WorkId`] admits each structure
//! **at most once**. A second presentation of the same structure —
//! even with a different transport tag — is a **replay**, not new work.
//!
//! ```text
//! claim.transport differs ∧ claim.work_id equal  ⇒  same credit key
//! ledger.admit(id) twice                         ⇒  second refuses
//! ```
//!
//! This module holds the pure set-algebra of that rule. Courts (datum)
//! and market edges (xylarium `edge`) apply it; they do not re-derive it.
//!
//! ## What this is not
//!
//! - Not multi-axial cover (`credit ≽ price`) — that is extent/flux.
//! - Not PoUW verification — call [`crate::work::WorkBody::verifies`].
//! - Not a token or mint — only “has this id been seen.”

use crate::WorkId;
use std::collections::BTreeSet;

/// Why a second credit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replay {
    /// This [`WorkId`] was already admitted.
    AlreadyCredited {
        /// The repeated work identity.
        work_id: WorkId,
    },
}

/// Pure once-credit book: each [`WorkId`] at most once.
///
/// Order of admission is recorded for diagnostics; equality of the set
/// is the law.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OnceCredit {
    credited: BTreeSet<WorkId>,
}

impl OnceCredit {
    /// Empty book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this work identity has already been credited.
    pub fn contains(&self, work_id: &WorkId) -> bool {
        self.credited.contains(work_id)
    }

    /// How many distinct work identities have been credited.
    pub fn len(&self) -> usize {
        self.credited.len()
    }

    /// Whether nothing has been credited.
    pub fn is_empty(&self) -> bool {
        self.credited.is_empty()
    }

    /// Admit `work_id` if new; refuse if already present (**replay**).
    pub fn admit(&mut self, work_id: WorkId) -> Result<(), Replay> {
        if self.credited.contains(&work_id) {
            return Err(Replay::AlreadyCredited { work_id });
        }
        self.credited.insert(work_id);
        Ok(())
    }
}

/// **Cover theorem (capacity form, scalar micros for external books).**
///
/// External payouts use integer micros. A payout of `want` is covered
/// exactly when `cover >= want`. This is the 1-axis case of multi-axial
/// `credit ≽ price` (POW++): one axis, exact integers, no floats.
#[inline]
pub fn cover_suffices(cover: u64, want: u64) -> bool {
    cover >= want
}

/// Split `total` across `n` equal shares (pro-rata of equal weight).
///
/// Remainder micros stay unpaid (dust) — never invent fractional micros.
/// Returns `None` if `n == 0`.
pub fn equal_share(total: u64, n: u64) -> Option<u64> {
    total.checked_div(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::Claim;
    use crate::{whole, Boundary, Facet, Orientation};

    fn closed(nonce: u64) -> Claim {
        let mut b = Boundary::new(2);
        let f = whole(3);
        assert!(b.face(Facet::new(0, Orientation::Low, f.clone())));
        assert!(b.face(Facet::new(0, Orientation::High, f.clone())));
        assert!(b.face(Facet::new(1, Orientation::Low, f.clone())));
        assert!(b.face(Facet::new(1, Orientation::High, f)));
        Claim::new(nonce, b)
    }

    #[test]
    fn transport_differs_work_id_equal_is_one_credit() {
        let a = closed(1).work_id();
        let b = closed(99).work_id();
        assert_eq!(a, b);
        let mut book = OnceCredit::new();
        assert!(book.admit(a.clone()).is_ok());
        assert_eq!(
            book.admit(b),
            Err(Replay::AlreadyCredited { work_id: a })
        );
        assert_eq!(book.len(), 1);
    }

    #[test]
    fn distinct_structure_earns_two_credits() {
        let a = closed(0).work_id();
        let mut boundary = Boundary::new(2);
        let f = whole(9);
        assert!(boundary.face(Facet::new(0, Orientation::Low, f.clone())));
        assert!(boundary.face(Facet::new(0, Orientation::High, f.clone())));
        assert!(boundary.face(Facet::new(1, Orientation::Low, f.clone())));
        assert!(boundary.face(Facet::new(1, Orientation::High, f)));
        let b = Claim::new(0, boundary).work_id();
        assert_ne!(a, b);
        let mut book = OnceCredit::new();
        assert!(book.admit(a).is_ok());
        assert!(book.admit(b).is_ok());
        assert_eq!(book.len(), 2);
    }

    #[test]
    fn cover_and_share_are_exact() {
        assert!(cover_suffices(100, 100));
        assert!(!cover_suffices(99, 100));
        assert_eq!(equal_share(100, 3), Some(33));
        assert_eq!(equal_share(100, 0), None);
    }
}
