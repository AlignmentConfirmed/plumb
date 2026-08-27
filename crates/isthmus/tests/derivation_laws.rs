//! LAWS for derived tags: a number nobody declares.
//!
//! `netstratum`'s mesh writes `MESH_HEAD_TAG: u8 = 64`. `IS-3` §5 grants
//! 64–79 to `isthmus`. Two substrate-layer protocols, written without
//! knowledge of each other, both reached for the same byte — and
//! **neither was wrong**, because each was choosing a global number from
//! a private constant.
//!
//! A constant cannot be right here. It encodes an assumption about who
//! else exists, and who else exists is exactly what a substrate cannot
//! know: a kernel that has not attached yet cannot have been consulted.
//! It is the same defect as `grants_available() -> 6`, wearing a number
//! instead of a count.
//!
//! So the tag is **derived**:
//!
//! ```text
//! tag = deed.low() + (spread(kind) mod deed.width())
//! ```
//!
//! and the laws below are the standard an outside implementation has to
//! meet. Determinism is the whole product: two parties that never meet
//! must compute the same answer with nothing exchanged.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::deed::{Deed, Ledger};
use isthmus::layout::Layout;

/// Record kinds from the protocols actually in this environment, plus
/// enough others to exercise a vocabulary.
const KINDS: [&str; 12] = [
    "mesh.head",
    "mesh.refusal",
    "hello",
    "relation",
    "manifold",
    "closures",
    "witness",
    "receipt",
    "aperture",
    "grammar",
    "lesson",
    "chronicle.segment",
];

fn deed_on(edge: &mut Ledger, holder: &str, width: u128) -> Deed {
    edge.issue(holder, width).expect("room for the deed")
}

// ===================================================================
// D1 — determinism
// ===================================================================

/// **A function of the kind and the deed alone.** Called twice, called
/// on a clone, called after unrelated acts land — the same answer.
#[test]
fn d1_the_same_kind_on_the_same_deed_is_always_the_same_tag() {
    let mut edge = Ledger::new(Layout::founding());
    edge.encumber(1, 31, "ancestral", "these laws");
    let deed = deed_on(&mut edge, "peer", 16);

    for kind in KINDS {
        let once = deed.tag_for(kind);
        assert!(once.is_some(), "{kind} derived nothing on a 16-wide deed");
        for _ in 0..8 {
            assert_eq!(deed.tag_for(kind), once, "{kind} moved between calls");
        }
        assert_eq!(deed.clone().tag_for(kind), once, "{kind} moved on a clone");
    }

    // Unrelated history does not move it: the derivation reads the
    // deed, not the ledger.
    let before: Vec<_> = KINDS.iter().map(|k| deed.tag_for(k)).collect();
    edge.encumber(200, 210, "somebody", "later");
    let _ = edge.issue("another", 8);
    let after: Vec<_> = KINDS.iter().map(|k| deed.tag_for(k)).collect();
    assert_eq!(before, after, "later acts moved a derived tag");
}

/// **Every derived tag is inside its own deed.** This is the property
/// that makes cross-holder collision structurally impossible rather
/// than merely unlikely.
#[test]
fn d1_a_derived_tag_never_leaves_its_deed() {
    let mut edge = Ledger::new(Layout::founding());
    edge.encumber(1, 31, "ancestral", "these laws");

    for width in [1u128, 2, 3, 16, 48, 100] {
        let mut fresh = Ledger::new(Layout::founding());
        fresh.encumber(1, 31, "ancestral", "these laws");
        let deed = deed_on(&mut fresh, "peer", width);
        for kind in KINDS {
            let tag = deed.tag_for(kind).expect("a deed with width derives");
            assert!(
                tag >= deed.low() && tag <= deed.high(),
                "{kind} derived {tag}, outside {}-{} on a {width}-wide deed",
                deed.low(),
                deed.high(),
            );
        }
    }
    let _ = edge;
}

// ===================================================================
// D2 — the collision that started this cannot happen
// ===================================================================

/// **Two holders cannot derive the same tag**, whatever they call their
/// records — because deeds are disjoint and a derivation never leaves
/// its deed.
///
/// This is the answer to `MESH_HEAD_TAG = 64`. Both parties may keep
/// the name `head`; they cannot keep the number, and they no longer
/// need to agree on one.
#[test]
fn d2_two_holders_deriving_the_same_names_never_collide() {
    let mut edge = Ledger::new(Layout::founding());
    edge.encumber(1, 31, "ancestral", "these laws");
    let mine = deed_on(&mut edge, "isthmus", 16);
    let theirs = deed_on(&mut edge, "ns-mesh", 16);

    // The worst case on purpose: identical vocabularies.
    for kind in KINDS {
        let here = mine.tag_for(kind).expect("derives");
        let there = theirs.tag_for(kind).expect("derives");
        assert_ne!(
            here, there,
            "{kind} derived {here} for both holders — the deeds overlap",
        );
    }

    // And the same name means the same THING on both, because the
    // offset is what identifies it. The number is the frame; the offset
    // is the invariant.
    for kind in KINDS {
        assert_eq!(
            mine.offset_for(kind),
            theirs.offset_for(kind),
            "{kind} has a different offset on two equal-width deeds — \
             then the same record kind is not the same record kind",
        );
    }
}

/// **The gate fires.** If the deeds overlapped, `d2` would be asserting
/// something false — so overlapping regions must produce a shared tag,
/// proving the disjointness is what the law rests on.
#[test]
fn d2_overlapping_regions_would_collide_which_is_why_deeds_are_disjoint() {
    let overlapping = |low: u64, high: u64| Deed {
        holder: "constructed".to_owned(),
        region: vec![(low, high)],
        live: true,
        within: None,
    };
    let a = overlapping(64, 79);
    let b = overlapping(64, 79);

    let mut shared = 0usize;
    for kind in KINDS {
        if a.tag_for(kind) == b.tag_for(kind) {
            shared += 1;
        }
    }
    assert_eq!(
        shared,
        KINDS.len(),
        "two identical regions derived different tags — the derivation \
         is not a function of the region",
    );
}

// ===================================================================
// D3 — stable under growth
// ===================================================================

/// **Declaring a new record kind never moves an existing one.**
///
/// An assignment that probed for a free slot would pack tighter and
/// would move tags when the vocabulary grew — a wire break dressed as
/// an optimisation. This derivation does not depend on what else is in
/// the vocabulary, so it cannot.
#[test]
fn d3_growing_the_vocabulary_moves_nothing() {
    let mut edge = Ledger::new(Layout::founding());
    edge.encumber(1, 31, "ancestral", "these laws");
    let deed = deed_on(&mut edge, "peer", 48);

    let base: Vec<(String, u64)> = KINDS
        .iter()
        .map(|k| ((*k).to_owned(), deed.tag_for(k).expect("derives")))
        .collect();

    // Add kinds one at a time; nothing already there may move.
    let mut vocabulary: Vec<&str> = KINDS.to_vec();
    for extra in ["vent", "docket", "uplink", "sublet", "anchor", "strike"] {
        vocabulary.push(extra);
        for (kind, was) in &base {
            assert_eq!(
                deed.tag_for(kind),
                Some(*was),
                "{kind} moved from {was} when {extra} was declared",
            );
        }
    }
    assert_eq!(vocabulary.len(), KINDS.len() + 6);
}

/// **A collision inside one vocabulary is reported, not resolved.**
///
/// The cost of `d3`: two names can land on one tag. The author renames
/// one. Resolving it silently would trade a visible refusal for a
/// property nobody could rely on.
#[test]
fn d3_a_vocabulary_collision_is_named() {
    let mut edge = Ledger::new(Layout::founding());
    edge.encumber(1, 31, "ancestral", "these laws");

    // A deed too narrow for the vocabulary forces collisions by
    // pigeonhole: 12 kinds cannot have 12 distinct tags in 4 slots.
    let narrow = deed_on(&mut edge, "cramped", 4);
    let found = narrow.collisions(&KINDS);
    assert!(
        !found.is_empty(),
        "12 kinds in a 4-wide deed reported no collision — pigeonhole \
         says otherwise, so the report is broken",
    );
    for (one, other, tag) in &found {
        assert_eq!(narrow.tag_for(one), Some(*tag));
        assert_eq!(narrow.tag_for(other), Some(*tag));
        assert_ne!(one, other, "a kind collided with itself");
    }

    // Order-invariant: the same vocabulary shuffled reports the same
    // pairs.
    let mut shuffled: Vec<&str> = KINDS.to_vec();
    shuffled.reverse();
    assert_eq!(
        narrow.collisions(&shuffled),
        found,
        "the collision report depends on declaration order",
    );

    // And a deed with room reports none for this vocabulary — so the
    // report is not simply always non-empty.
    let roomy = deed_on(&mut edge, "roomy", 200);
    assert!(
        roomy.collisions(&KINDS).is_empty(),
        "a 200-wide deed collided on 12 names: {:?}",
        roomy.collisions(&KINDS),
    );
}

// ===================================================================
// D4 — the offset is what crosses
// ===================================================================

/// **Two edges give a holder different numbers and the same offsets.**
///
/// The absolute tag is a fact about one edge; the offset is the record
/// kind's identity. This is `potential_at`'s rule — the box origin is
/// the frame, the offset is gauge-invariant — applied to vocabulary.
#[test]
fn d4_the_offset_survives_a_change_of_edge() {
    let mut edge_a = Ledger::new(Layout::founding());
    edge_a.encumber(1, 31, "ancestral", "these laws");
    let here = deed_on(&mut edge_a, "peer", 32);

    let mut edge_b = Ledger::new(Layout::founding());
    edge_b.encumber(1, 120, "a busier neighbour", "their advert");
    let there = deed_on(&mut edge_b, "peer", 32);

    assert_ne!(here.low(), there.low(), "the edges agree — nothing to test");

    for kind in KINDS {
        assert_eq!(
            here.offset_for(kind),
            there.offset_for(kind),
            "{kind} changed identity between edges",
        );
        assert_ne!(
            here.tag_for(kind),
            there.tag_for(kind),
            "{kind} got the same absolute tag on two different edges",
        );
        // And the absolute tag is exactly origin + offset on each.
        assert_eq!(
            here.tag_for(kind),
            Some(here.low() + here.offset_for(kind).expect("derives")),
        );
        assert_eq!(
            there.tag_for(kind),
            Some(there.low() + there.offset_for(kind).expect("derives")),
        );
    }
}

/// **A deed with no width derives nothing**, rather than deriving zero.
///
/// Tag 0 is the void — a zero-filled buffer decodes to it — so a
/// derivation that fell back to zero would put every record from an
/// empty deed on the one tag that must name nothing.
#[test]
fn d4_an_empty_deed_derives_nothing() {
    let empty = Deed {
        holder: "nobody".to_owned(),
        region: Vec::new(),
        live: true,
        within: None,
    };
    for kind in KINDS {
        assert_eq!(empty.tag_for(kind), None, "{kind} derived from no region");
        assert_eq!(empty.offset_for(kind), None);
    }
    assert!(empty.collisions(&KINDS).is_empty());
}
