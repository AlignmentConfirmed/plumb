//! Sphere merge SM1–SM11: bulk/residual, admit, payout, highway path.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use assay::shape::triangle_claim;
use assay::{whole, Boundary, Facet, Orientation};
use datum::extent::Extent;
use datum::merge::{
    admit_merge, admit_pouw, admit_powc, split_with_policy, MergePayout, MergeRefused, MergeSplit,
    DEFAULT_BULK_DEN, DEFAULT_BULK_NUM,
};
use datum::onramp::{shape_body, shape_from_edges};
use datum::reward::RewardBook;
use datum::settle;
use isthmus::sphere::Precedence;
use isthmus::work::{self, SHAPE_CLAIM_TAG};

fn closed_1d(flux: i64) -> Boundary {
    let mut b = Boundary::new(1);
    let f = whole(flux);
    assert!(b.face(Facet::new(0, Orientation::Low, f.clone())));
    assert!(b.face(Facet::new(0, Orientation::High, f)));
    b
}

// ─── SM1–SM3 ─────────────────────────────────────────────────────────

#[test]
fn sm1_meet_and_residual_are_componentwise() {
    let a = Extent::new(vec![10, 4, 8]);
    let b = Extent::new(vec![6, 9, 8]);
    let m = a.meet(&b).expect("meet");
    assert_eq!(m.components(), &[6, 4, 8]);
    let ra = a.saturating_sub(&m).expect("ra");
    let rb = b.saturating_sub(&m).expect("rb");
    assert_eq!(ra.components(), &[4, 0, 0]);
    assert_eq!(rb.components(), &[0, 5, 0]);
}

#[test]
fn sm2_merge_split_refuses_arity_mismatch() {
    let a = Extent::new(vec![3, 3]);
    let b = Extent::new(vec![3]);
    assert!(matches!(
        MergeSplit::of(&a, &b),
        Err(MergeRefused::Arity { left: 2, right: 1 })
    ));
}

#[test]
fn sm2_empty_bulk_refuses() {
    let a = Extent::new(vec![0, 5]);
    let b = Extent::new(vec![3, 0]);
    // meet = [0,0] → nothing
    assert!(matches!(
        MergeSplit::of(&a, &b),
        Err(MergeRefused::EmptyBulk)
    ));
}

#[test]
fn sm3_bulk_dominance_default_nine_tenths() {
    // 90% bulk on every axis: A=B=10 → M=10, residual 0 → ok
    let a = Extent::new(vec![10, 10]);
    let b = Extent::new(vec![10, 10]);
    split_with_policy(&a, &b, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN).expect("full overlap");

    // A=10, B=1 → M=1, span=10, 1/10 < 9/10 → thin
    let thin = Extent::new(vec![10]);
    let small = Extent::new(vec![1]);
    match split_with_policy(&thin, &small, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN) {
        Err(MergeRefused::BulkTooThin {
            axis: 0,
            bulk: 1,
            span: 10,
        }) => {}
        other => panic!("expected BulkTooThin, got {other:?}"),
    }

    // 9/10 exactly: M=9, span=10 → 9*10 >= 10*9 → ok
    let a9 = Extent::new(vec![9]);
    let b10 = Extent::new(vec![10]);
    split_with_policy(&a9, &b10, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN).expect("exactly 9/10");
}

#[test]
fn sm3_ratios_are_per_axis_not_product() {
    let a = Extent::new(vec![10, 5]);
    let b = Extent::new(vec![10, 10]);
    let s = MergeSplit::of(&a, &b).expect("split");
    let r = s.ratios_per(&a, &b, 10).expect("ratios");
    // axis0: 10/10 = 10/10, axis1: 5/10 = 5/10
    assert_eq!(r, vec![10, 5]);
}

// ─── SM4–SM5 admit ───────────────────────────────────────────────────

#[test]
fn sm4_powc_requires_both_closed() {
    let ok = closed_1d(3);
    admit_powc(&ok, &ok).expect("closed");
    let mut open = Boundary::new(1);
    assert!(open.face(Facet::new(0, Orientation::Low, whole(1))));
    assert!(open.face(Facet::new(0, Orientation::High, whole(2))));
    assert!(matches!(
        admit_powc(&ok, &open),
        Err(MergeRefused::PowcOpen)
    ));
}

#[test]
fn sm5_pouw_shape_work_ids() {
    let a = triangle_claim(1);
    let b = triangle_claim(2);
    let (wa, wb) = admit_pouw(&a, &b).expect("pouw");
    assert_eq!(wa, a.work_id());
    assert_eq!(wb, b.work_id());
    // same structure → same work_id
    assert_eq!(wa, wb);
}

// ─── SM6–SM7 admit_merge ─────────────────────────────────────────────

#[test]
fn sm6_admit_by_pouw_with_extents() {
    // High overlap so bulk policy passes: [10,10] ∧ [10,10]
    let a = Extent::new(vec![10, 10]);
    let b = Extent::new(vec![10, 10]);
    let body_a = triangle_claim(0).encode();
    let body_b = triangle_claim(1).encode();
    let admit = admit_merge(
        &a,
        &b,
        None,
        Some((body_a.as_slice(), body_b.as_slice())),
        Precedence::Concurrent,
        false,
    )
    .expect("admit");
    assert_eq!(admit.split.bulk.components(), &[10, 10]);
    assert!(admit.work_a.is_some());
}

#[test]
fn sm7_ordered_fault_refuses_without_override() {
    let a = Extent::new(vec![10]);
    let b = Extent::new(vec![10]);
    let body = triangle_claim(0).encode();
    match admit_merge(
        &a,
        &b,
        None,
        Some((body.as_slice(), body.as_slice())),
        Precedence::HereSawThere,
        false,
    ) {
        Err(MergeRefused::OrderedFault {
            order: Precedence::HereSawThere,
        }) => {}
        other => panic!("expected OrderedFault, got {other:?}"),
    }
    // override allows
    admit_merge(
        &a,
        &b,
        None,
        Some((body.as_slice(), body.as_slice())),
        Precedence::HereSawThere,
        true,
    )
    .expect("override");
}

// ─── SM8–SM10 payout ─────────────────────────────────────────────────

#[test]
fn sm8_payout_principal_and_carry_are_multi_axial() {
    let a = Extent::new(vec![10, 4]);
    let b = Extent::new(vec![6, 9]);
    let s = MergeSplit::of(&a, &b).expect("split");
    let p = MergePayout::from_split(&s);
    assert_eq!(p.principal.components(), &[6, 4]);
    assert_eq!(p.carry_a.components(), &[4, 0]);
    assert_eq!(p.carry_b.components(), &[0, 5]);
    // No fold: total obligation is three vectors, not one product
    assert_ne!(
        p.principal.components().iter().product::<u128>(),
        0,
        "sanity"
    );
}

#[test]
fn sm9_settle_merge_routes_three_books() {
    let a = Extent::new(vec![10, 10]);
    let b = Extent::new(vec![10, 10]);
    let body = triangle_claim(0).encode();
    let mut principal = RewardBook::new();
    let mut ca = RewardBook::new();
    let mut cb = RewardBook::new();
    let req = settle::MergeSettle {
        a: &a,
        b: &b,
        powc: None,
        pouw: Some((body.as_slice(), body.as_slice())),
        precedence: Precedence::Concurrent,
        allow_ordered: false,
        price_on_star: Some(&Extent::new(vec![10, 10])),
    };
    let (admit, payout) =
        settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("settle");
    assert_eq!(principal.total().components(), payout.principal.components());
    assert_eq!(ca.total().components(), payout.carry_a.components());
    assert_eq!(cb.total().components(), payout.carry_b.components());
    assert_eq!(admit.split.bulk.components(), &[10, 10]);
}

#[test]
fn sm10_carry_not_folded_to_scalar_dust() {
    let a = Extent::new(vec![10, 1]);
    let b = Extent::new(vec![10, 10]);
    // bulk [10,1] — axis1 is 1/10 < 9/10 → policy fail (thin)
    assert!(matches!(
        split_with_policy(&a, &b, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN),
        Err(MergeRefused::BulkTooThin { axis: 1, .. })
    ));
    // Without policy, residual still multi-axial
    let s = MergeSplit::of(&a, &b).expect("split");
    assert_eq!(s.residual_b.components(), &[0, 9]);
    assert!(!s.residual_b.is_nothing() || s.residual_b.component(1) == Some(9));
}

// ─── SM11 highway path ───────────────────────────────────────────────

#[test]
fn sm11_highway_shape_frames_admit_merge() {
    // Two overlapping high-capacity spheres + tag-82 framed shape bodies
    let cap_a = Extent::new(vec![10, 10, 10]);
    let cap_b = Extent::new(vec![10, 10, 10]);
    let shape = shape_from_edges(
        3,
        [
            (0, 1, whole(1)),
            (1, 2, whole(1)),
            (0, 2, whole(1)),
        ],
    )
    .expect("shape");
    let body = shape_body(0, shape).expect("body");
    let mut wire_a = Vec::new();
    let mut wire_b = Vec::new();
    work::put_shape_claim(&body, &mut wire_a).expect("frame a");
    work::put_shape_claim(&body, &mut wire_b).expect("frame b");

    let (tag, value_a) = work::take_frame(&wire_a).expect("take a");
    assert_eq!(tag, SHAPE_CLAIM_TAG);
    let (_, value_b) = work::take_frame(&wire_b).expect("take b");

    // Carrier classifies as mine (work tag)
    match work::classify(&wire_a).expect("classify") {
        work::Envelope::Mine { tag, .. } => assert_eq!(tag, SHAPE_CLAIM_TAG),
        work::Envelope::Forward { .. } => panic!("shape must deliver"),
    }

    let mut principal = RewardBook::new();
    let mut ca = RewardBook::new();
    let mut cb = RewardBook::new();
    let price = Extent::new(vec![10, 10, 10]);
    let req = settle::MergeSettle {
        a: &cap_a,
        b: &cap_b,
        powc: None,
        pouw: Some((value_a, value_b)),
        precedence: Precedence::Concurrent,
        allow_ordered: false,
        price_on_star: Some(&price),
    };
    let (_admit, payout) =
        settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("highway merge settle");
    assert_eq!(payout.principal.components(), &[10, 10, 10]);
    assert!(payout.carry_a.is_nothing());
    assert!(payout.carry_b.is_nothing());
}
