//! THE VERTICAL — Act::Anchor across independent chains.
//!
//! Horizontal: one chain's acts, totally ordered because one party appends.
//! Vertical: Anchor { chain, height, digest } — observation of another
//! chain's prefix. Frontiers join by per-chain max; concurrent is None.
//!
//! Pins IS-6 §7 vector C8 and the sphere-frontier behaviour the multi-axial
//! ledger requires. ENFORCED, not prose.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use isthmus::deed::chain::{self, ANCHOR};
use isthmus::deed::{Act, Ledger};
use isthmus::frame::Reader;
use isthmus::layout::Layout;
use isthmus::sphere::Frontier;
use std::cmp::Ordering;

/// IS-6 §7 published vector for Anchor (chain test C8).
const C8_HEX: &str = "\
08240000000500736f757468\
0f00000000000000\
080000000102030405060708\
070073657373696f6e";

fn c8_act() -> Act {
    Act::Anchor {
        chain: "south".to_owned(),
        height: 15,
        digest: vec![1, 2, 3, 4, 5, 6, 7, 8],
        witnessed: "session".to_owned(),
    }
}

#[test]
fn anchor_vector_round_trips_and_matches_is6() {
    let act = c8_act();
    let bytes = chain::encode(std::slice::from_ref(&act));
    let published: String = C8_HEX.chars().filter(|c| !c.is_whitespace()).collect();
    let produced: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(produced, published, "codec drifted from IS-6 §7 C8");
    assert_eq!(chain::decode(&bytes), Ok(vec![act]));

    // Tag is the vertical act tag.
    let mut reader = Reader::new(&bytes);
    let (tag, _) = reader.frame(&Layout::founding()).expect("frame");
    assert_eq!(tag, ANCHOR);
}

#[test]
fn frontier_records_vertical_observation() {
    let mut court = Ledger::new(Layout::founding()).under("north");
    court.anchor("south", 15, &[1, 2, 3, 4, 5, 6, 7, 8], "session");
    let f = court.frontier();
    assert_eq!(f.height_of("north"), 1); // one act on this chain
    assert_eq!(f.height_of("south"), 15);
    assert!(f.chains().contains(&"south"));
}

#[test]
fn frontiers_join_by_per_chain_max_and_concurrency_is_none() {
    let mut a = Frontier::new();
    a.observe("datum", 10);
    a.observe("strand", 3);

    let mut b = Frontier::new();
    b.observe("datum", 7);
    b.observe("strand", 9);

    let j = a.join(&b);
    assert_eq!(j.height_of("datum"), 10);
    assert_eq!(j.height_of("strand"), 9);

    // a saw more datum, b saw more strand → concurrent
    assert!(a.concurrent_with(&b));
    assert_eq!(a.compare(&b), None);

    // after join, both are ≤ join
    assert_eq!(a.compare(&j), Some(Ordering::Less));
    assert_eq!(b.compare(&j), Some(Ordering::Less));
}

#[test]
fn unknown_chain_act_refuses_not_skips() {
    // A chain of only a known Anchor decodes; a foreign tag mid-chain refuses.
    let good = chain::encode(&[c8_act()]);
    assert!(chain::decode(&good).is_ok());

    // Corrupt: flip act tag to 99 (not a chain act).
    let mut bad = good;
    if let Some(t) = bad.get_mut(0) {
        *t = 99;
    }
    assert!(
        chain::decode(&bad).is_err(),
        "unknown act must refuse so history cannot be silently misfolded"
    );
}
