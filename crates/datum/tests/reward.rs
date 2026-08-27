//! Multi-axial rewards: boundary + shape, work_id anti-replay.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use assay::{whole, Boundary, Facet, Orientation};
use datum::extent::Extent;
use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};

#[test]
fn closed_work_credits_per_axis() {
    let mut book = RewardBook::new();
    let body = closed_box_claim(11, 5).encode();
    let credit = book.credit_claim(&body).expect("credit");
    assert_eq!(credit.transport, 11);
    assert_eq!(credit.axes.components(), &[1, 1]);
    assert!(credit.witness.is_some());
    assert_eq!(book.total().components(), &[1, 1]);
}

#[test]
fn same_structure_replays_even_with_different_transport() {
    let mut book = RewardBook::new();
    let a = closed_box_claim(1, 1).encode();
    let b = closed_box_claim(99, 1).encode();
    book.credit_claim(&a).expect("first");
    match book.credit_claim(&b) {
        Err(RewardRefused::Replay { work_id }) => {
            assert_eq!(work_id, closed_box_claim(0, 1).work_id());
        }
        other => panic!("expected structure replay, got {other:?}"),
    }
}

#[test]
fn shape_triangle_credits_per_orb() {
    let mut book = RewardBook::new();
    let body = triangle_claim(3).encode();
    let credit = book.credit_claim(&body).expect("shape credit");
    assert_eq!(credit.axes.components(), &[1, 1, 1]);
    assert!(credit.witness.is_none());
    // same shape, different transport → replay
    assert!(matches!(
        book.credit_claim(&triangle_claim(9).encode()),
        Err(RewardRefused::Replay { .. })
    ));
}

#[test]
fn open_work_earns_nothing() {
    let mut b = Boundary::new(1);
    assert!(b.face(Facet::new(0, Orientation::Low, whole(1))));
    assert!(b.face(Facet::new(0, Orientation::High, whole(9))));
    let body = assay::Claim::new(1, b).encode();
    let mut book = RewardBook::new();
    assert!(matches!(
        book.credit_claim(&body),
        Err(RewardRefused::OpenWork)
    ));
}

#[test]
fn credit_must_cover_price_on_every_axis() {
    let mut book = RewardBook::new();
    book.credit_claim(&closed_box_claim(1, 2).encode())
        .expect("credit");
    assert!(book.covers(&Extent::new(vec![1, 1])));
    assert!(!book.covers(&Extent::new(vec![2, 1])));
    book.settle_against(&Extent::new(vec![1, 1])).expect("ok");
    match book.settle_against(&Extent::new(vec![2, 0])) {
        Err(RewardRefused::Underfunded { .. }) => {}
        other => panic!("expected underfunded, got {other:?}"),
    }
}

#[test]
fn different_structures_stack_credit() {
    let mut book = RewardBook::new();
    book.credit_claim(&closed_box_claim(1, 1).encode())
        .expect("a");
    book.credit_claim(&closed_box_claim(1, 2).encode())
        .expect("b");
    assert_eq!(book.total().components(), &[2, 2]);
}

#[test]
fn onramp_shape_body_in_highway_frame() {
    // Tollway produces shape body; superhighway frames it; court credits
    // the opaque body without knowing the tollway.
    let body = triangle_claim(0).encode();
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frame");
    let (tag, value) = isthmus::work::take_frame(&wire).expect("take");
    assert_eq!(tag, isthmus::work::SHAPE_CLAIM_TAG);
    let mut book = RewardBook::new();
    book.credit_claim(value).expect("credit from frame body");
    assert_eq!(book.total().components(), &[1, 1, 1]);
}
