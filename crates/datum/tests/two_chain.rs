//! Local two-chain anchor demo — highway beta gate.
//!
//! ```text
//! south  independent ledger (in-process)
//!   → Hello + Uplink declaration over IS-5/2 framing
//! north  independent ledger records ONE vertical Anchor
//!   → sphere::confirms against south's actual prefix
//!   → frontiers join / compare; concurrent until reciprocal
//!   → chain encode round-trips
//! ```
//!
//! Read-only of disk: both chains are built in-process so the gate does
//! not depend on a live founding file. The example `examples/uplink.rs`
//! still walks the authority for operators; this file is the pin.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use isthmus::deed::chain;
use isthmus::deed::{Act, Ledger};
use isthmus::hello::{Hello, Uplink};
use isthmus::layout::Layout;
use isthmus::sphere::{self, Frontier};
use std::cmp::Ordering;

/// Deterministic toy digest (same as `examples/uplink.rs`). Not a
/// security recommendation — confirms takes the function in.
fn fnv(bytes: &[u8]) -> Vec<u8> {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash.to_le_bytes().to_vec()
}

fn named_estate(name: &str, holder: &str, width: u128) -> Ledger {
    let mut court = Ledger::new(Layout::founding()).under(name);
    court
        .issue(holder, width)
        .unwrap_or_else(|why| panic!("{name} could not issue estate: {why:?}"));
    court
}

// ===================================================================
// full local demo
// ===================================================================

#[test]
fn two_independent_chains_anchor_and_confirm() {
    let south = named_estate("south", "producer", 8);
    assert_eq!(south.name(), Some("south"));
    assert!(south.height() >= 1, "south must hold at least the Open");

    let mut north = named_estate("north", "newcomer", 8);
    assert_eq!(north.name(), Some("north"));

    // South declares who it is (IS-5/2 uplink).
    let declared = Hello::of(&south, "isthmus", 1 << 20).declaring(Uplink::of(&south, fnv));
    let bytes = declared.encode();
    let heard = Hello::decode(&bytes).expect("declaration survives the wire");
    let uplink = heard
        .uplink
        .as_ref()
        .expect("south declared an uplink");
    assert_eq!(uplink.chain, "south");
    assert_eq!(uplink.height(), south.height());
    assert_eq!(uplink.digest, fnv(&chain::encode(south.acts())));

    // North records ONE vertical over south's own chain.
    let height_before = north.height();
    let act = uplink.anchor("local two-chain demo");
    north.record(act);
    assert_eq!(
        north.height(),
        height_before + 1,
        "north gained exactly one vertical act"
    );

    let vertical = north.acts().last().expect("anchor landed");
    assert!(
        matches!(vertical, Act::Anchor { chain, .. } if chain == "south"),
        "last act must be the south anchor"
    );

    // Digest truth against the observed chain.
    assert_eq!(
        sphere::confirms(vertical, &south, fnv),
        Some(true),
        "anchor must confirm against south's prefix"
    );

    // A forged digest is false, not unanswerable.
    let mut forged = vertical.clone();
    if let Act::Anchor { digest, .. } = &mut forged {
        digest.fill(0xFF);
    }
    assert_eq!(
        sphere::confirms(&forged, &south, fnv),
        Some(false),
        "wrong digest must refuse confirmation"
    );

    // Wrong chain name is unanswerable (None), not a false accusation.
    let mut wrong_name = vertical.clone();
    if let Act::Anchor { chain, .. } = &mut wrong_name {
        *chain = "west".into();
    }
    assert_eq!(
        sphere::confirms(&wrong_name, &south, fnv),
        None,
        "anchor naming a different chain is unanswerable"
    );

    // Vertical grants no ground — live deeds stay what north issued.
    let live = north.deeds().iter().filter(|d| d.live).count();
    assert_eq!(live, 1, "anchor must not mint a second live deed");

    // North's frontier records the observation of south.
    let f = north.frontier();
    assert_eq!(f.height_of("north"), north.height());
    assert_eq!(f.height_of("south"), south.height());
}

#[test]
fn reciprocal_anchors_order_the_frontiers() {
    let mut south = named_estate("south", "producer", 8);
    let mut north = named_estate("north", "newcomer", 8);

    // Concurrent until either sees the other.
    let s0 = Hello::of(&south, "s", 1 << 20).declaring(Uplink::of(&south, fnv));
    let n0 = Hello::of(&north, "n", 1 << 20).declaring(Uplink::of(&north, fnv));
    assert_eq!(
        s0.against(&n0),
        Some(None),
        "independent strangers start concurrent"
    );

    // North observes south.
    let south_up = Uplink::of(&south, fnv).expect("named");
    north.record(south_up.anchor("north saw south"));

    // South observes north (which now includes the vertical).
    let north_up = Uplink::of(&north, fnv).expect("named");
    south.record(north_up.anchor("south saw north"));

    let s1 = Hello::of(&south, "s", 1 << 20).declaring(Uplink::of(&south, fnv));
    let n1 = Hello::of(&north, "n", 1 << 20).declaring(Uplink::of(&north, fnv));

    // After reciprocal observation the fronts are comparable via join.
    let sf = s1.uplink.as_ref().unwrap().frontier.clone();
    let nf = n1.uplink.as_ref().unwrap().frontier.clone();
    let joined = sf.join(&nf);
    assert!(
        sf.compare(&joined) == Some(Ordering::Less) || sf.compare(&joined) == Some(Ordering::Equal),
        "south ≤ join"
    );
    assert!(
        nf.compare(&joined) == Some(Ordering::Less) || nf.compare(&joined) == Some(Ordering::Equal),
        "north ≤ join"
    );
    assert_eq!(joined.height_of("south"), south.height());
    assert_eq!(joined.height_of("north"), north.height());

    // North's anchor of south still confirms: south only grew *after*
    // that height, and confirms digests the cited prefix.
    let north_vertical = north
        .acts()
        .iter()
        .find(|a| matches!(a, Act::Anchor { chain, .. } if chain == "south"))
        .expect("north holds south anchor");
    assert_eq!(
        sphere::confirms(north_vertical, &south, fnv),
        Some(true),
        "north's anchor of south still confirms"
    );

    let south_vertical = south
        .acts()
        .iter()
        .find(|a| matches!(a, Act::Anchor { chain, .. } if chain == "north"))
        .expect("south holds north anchor");
    assert_eq!(
        sphere::confirms(south_vertical, &north, fnv),
        Some(true),
        "south's anchor of north confirms"
    );
}

#[test]
fn chain_bytes_round_trip_including_verticals() {
    let south = named_estate("south", "producer", 8);
    let mut north = named_estate("north", "newcomer", 8);
    let uplink = Uplink::of(&south, fnv).expect("named");
    north.record(uplink.anchor("persist me"));

    let bytes = chain::encode(north.acts());
    let decoded = chain::decode(&bytes).expect("decode");
    assert_eq!(decoded, north.acts());
    let verticals = decoded
        .iter()
        .filter(|a| matches!(a, Act::Anchor { .. }))
        .count();
    assert_eq!(verticals, 1);
    assert!(decoded.len() >= 2, "Open + Anchor at minimum");
}

#[test]
fn frontiers_join_is_per_chain_max() {
    let mut a = Frontier::new();
    a.observe("north", 3);
    a.observe("south", 10);
    let mut b = Frontier::new();
    b.observe("north", 7);
    b.observe("south", 4);
    let j = a.join(&b);
    assert_eq!(j.height_of("north"), 7);
    assert_eq!(j.height_of("south"), 10);
    assert!(a.concurrent_with(&b));
    assert_eq!(a.compare(&b), None);
}
