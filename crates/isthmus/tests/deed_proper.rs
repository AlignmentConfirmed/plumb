//! THE DEED PROPER — maturation, facets, and the Recognised seam.
//!
//! The three pieces a claim needs to become computable geometry:
//! the claim matures into a deed (so potentials exist over it), the
//! deed carries its boundary (so a closure proof has facets to walk),
//! and the seam recognises the deed's tags (so the mesh delivers the
//! unfractured record without evaluating a payload byte).

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::deed::{Flaw, Ledger, Refused, Standing};
use isthmus::layout::Layout;
use isthmus::Verdict;

// ===================================================================
// Maturation: a claim growing into a deed, and only its own claim
// ===================================================================

#[test]
fn a_claim_matures_into_its_claimants_deed_and_nobody_elses() {
    let mut edge = Ledger::new(Layout::founding());
    edge.encumber(55, 56, "strand", "wire.rs registry, read today");
    edge.encumber(32, 54, "netstratum", "NS registries");

    // The admitted arm: the claimant matures their own claim.
    let deed = edge.mature("strand", 55, 56).expect("the claim is theirs");
    assert_eq!((deed.low(), deed.high()), (55, 56));
    assert_eq!(edge.standing_of(55), Standing::Deeded { holder: "strand".into() });
    assert_eq!(edge.standing_of(56), Standing::Deeded { holder: "strand".into() });
    edge.well_formed().expect("maturation is lawful history");

    // Potentials now exist over the matured ground — which is the whole
    // point: the gauge-invariant reading needs a deed to read against.
    let (holder, offsets) = edge.potential_at(&[56]).expect("deeded ground");
    assert_eq!(holder, "strand");
    assert_eq!(offsets, vec![1]);

    // The refused arms, each named:
    // somebody else's claim is not yours to mature —
    let mut thief = Ledger::new(Layout::founding());
    thief.encumber(55, 56, "strand", "their claim");
    assert!(matches!(
        thief.mature("chitin", 55, 56),
        Err(Refused::NotYourClaim { .. })
    ));
    // open ground is not a claim at all —
    assert!(matches!(
        thief.mature("strand", 200, 207),
        Err(Refused::NotYourClaim { .. })
    ));
    // and H1 is not suspended for maturation.
    let mut greedy = Ledger::new(Layout::founding());
    greedy.encumber(55, 56, "strand", "claim");
    greedy.issue("strand", 8).expect("room");
    assert!(matches!(
        greedy.mature("strand", 55, 56),
        Err(Refused::AlreadyHeld { .. })
    ));
}

/// The checker agrees: an issue over the holder's OWN encumbrance is
/// well-formed history; over anybody else's it stays an Overlap.
#[test]
fn well_formed_admits_maturation_and_still_refuses_squatting() {
    let mut matured = Ledger::new(Layout::founding());
    matured.encumber(55, 56, "strand", "claim");
    matured.record(isthmus::deed::Act::Issue {
        holder: "strand".into(),
        low: 55,
        high: 56,
    });
    matured.well_formed().expect("a matured claim is lawful");

    let mut squatted = Ledger::new(Layout::founding());
    squatted.encumber(55, 56, "strand", "claim");
    squatted.record(isthmus::deed::Act::Issue {
        holder: "squatter".into(),
        low: 55,
        high: 56,
    });
    assert!(matches!(
        squatted.well_formed(),
        Err(Flaw::Overlap { at: 1, .. })
    ));
}

// ===================================================================
// Facets: the boundary a closure proof walks
// ===================================================================

#[test]
fn facets_bound_the_box_with_alternating_orientation() {
    let mut edge = Ledger::new(Layout::with_tag_width(1));
    edge.open_axis("revision", 7);
    let deed = edge.issue_box("H", &[4, 3]).expect("room");

    let facets = deed.facets();
    assert_eq!(facets.len(), 4, "2n facets for n axes");

    for axis in 0..2usize {
        let pair: Vec<_> = facets.iter().filter(|f| f.axis == axis).collect();
        assert_eq!(pair.len(), 2);
        let total: i8 = pair.iter().map(|f| f.orientation).sum();
        assert_eq!(total, 0, "opposite faces must cancel — ∂∂ = 0");

        for facet in pair {
            // The face is flat on its axis and full on every other.
            let (low, high) = facet.region[axis];
            assert_eq!(low, high, "a facet is flat on its own axis");
            for other in 0..2usize {
                if other != axis {
                    assert_eq!(facet.region[other], deed.region[other]);
                }
            }
            // And it lies on the deed's boundary, not inside it.
            assert!(
                low == deed.region[axis].0 || low == deed.region[axis].1,
                "a facet strayed off the boundary"
            );
        }
    }
}

// ===================================================================
// Recognised: the fifth verdict, and its lawful degradation
// ===================================================================

#[test]
fn the_seam_recognises_deeded_tags_and_degrades_without_a_court() {
    let mut court = Ledger::new(Layout::founding());
    court.encumber(55, 56, "strand", "claim");
    court.mature("strand", 55, 56).expect("the deed proper");

    let mine = |tag: u64| (64..=79).contains(&tag);
    let bound = 1 << 16;

    // A record under strand's deed, framed whole.
    let mut wire = Vec::new();
    isthmus::frame::put_frame(&Layout::founding(), 56, &[0xAA, 0xBB], &mut wire)
        .expect("fits");

    // With the court: RECOGNISED — shape confirmed, payload opaque,
    // the whole record named for unfractured delivery.
    assert_eq!(
        isthmus::verdict(&Layout::founding(), &wire, bound, mine, Some(&court)),
        Verdict::Recognised { tag: 56, whole: 7 }
    );

    // Without the court: the SAME bytes lawfully degrade to Skip —
    // forwarded instead of delivered. Economy lost, correctness never.
    assert_eq!(
        isthmus::verdict(&Layout::founding(), &wire, bound, mine, None),
        Verdict::Skip { tag: 56, whole: 7 }
    );

    // The neighbours, so Recognised sits strictly BETWEEN them:
    // a tag nobody holds skips even with the court —
    let mut unknown = Vec::new();
    isthmus::frame::put_frame(&Layout::founding(), 200, &[0xCC], &mut unknown)
        .expect("fits");
    assert!(matches!(
        isthmus::verdict(&Layout::founding(), &unknown, bound, mine, Some(&court)),
        Verdict::Skip { tag: 200, .. }
    ));
    // one this reader owns accepts, court or none —
    let mut owned = Vec::new();
    isthmus::frame::put_frame(&Layout::founding(), 64, &[], &mut owned).expect("fits");
    assert_eq!(
        isthmus::verdict(&Layout::founding(), &owned, bound, mine, Some(&court)),
        Verdict::Accept
    );
    // and the shape gates still precede everything: torn stays refused,
    // short stays waiting, court or none.
    let mut torn = vec![56u8];
    torn.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        isthmus::verdict(&Layout::founding(), &torn, bound, mine, Some(&court)),
        Verdict::Refuse(_)
    ));
    assert_eq!(
        isthmus::verdict(&Layout::founding(), &wire[..3], bound, mine, Some(&court)),
        Verdict::Wait
    );
}