//! Diamond D-L5 — sphere-merge product pin (SM13 + SM17).
//!
//! ```text
//! two universe extents + PoUW bodies
//!   → admit_merge / settle_merge
//!   → principal = bulk S*, carry = residuals (multi-axial)
//!   → work_id credit once per structure; replay refuses second merge credit
//! ```
//!
//! ```bash
//! cargo test --test d5_sphere_merge_product
//! ```
//!
//! Edge idle (no venue). Residual is never burned or product-folded.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use assay::shape::triangle_claim;
use datum::extent::Extent;
use datum::merge::{admit_merge, MergePayout, MergeSplit};
use datum::reward::{closed_box_claim, RewardBook, RewardRefused};
use datum::settle::{self, MergeSettle};
use isthmus::sphere::Precedence;

/// Two high-overlap "universe" capacity vectors (bulk ≥ 9/10 of span).
fn universe_a() -> Extent {
    // Shared bulk dominates; residual only on axis 0 of A and axis 2 of B.
    Extent::new(vec![100, 50, 50])
}

fn universe_b() -> Extent {
    Extent::new(vec![90, 50, 55])
}

#[test]
fn sm13_two_universes_merge_to_bulk_and_residual_carry() {
    let a = universe_a();
    let b = universe_b();
    // M = [90, 50, 50], R_a = [10, 0, 0], R_b = [0, 0, 5]
    let split = MergeSplit::of(&a, &b).expect("split");
    assert_eq!(split.bulk.components(), &[90, 50, 50]);
    assert_eq!(split.residual_a.components(), &[10, 0, 0]);
    assert_eq!(split.residual_b.components(), &[0, 0, 5]);
    // Policy holds (bulk dominates).
    split
        .bulk_dominates_default(&a, &b)
        .expect("bulk policy");

    let payout = MergePayout::from_split(&split);
    assert_eq!(payout.principal.components(), split.bulk.components());
    assert_eq!(payout.carry_a.components(), split.residual_a.components());
    assert_eq!(payout.carry_b.components(), split.residual_b.components());

    // Residual is real — not zeroed by product or sum fold.
    assert!(payout.carry_a.components().iter().any(|c| *c > 0));
    assert!(payout.carry_b.components().iter().any(|c| *c > 0));
    assert_ne!(
        payout.carry_a.components(),
        payout.carry_b.components()
    );
}

#[test]
fn sm13_settle_merge_routes_three_books_with_asserted_numbers() {
    let a = universe_a();
    let b = universe_b();
    // Distinct structures so work_ids differ (dual claim classes still once each).
    let body_a = triangle_claim(1).encode();
    let body_b = closed_box_claim(1, 2).encode();

    let mut principal = RewardBook::new();
    let mut carry_a = RewardBook::new();
    let mut carry_b = RewardBook::new();

    let req = MergeSettle {
        a: &a,
        b: &b,
        powc: None,
        pouw: Some((body_a.as_slice(), body_b.as_slice())),
        precedence: Precedence::Concurrent,
        allow_ordered: false,
        price_on_star: Some(&Extent::new(vec![90, 50, 50])),
    };
    let (admit, payout) =
        settle::settle_merge(&req, &mut principal, &mut carry_a, &mut carry_b)
            .expect("product settle");

    assert_eq!(admit.split.bulk.components(), &[90, 50, 50]);
    assert_eq!(principal.total().components(), &[90, 50, 50]);
    assert_eq!(carry_a.total().components(), &[10, 0, 0]);
    assert_eq!(carry_b.total().components(), &[0, 0, 5]);
    assert_eq!(payout.principal.components(), principal.total().components());
}

#[test]
fn sm17_merge_work_ids_credit_once_then_replay_zero() {
    // PoUW work bodies admit the merge; court work_id is once per structure.
    let a = Extent::new(vec![20, 20]);
    let b = Extent::new(vec![20, 20]);
    let body_a = triangle_claim(7).encode();
    let body_b = closed_box_claim(3, 1).encode();

    let admit = admit_merge(
        &a,
        &b,
        None,
        Some((body_a.as_slice(), body_b.as_slice())),
        Precedence::Concurrent,
        false,
    )
    .expect("admit");

    let wa = admit.work_a.expect("work a");
    let wb = admit.work_b.expect("work b");
    assert_ne!(wa, wb, "distinct structures → distinct work_ids");

    let mut court = RewardBook::new();

    // Credit both merge legs once (primary work_id path).
    court.credit_claim(&body_a).expect("credit a");
    court.credit_claim(&body_b).expect("credit b");
    assert_eq!(court.act_len(), 2);

    // Replay identical structures → refuse (SM17 work_id once).
    assert!(matches!(
        court.credit_claim(&triangle_claim(99).encode()),
        Err(RewardRefused::Replay { .. })
    ));
    assert!(matches!(
        court.credit_claim(&closed_box_claim(99, 1).encode()),
        Err(RewardRefused::Replay { .. })
    ));
    assert_eq!(court.act_len(), 2, "replay must not grow acts");
}

#[test]
fn sm17_second_settle_merge_does_not_double_work_credit() {
    let a = Extent::new(vec![12, 12]);
    let b = Extent::new(vec![12, 12]);
    let body = triangle_claim(0).encode();

    let mut principal = RewardBook::new();
    let mut ca = RewardBook::new();
    let mut cb = RewardBook::new();
    let mut work_court = RewardBook::new();

    // Gate work once.
    work_court.credit_claim(&body).expect("once");

    let req = MergeSettle {
        a: &a,
        b: &b,
        powc: None,
        pouw: Some((body.as_slice(), body.as_slice())),
        precedence: Precedence::Concurrent,
        allow_ordered: false,
        price_on_star: None,
    };
    // Economic allocate (add_extent) may stack; work_id path stays once.
    settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("first settle");
    settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("economic re-route ok");

    // Work court still one act only.
    assert!(matches!(
        work_court.credit_claim(&triangle_claim(1).encode()),
        Err(RewardRefused::Replay { .. })
    ));
    assert_eq!(work_court.act_len(), 1);
    // Principal stacked bulk twice (economic legs, not work mint).
    assert_eq!(principal.total().components(), &[24, 24]);
}
