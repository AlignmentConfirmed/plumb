//! `IS-5` — a declaration states, it does not agree.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::deed::Ledger;
use isthmus::frame::{put_frame, Reader};
use isthmus::hello::{expects_declaration, Hello};
use isthmus::layout::Layout;

/// This test's own bound. There is no crate-wide one — see
/// `session.rs`'s note on why a measurement of one deployment's corpus
/// does not belong in a protocol crate.
const BOUND: u32 = 1 << 20;

/// An edge with the founding encumbrances and one deed, built at
/// runtime. Nothing here is a constant the crate carries.
fn edge_with(holder: &str, width: u128) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "both registries");
    ledger.issue(holder, width).expect("room on a fresh edge");
    ledger
}

#[test]
fn a_declaration_round_trips_through_a_record() {
    let ledger = edge_with("me", 16);
    let hello = Hello::of(&ledger, "me", BOUND);

    // The declaration is the FIRST record on the edge. Its tag is
    // whatever this edge deeded, not a reserved global number.
    let deed = &ledger.deeds()[0];
    let mut wire = Vec::new();
    put_frame(ledger.layout(), deed.low(), &hello.encode(), &mut wire).expect("fits");

    let mut reader = Reader::new(&wire);
    let (tag, value) = reader.frame(ledger.layout()).expect("a well-formed record");
    assert_eq!(tag, deed.low(), "the edge chose this number, not the crate");

    let back = Hello::decode(value).expect("its own bytes");
    assert_eq!(back, hello);
    assert!(reader.is_done());
}

/// The bootstrap rule, whole: **position, not number.**
///
/// An edge that has read nothing expects a declaration. An edge that has
/// read anything does not. No tag is carved out of any edge to let the
/// negotiation refer to itself.
#[test]
fn the_first_record_on_an_edge_is_the_declaration() {
    assert!(expects_declaration(0));
    for read in 1usize..64 {
        assert!(
            !expects_declaration(read),
            "record {read} was still treated as the opening declaration"
        );
    }
}

/// A declaration names the deeds its sender holds **on this edge**, so
/// the same peer declares different numbers on different edges.
#[test]
fn what_a_peer_declares_is_a_property_of_the_edge() {
    let mut quiet = Ledger::new(Layout::founding());
    let mut busy = Ledger::new(Layout::founding());
    busy.encumber(1, 90, "a busier neighbour", "their advert");

    quiet.issue("chitin", 16).expect("room");
    busy.issue("chitin", 16).expect("room");

    let on_quiet = Hello::of(&quiet, "chitin", BOUND);
    let on_busy = Hello::of(&busy, "chitin", BOUND);

    assert_ne!(
        on_quiet.ranges, on_busy.ranges,
        "the same holder declared identical ranges on two different edges \
         — then the numbering is global after all"
    );
    assert_eq!(on_quiet.revisions, on_busy.revisions, "revisions are not per-edge");
}

/// A truncated declaration is not a partial one. A peer acting on half
/// a declaration acts on terms the sender did not state.
#[test]
fn a_truncated_declaration_refuses_rather_than_reading_what_arrived() {
    let full = Hello::of(&edge_with("me", 16), "me", BOUND).encode();
    for cut in 0..full.len() {
        assert!(
            Hello::decode(&full[..cut]).is_err(),
            "a {cut}-byte prefix decoded — that is half a declaration acted on"
        );
    }
    assert!(Hello::decode(&full).is_ok(), "and the whole thing decodes");
}

#[test]
fn trailing_bytes_are_not_this_declaration() {
    let mut over = Hello::of(&edge_with("me", 16), "me", BOUND).encode();
    over.push(0);
    assert!(Hello::decode(&over).is_err());
}

/// Revisions are compared for **equality and never ordered**. Ordering
/// would let a peer decide it is ahead and act on the difference, which
/// is authority this substrate does not have.
#[test]
fn two_peers_on_different_revisions_are_both_right() {
    let a = Hello {
        revisions: vec!["IS-1/1".into(), "IS-2/1".into()],
        ranges: vec![(64, 79)],
        max_record: 1 << 20,
        ..Default::default()
    };
    let b = Hello {
        revisions: vec!["IS-1/2".into(), "IS-2/1".into()],
        ranges: vec![(192, 199)],
        max_record: 1 << 16,
        ..Default::default()
    };

    assert_eq!(a.shared_revisions(&b), vec!["IS-2/1".to_string()]);
    assert_eq!(b.shared_revisions(&a), a.shared_revisions(&b), "symmetric");

    // Sharing nothing is not an error. The peers still exchange records;
    // each forwards what the other owns.
    let c = Hello {
        revisions: vec!["NS-9/4".into()],
        ..Default::default()
    };
    assert!(a.shared_revisions(&c).is_empty());
}

/// A peer that speaks less is **limited, never refused**.
#[test]
fn a_peer_that_declares_nothing_still_connects() {
    let silent = Hello::default();
    for tag in 0u64..=255 {
        assert!(!silent.reads(tag), "it claims nothing");
    }
    // The only thing its empty declaration changes is what gets
    // forwarded rather than read. There is no verdict that rejects it.
    assert_eq!(Hello::bound_for(Some(&silent), 4096), 0);
    assert_eq!(
        Hello::bound_for(None, 4096),
        4096,
        "no declaration heard means the CALLER's fallback, not a crate default"
    );
}

#[test]
fn declared_ranges_decide_what_a_peer_reads() {
    let peer = Hello {
        ranges: vec![(192, 199)],
        ..Default::default()
    };
    assert!(peer.reads(192));
    assert!(peer.reads(199));
    assert!(!peer.reads(191));
    assert!(!peer.reads(200));
}

/// A range outside the one-byte tag space is not a range.
#[test]
fn an_impossible_range_refuses() {
    let with = |ranges: Vec<(u64, u64)>| Hello {
        ranges,
        ..Default::default()
    };

    // Inverted is not a range.
    assert!(Hello::decode(&with(vec![(80, 64)]).encode()).is_err());

    // And the gate passes. Note `(0, 300)` is now ACCEPTED: a range
    // above 255 is only impossible on a one-byte layout, and refusing it
    // here asserted the tag width in a third place. A peer on a wider
    // layout declaring it is telling the truth.
    assert!(Hello::decode(&with(vec![(0, 300)]).encode()).is_ok());
    assert!(Hello::decode(&with(vec![(64, 79)]).encode()).is_ok());
    assert!(Hello::decode(&with(vec![(64, 64)]).encode()).is_ok());
}
