//! UC3's measurement at the court: a declared-domain claim credits
//! multi-axially, and replay refuses across transports — the same
//! closure resubmitted under a new nonce is the same work.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{DeclaredClaim, DeclaredComplex, Entry};
use assay::whole;
use datum::reward::{RewardBook, RewardRefused};

fn hexagon(transport: u64) -> DeclaredClaim {
    let n = 6u32;
    let mut op = Vec::new();
    for i in 0..n {
        let (source, target) = (i, (i + 1) % n);
        let mut pair = vec![
            Entry { row: target, col: i, coeff: whole(1) },
            Entry { row: source, col: i, coeff: whole(-1) },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    DeclaredClaim {
        transport,
        complex: DeclaredComplex {
            cells: vec![n, n],
            ops: vec![op],
        },
        dim: 1,
        witness: (0..n).map(|i| (i, whole(1))).collect(),
    }
}

#[test]
fn a_declared_closure_credits_once_across_transports() {
    let mut book = RewardBook::new();
    let credit = book
        .credit_claim(&hexagon(1).encode())
        .expect("the hexagon closes and credits");
    assert_eq!(
        credit.axes.components(),
        &[6, 6],
        "multi-axial: one component per declared dimension"
    );

    // The same structure under a different transport is the same work.
    match book.credit_claim(&hexagon(2).encode()) {
        Err(RewardRefused::Replay { .. }) => {}
        other => panic!("expected replay, got {other:?}"),
    }
}

#[test]
fn an_open_declared_chain_earns_nothing() {
    let mut open = hexagon(1);
    open.witness.pop();
    let mut book = RewardBook::new();
    assert!(
        book.credit_claim(&open.encode()).is_err(),
        "claims that do not close earn nothing — same law, new domain"
    );
    assert_eq!(book.act_len(), 0);
}

#[test]
fn a_declared_claim_travels_the_highway_like_any_other() {
    // Enveloped under the shape-claim tag, opaque in transit, opened
    // and credited by the court: the substrate needed no change to
    // carry a domain invented after it shipped.
    let body = hexagon(7).encode();
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
    let (tag, carried) = isthmus::work::take_frame(&wire).expect("opens");
    assert_eq!(tag, isthmus::work::SHAPE_CLAIM_TAG);
    let mut book = RewardBook::new();
    assert!(book.credit_claim(carried).is_ok());
}
