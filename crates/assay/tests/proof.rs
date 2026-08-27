//! SQ1 laws: a proof is a chain with a PRESCRIBED boundary.
//!
//! `∂c = target − axioms`: watertight means no missing premises and
//! no dangling conclusions, and the mismatch refusal names the cell.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{
    ComplexBroken, DeclaredComplex, Entry, ProofClaim, DEFAULT_FUEL,
};
use assay::whole;

/// The path universe: vertices v0..=vn, edge i runs v_i → v_{i+1}.
/// The full path chain has boundary v_n − v_0 — axioms to theorem.
fn path(n: u32) -> DeclaredComplex {
    let mut op = Vec::new();
    for i in 0..n {
        op.push(Entry { row: i, col: i, coeff: whole(-1) });
        op.push(Entry { row: i + 1, col: i, coeff: whole(1) });
    }
    DeclaredComplex {
        cells: vec![n + 1, n],
        ops: vec![op],
    }
}

/// The derivation from v0 (axiom) to v3 (theorem): all three steps.
fn derivation() -> ProofClaim {
    ProofClaim {
        transport: 0,
        complex: path(3),
        dim: 1,
        target: vec![(0, whole(-1)), (3, whole(1))], // theorem − axiom
        witness: (0..3).map(|i| (i, whole(1))).collect(),
        deps: Vec::new(),
    }
}

#[test]
fn a_derivation_closes_onto_its_prescribed_boundary() {
    let proof = derivation();
    let spent = proof.verify(DEFAULT_FUEL).expect("axioms reach the theorem");
    assert!(spent > 0, "and the checking had a price");
    assert_eq!(proof.credit_axes(), vec![4, 3]);
}

#[test]
fn a_missing_premise_refuses_and_names_the_cell() {
    let mut gappy = derivation();
    gappy.witness.remove(1); // drop the middle inference step
    match gappy.verify(DEFAULT_FUEL) {
        Err(ComplexBroken::BoundaryMismatch { cell }) => {
            assert!(
                cell == 1 || cell == 2,
                "the gap leaks at the missing step's endpoints, got {cell}"
            );
        }
        other => panic!("expected BoundaryMismatch, got {other:?}"),
    }
}

#[test]
fn a_dangling_conclusion_refuses() {
    // Right derivation, wrong claim: the target says the theorem is
    // v2, but the witness derives all the way to v3.
    let mut overreach = derivation();
    overreach.target = vec![(0, whole(-1)), (2, whole(1))];
    match overreach.verify(DEFAULT_FUEL) {
        Err(ComplexBroken::BoundaryMismatch { cell }) => {
            assert!(cell == 2 || cell == 3);
        }
        other => panic!("expected BoundaryMismatch, got {other:?}"),
    }
}

#[test]
fn an_empty_target_recovers_plain_closure() {
    // The 3-cycle witness against an empty target = ∂c = 0.
    let n = 3u32;
    let mut op = Vec::new();
    for i in 0..n {
        let (s, t) = (i, (i + 1) % n);
        let mut pair = vec![
            Entry { row: t, col: i, coeff: whole(1) },
            Entry { row: s, col: i, coeff: whole(-1) },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    let cycle = ProofClaim {
        transport: 0,
        complex: DeclaredComplex { cells: vec![n, n], ops: vec![op] },
        dim: 1,
        target: Vec::new(),
        witness: (0..n).map(|i| (i, whole(1))).collect(),
        deps: Vec::new(),
    };
    assert!(cycle.verify(DEFAULT_FUEL).is_ok(), "z = ∅ is UC2's law");
}

#[test]
fn codec_round_trips_and_citations_are_identity() {
    let mut proof = derivation();
    proof.deps = vec![vec![1u8; 8], vec![2u8; 8]];
    let back = ProofClaim::decode(&proof.encode()).expect("its own bytes");
    assert_eq!(back, proof);

    // Transport is not identity…
    let mut moved = proof.clone();
    moved.transport = 42;
    assert_eq!(moved.work_id(), proof.work_id());

    // …but the citations ARE: the same derivation standing on
    // different lemmas is a different proof.
    let mut recited = proof.clone();
    recited.deps = vec![vec![3u8; 8]];
    assert_ne!(recited.work_id(), proof.work_id());
}

#[test]
fn citing_lemmas_costs_no_verification_fuel() {
    let plain = derivation();
    let mut heavy = derivation();
    heavy.deps = (0..100).map(|i| vec![i as u8; 32]).collect();
    let a = plain.verify(DEFAULT_FUEL).expect("verifies");
    let b = heavy.verify(DEFAULT_FUEL).expect("verifies");
    assert_eq!(a, b, "the cache is spent as a cache: citations are free here");
}
