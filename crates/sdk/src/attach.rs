//! Declare yourself on an edge, and agree on what is spoken (IS-5).
//!
//! The first record on an edge **is** the declaration — position, not
//! number. This module builds it from chain state and decides whether
//! two declarations can talk.

use isthmus::deed::Ledger;
use isthmus::frame::put_frame;
use isthmus::hello::Hello;
use isthmus::layout::{Layout, Tag};
use isthmus::Malformed;

/// Build this kernel's declaration from what the chain shows it holds.
///
/// `bound` is the largest record value this deployment will accept —
/// measured, not guessed. The declaration carries only live deeds for
/// `holder`; an empty range list is a valid declaration by a party
/// that holds nothing yet.
pub fn declare(ledger: &Ledger, holder: &str, bound: u32) -> Hello {
    Hello::of(ledger, holder, bound)
}

/// Frame a declaration for the wire, under a tag the declarer holds.
///
/// The tag comes from the caller's own grant (see
/// [`grant::holdings`](crate::grant::holdings)) because the substrate
/// reserves no global hello tag — reserving one would carve a tag out
/// of every edge forever so the negotiation could refer to itself.
pub fn wire(layout: &Layout, tag: Tag, hello: &Hello) -> Result<Vec<u8>, Malformed> {
    let mut out = Vec::new();
    put_frame(layout, tag, &hello.encode(), &mut out)?;
    Ok(out)
}

/// What two declarations agreed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreement {
    /// Revision strings both sides implement. Equality, never order.
    pub revisions: Vec<String>,
    /// The record bound the session runs under: the smaller of the two
    /// declared bounds, because a record either side would refuse is a
    /// record the session cannot carry.
    pub bound: u32,
}

/// Why an attachment did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachRefused {
    /// No revision string in common. Not an error in either party —
    /// two peers on different revisions disagree about what a frame
    /// means, and neither is wrong. They just cannot talk.
    NoSharedRevision {
        /// What we declared.
        ours: Vec<String>,
        /// What they declared.
        theirs: Vec<String>,
    },
}

/// Decide whether two declarations can hold a session.
pub fn agree(ours: &Hello, theirs: &Hello) -> Result<Agreement, AttachRefused> {
    let revisions = ours.shared_revisions(theirs);
    if revisions.is_empty() {
        return Err(AttachRefused::NoSharedRevision {
            ours: ours.revisions.clone(),
            theirs: theirs.revisions.clone(),
        });
    }
    Ok(Agreement {
        revisions,
        bound: ours.max_record.min(theirs.max_record),
    })
}

#[cfg(test)]
mod tests {
    // Tests are allowed to panic: a test that cannot reach its subject
    // must say so loudly rather than pass quietly.
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn same_build_agrees_with_itself() {
        let ledger = Ledger::new(Layout::founding());
        let ours = declare(&ledger, "kernel-a", 1 << 16);
        let theirs = declare(&ledger, "kernel-b", 1 << 12);
        let pact = agree(&ours, &theirs).expect("same revisions");
        assert_eq!(pact.bound, 1 << 12, "the smaller bound rules");
        assert!(!pact.revisions.is_empty());
    }

    #[test]
    fn disjoint_revisions_refuse_without_blame() {
        let ledger = Ledger::new(Layout::founding());
        let ours = declare(&ledger, "kernel-a", 1 << 16);
        let theirs = Hello {
            revisions: vec!["XX-9/9".into()],
            ..Hello::default()
        };
        match agree(&ours, &theirs) {
            Err(AttachRefused::NoSharedRevision { .. }) => {}
            other => panic!("expected refusal, got {other:?}"),
        }
    }
}
