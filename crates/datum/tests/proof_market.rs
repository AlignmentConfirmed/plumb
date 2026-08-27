//! SQ2's measurement at the court: citations answer to the book.
//!
//! A lemma settles; a theorem citing it credits; a theorem citing
//! nothing-yet-settled refuses by naming the missing address.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{DeclaredComplex, Entry, ProofClaim};
use assay::whole;
use datum::reward::{RewardBook, RewardRefused};

fn path(n: u32) -> DeclaredComplex {
    let mut op = Vec::new();
    for i in 0..n {
        op.push(Entry { row: i, col: i, coeff: whole(-1) });
        op.push(Entry { row: i + 1, col: i, coeff: whole(1) });
    }
    DeclaredComplex { cells: vec![n + 1, n], ops: vec![op] }
}

/// A derivation from v0 to v_n over the path universe.
fn proof(n: u32, transport: u64, deps: Vec<Vec<u8>>) -> ProofClaim {
    ProofClaim {
        transport,
        complex: path(n),
        dim: 1,
        target: vec![(0, whole(-1)), (n, whole(1))],
        witness: (0..n).map(|i| (i, whole(1))).collect(),
        deps,
    }
}

#[test]
fn the_lemma_market_credits_in_citation_order() {
    let mut book = RewardBook::new();

    // The lemma: a 2-step derivation, settled first.
    let lemma = proof(2, 1, Vec::new());
    let lemma_id = lemma.work_id();
    book.credit_claim(&lemma.encode()).expect("the lemma settles");

    // The theorem: a 5-step derivation standing on the lemma.
    let theorem = proof(5, 1, vec![lemma_id.as_bytes().to_vec()]);
    let credit = book
        .credit_claim(&theorem.encode())
        .expect("a theorem on settled ground credits");
    assert_eq!(credit.axes.components(), &[6, 5]);
}

#[test]
fn citing_the_unsettled_refuses_by_address() {
    let mut book = RewardBook::new();
    let phantom = proof(2, 1, Vec::new()).work_id();
    let theorem = proof(5, 1, vec![phantom.as_bytes().to_vec()]);
    match book.credit_claim(&theorem.encode()) {
        Err(RewardRefused::UnsettledDependency { work_id }) => {
            assert_eq!(work_id, phantom, "the refusal names the missing lemma");
        }
        other => panic!("expected UnsettledDependency, got {other:?}"),
    }

    // Settle the lemma; the same theorem now credits: citation order
    // is settlement order, not submission order.
    book.credit_claim(&proof(2, 9, Vec::new()).encode())
        .expect("the lemma settles under any transport");
    book.credit_claim(&theorem.encode())
        .expect("and the theorem follows it");
}

#[test]
fn a_proof_replays_like_any_other_work() {
    let mut book = RewardBook::new();
    let lemma = proof(2, 1, Vec::new());
    let lemma_id = lemma.work_id();
    book.credit_claim(&lemma.encode()).expect("settles");
    let theorem = proof(4, 1, vec![lemma_id.as_bytes().to_vec()]);
    book.credit_claim(&theorem.encode()).expect("credits");
    match book.credit_claim(&proof(4, 77, vec![lemma_id.as_bytes().to_vec()]).encode()) {
        Err(RewardRefused::Replay { .. }) => {}
        other => panic!("expected replay, got {other:?}"),
    }
}

#[test]
fn a_broken_derivation_earns_nothing_even_on_settled_ground() {
    let mut book = RewardBook::new();
    let lemma = proof(2, 1, Vec::new());
    let lemma_id = lemma.work_id();
    book.credit_claim(&lemma.encode()).expect("settles");
    let mut gappy = proof(5, 1, vec![lemma_id.as_bytes().to_vec()]);
    gappy.witness.remove(2);
    assert!(matches!(
        book.credit_claim(&gappy.encode()),
        Err(RewardRefused::OpenWork)
    ));
    assert_eq!(book.act_len(), 1, "citations license nothing by themselves");
}
