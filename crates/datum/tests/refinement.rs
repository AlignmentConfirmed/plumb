//! O2–O4 measurements: a strictly leaner chain refines settled work;
//! the threshold refuses with every number named; the equivalence is
//! an append that federates; and the homology certificate is a proof
//! claim whose prescribed boundary is exactly the difference.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{DeclaredClaim, DeclaredComplex, Entry, ProofClaim};
use assay::whole;
use datum::bounty::{settle_refinement, RefineRefused, RefinementBounty};
use datum::reward::RewardBook;

const CEILING: u64 = 10_000;

/// Theta with a filling: two vertices, three parallel edges, and ONE
/// 2-cell f with ∂f = 2·e1 − 2·e2 — so the fat and lean cycles below
/// are not merely same-boundary but genuinely homologous, and the
/// certificate has something true to prove.
fn theta_filled() -> DeclaredComplex {
    let mut op1 = Vec::new();
    for edge in 0..3u32 {
        op1.push(Entry { row: 0, col: edge, coeff: whole(-1) });
        op1.push(Entry { row: 1, col: edge, coeff: whole(1) });
    }
    let op2 = vec![
        Entry { row: 1, col: 0, coeff: whole(2) },
        Entry { row: 2, col: 0, coeff: whole(-2) },
    ];
    DeclaredComplex {
        cells: vec![2, 3, 1],
        ops: vec![op1, op2],
    }
}

/// The fat cycle: e0 + e1 − 2·e2.
fn fat(transport: u64) -> DeclaredClaim {
    DeclaredClaim {
        transport,
        complex: theta_filled(),
        dim: 1,
        witness: vec![(0, whole(1)), (1, whole(1)), (2, whole(-2))],
    }
}

/// The lean cycle: e0 − e1. Same boundary (none), fewer everything.
fn lean(transport: u64) -> DeclaredClaim {
    DeclaredClaim {
        transport,
        complex: theta_filled(),
        dim: 1,
        witness: vec![(0, whole(1)), (1, whole(-1))],
    }
}

/// O4's certificate: the 2-cell fills the difference —
/// fat − lean = 2·e1 − 2·e2 = ∂f, exhibited as a proof claim.
fn certificate() -> Vec<u8> {
    ProofClaim {
        transport: 0,
        complex: theta_filled(),
        dim: 2,
        target: vec![(1, whole(2)), (2, whole(-2))],
        witness: vec![(0, whole(1))],
        deps: Vec::new(),
    }
    .encode()
}

fn settled_original() -> (RewardBook, RefinementBounty) {
    let mut book = RewardBook::new();
    let original = fat(1);
    let target = original.work_id();
    book.credit_claim(&original.encode()).expect("original settles");
    let bounty = RefinementBounty {
        target,
        min_improvement_percent: 10,
        reward: 5_000,
    };
    (book, bounty)
}

#[test]
fn a_strictly_leaner_chain_refines_settled_work() {
    let (mut book, bounty) = settled_original();
    let refined = settle_refinement(&bounty, &lean(1).encode(), None, &mut book, CEILING)
        .expect("the lean cycle refines");

    assert!(refined.saved_fuel > 0, "the meter measured real savings");
    assert!(refined.saved_bytes > 0);
    assert_eq!(refined.payout, 5_000);
    assert!(!refined.homologous, "no certificate was offered");

    // O3 — the record advertises the cheap articulation; nothing was
    // rewritten, and both ids remain settled work.
    let advertised = book.refinements_of(&bounty.target);
    assert_eq!(advertised.len(), 1);
    assert_eq!(advertised.first().map(|(id, _, _)| id.clone()), Some(refined.credit.work_id.clone()));
    assert!(book.seen().contains(&bounty.target), "old citations unbroken");
}

#[test]
fn the_threshold_refuses_with_every_number_named() {
    let (mut book, mut bounty) = settled_original();
    bounty.min_improvement_percent = 20; // the same lean chain falls short here
    match settle_refinement(&bounty, &lean(1).encode(), None, &mut book, CEILING) {
        Err(RefineRefused::NotAnImprovement {
            needed_percent,
            original_fuel,
            refined_fuel,
        }) => {
            assert_eq!(needed_percent, 20);
            assert!(
                u128::from(refined_fuel) * 100 > u128::from(original_fuel) * 80,
                "the numbers say exactly how far it fell short: {refined_fuel} vs {original_fuel}"
            );
        }
        other => panic!("expected NotAnImprovement, got {other:?}"),
    }
    assert_eq!(book.act_len(), 1, "an almost-improvement earns nothing");
}

#[test]
fn identical_resubmission_and_unsettled_target_refuse() {
    let (mut book, bounty) = settled_original();

    // The original "refining" itself dies at the THRESHOLD, before
    // the book is even consulted: zero improvement is not an
    // improvement, and the anti-dust gate fires first.
    assert!(matches!(
        settle_refinement(&bounty, &fat(9).encode(), None, &mut book, CEILING),
        Err(RefineRefused::NotAnImprovement { .. })
    ));
    assert_eq!(book.act_len(), 1, "and the book never moved");

    // A bounty on work nobody settled.
    let phantom = RefinementBounty {
        target: lean(0).work_id(),
        min_improvement_percent: 10,
        reward: 1,
    };
    let mut fresh = RewardBook::new();
    assert_eq!(
        settle_refinement(&phantom, &lean(1).encode(), None, &mut fresh, CEILING),
        Err(RefineRefused::UnsettledTarget)
    );
}

#[test]
fn the_homology_certificate_is_verified_not_believed() {
    // With the true certificate: homologous, provably.
    let (mut book, bounty) = settled_original();
    let refined = settle_refinement(
        &bounty,
        &lean(1).encode(),
        Some(&certificate()),
        &mut book,
        CEILING,
    )
    .expect("refines with proof of class");
    assert!(refined.homologous, "∂h = fat − lean, exhibited and checked");

    // A certificate claiming the wrong difference refuses by name.
    let (mut book, bounty) = settled_original();
    let mut wrong = ProofClaim::decode(&certificate()).expect("decodes");
    wrong.target = vec![(1, whole(1)), (2, whole(-1))];
    assert_eq!(
        settle_refinement(&bounty, &lean(1).encode(), Some(&wrong.encode()), &mut book, CEILING),
        Err(RefineRefused::CertificateWrongDifference)
    );

    // A certificate whose filling does not actually fill: the SQ1
    // evaluator refuses it, and the refusal carries through.
    let (mut book, bounty) = settled_original();
    let mut unfilled = ProofClaim::decode(&certificate()).expect("decodes");
    unfilled.witness = vec![(0, whole(2))]; // ∂(2f) = 4e1 − 4e2 ≠ target
    assert!(matches!(
        settle_refinement(&bounty, &lean(1).encode(), Some(&unfilled.encode()), &mut book, CEILING),
        Err(RefineRefused::CertificateBroken(_))
    ));
}

#[test]
fn the_equivalence_federates_once_like_everything_else() {
    let (mut book_a, bounty) = settled_original();
    settle_refinement(&bounty, &lean(1).encode(), None, &mut book_a, CEILING)
        .expect("refines");

    let mut book_b = RewardBook::new();
    let first = book_b.merge_acts_from(&book_a);
    assert!(first >= 3, "two credits and the equivalence crossed");
    assert_eq!(book_b.refinements_of(&bounty.target).len(), 1);

    let again = book_b.merge_acts_from(&book_a);
    assert_eq!(again, 0, "gossip creates no value, equivalences included");
}
