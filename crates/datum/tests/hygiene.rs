//! H7 — IS-2 §6 transport replay hygiene (secondary, not PoUW identity).
//!
//! Session has no seen-set. Claims re-derive. Effects need identity:
//! primary = work_id; secondary = exact wire bytes at the authority.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use datum::hygiene::{credit_effect, CreditEffectRefused, HygieneRefused, WireHygiene};
use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};
use isthmus::work;

#[test]
fn session_rule_is_not_a_seen_set() {
    // IS-2 §6.1: session step only separates wait / take / refuse by length.
    // No digest lives there.
    let body = closed_box_claim(0, 1).encode();
    let mut frame = Vec::new();
    work::put_shape_claim(&body, &mut frame).expect("frame");
    // Twice through the framing rule — both take.
    let bound = 1 << 20;
    for _ in 0..2 {
        match isthmus::session::step(&isthmus::layout::Layout::founding(), &frame, bound) {
            isthmus::session::Step::Take(n) => assert_eq!(n, frame.len()),
            other => panic!("session should take the frame, got {other:?}"),
        }
    }
}

#[test]
fn authority_drops_identical_effect_wire_bytes() {
    let mut h = WireHygiene::new();
    let mut book = RewardBook::new();
    let body = triangle_claim(0).encode();
    credit_effect(&mut h, &mut book, &body).expect("first credit");
    match credit_effect(&mut h, &mut book, &body) {
        Err(CreditEffectRefused::Wire(HygieneRefused::WireReplay { .. })) => {}
        other => panic!("expected wire hygiene refuse, got {other:?}"),
    }
    // Book still has one credit only
    assert_eq!(book.total().components(), &[1, 1, 1]);
}

#[test]
fn work_id_still_primary_when_wire_differs() {
    let mut h = WireHygiene::new();
    let mut book = RewardBook::new();
    credit_effect(&mut h, &mut book, &closed_box_claim(1, 2).encode()).expect("a");
    match credit_effect(&mut h, &mut book, &closed_box_claim(2, 2).encode()) {
        Err(CreditEffectRefused::Work(RewardRefused::Replay { .. })) => {}
        other => panic!("expected structure replay, got {other:?}"),
    }
}

#[test]
fn framed_shape_body_peel_then_hygiene() {
    // Carrier delivers frame; court peels body; hygiene + book apply.
    let body = triangle_claim(5).encode();
    let mut wire = Vec::new();
    work::put_shape_claim(&body, &mut wire).expect("put");
    let (tag, value) = work::take_frame(&wire).expect("take");
    assert_eq!(tag, work::SHAPE_CLAIM_TAG);

    let mut h = WireHygiene::new();
    let mut book = RewardBook::new();
    credit_effect(&mut h, &mut book, value).expect("credit");
    assert!(h.would_replay(value));
    assert!(matches!(
        credit_effect(&mut h, &mut book, value),
        Err(CreditEffectRefused::Wire(_))
    ));
}
