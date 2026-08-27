//! UC1–UC2 laws: the geometry is data, the evaluator is the invariant.
//!
//! A hexagon and a five-simplex are distinct BYTES; a non-complex
//! refuses; a closure accepts and an open chain refuses by naming the
//! leaking cell; fuel exhaustion is a refusal, never a hang.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{ComplexBroken, DeclaredClaim, DeclaredComplex, Entry, DEFAULT_FUEL};
use assay::whole;

/// The n-cycle: n vertices, n edges, edge i runs cell i → i+1 (mod n).
/// Its full edge-chain with unit coefficients is a cycle: ∂c = 0.
fn cycle(n: u32) -> DeclaredClaim {
    let mut op = Vec::new();
    for i in 0..n {
        // ∂(edge i) = target − source.
        let (source, target) = (i, (i + 1) % n);
        // Canonical order: entries sorted by (col, row).
        let mut pair = vec![
            Entry { row: target, col: i, coeff: whole(1) },
            Entry { row: source, col: i, coeff: whole(-1) },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    DeclaredClaim {
        transport: 0,
        complex: DeclaredComplex {
            cells: vec![n, n],
            ops: vec![op],
        },
        dim: 1,
        witness: (0..n).map(|i| (i, whole(1))).collect(),
    }
}

/// The complete graph on `n` vertices — the five-simplex's 1-skeleton
/// when n = 6. Witness: a triangle cycle inside it.
fn complete_graph(n: u32) -> DeclaredClaim {
    let mut edges = Vec::new();
    for a in 0..n {
        for b in (a + 1)..n {
            edges.push((a, b));
        }
    }
    let mut op = Vec::new();
    for (col, (a, b)) in edges.iter().enumerate() {
        let mut pair = vec![
            Entry { row: *b, col: col as u32, coeff: whole(1) },
            Entry { row: *a, col: col as u32, coeff: whole(-1) },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    // Witness: edges (0,1), (1,2) forward and (0,2) backward close a
    // triangle. Edge indices: (0,1)=0, (0,2)=1, (1,2)=n-1.
    let witness = vec![(0u32, whole(1)), (1, whole(-1)), (n - 1, whole(1))];
    DeclaredClaim {
        transport: 0,
        complex: DeclaredComplex {
            cells: vec![n, edges.len() as u32],
            ops: vec![op],
        },
        dim: 1,
        witness,
    }
}

#[test]
fn a_hexagon_and_a_five_simplex_are_distinct_bytes() {
    let hexagon = cycle(6);
    let simplex = complete_graph(6);
    assert!(hexagon.verify(DEFAULT_FUEL).is_ok(), "the hexagon closes");
    assert!(simplex.verify(DEFAULT_FUEL).is_ok(), "the triangle closes");
    assert_ne!(
        hexagon.encode(),
        simplex.encode(),
        "the tag-51 defect class: six orbs is not a geometry — the \
         incidence is, and it is on the wire"
    );
    assert_ne!(hexagon.work_id(), simplex.work_id());
}

#[test]
fn codec_round_trips_and_transport_does_not_move_identity() {
    let claim = cycle(6);
    let back = DeclaredClaim::decode(&claim.encode()).expect("its own bytes");
    assert_eq!(back, claim);

    let mut moved = claim.clone();
    moved.transport = 99;
    assert_eq!(moved.work_id(), claim.work_id(), "transport is not identity");
    assert_ne!(moved.encode(), claim.encode(), "but it IS on the wire");
}

#[test]
fn an_open_chain_refuses_and_names_the_leaking_cell() {
    let mut open = cycle(6);
    open.witness.pop(); // break the cycle: one edge missing
    match open.verify(DEFAULT_FUEL) {
        Err(ComplexBroken::OpenBoundary { cell }) => {
            assert!(cell == 0 || cell == 5, "the cut leaks at its ends");
        }
        other => panic!("expected OpenBoundary, got {other:?}"),
    }
}

#[test]
fn a_declaration_that_is_not_a_complex_refuses() {
    // Two dimensions of operators where ∂∘∂ ≠ 0: one 2-cell whose
    // boundary is edge 0 alone, while edge 0's own boundary does not
    // vanish. The declaration is refused as a whole.
    let claim = DeclaredClaim {
        transport: 0,
        complex: DeclaredComplex {
            cells: vec![2, 1, 1],
            ops: vec![
                vec![
                    Entry { row: 0, col: 0, coeff: whole(-1) },
                    Entry { row: 1, col: 0, coeff: whole(1) },
                ],
                vec![Entry { row: 0, col: 0, coeff: whole(1) }],
            ],
        },
        dim: 2,
        witness: vec![(0, whole(1))],
    };
    assert_eq!(
        claim.verify(DEFAULT_FUEL),
        Err(ComplexBroken::NotAComplex { dim: 0 }),
        "∂∘∂ = 0 is the axiom the engine holds for every domain"
    );
}

#[test]
fn non_canonical_bytes_refuse() {
    // Same structure, entries out of canonical order: one structure
    // must be one byte string, or work_id is not content-addressed.
    let mut claim = cycle(3);
    if let Some(op) = claim.complex.ops.first_mut() {
        op.swap(0, 1);
    }
    assert_eq!(claim.verify(DEFAULT_FUEL), Err(ComplexBroken::NotCanonical));
}

#[test]
fn fuel_exhaustion_refuses_rather_than_hanging() {
    let big = cycle(64);
    assert!(big.verify(DEFAULT_FUEL).is_ok(), "affordable at the default");
    assert_eq!(
        big.verify(10),
        Err(ComplexBroken::FuelExhausted { budget: 10 }),
        "an axiom pack is code by another name, and this is its meter — \
         the refusal names the budget"
    );
    let spent = big.verify(DEFAULT_FUEL).expect("affordable");
    assert!(spent > 10, "and the meter reads what checking actually cost");
}

#[test]
fn a_zero_chain_witness_closes_trivially_and_empty_refuses() {
    let mut vertices = cycle(4);
    vertices.dim = 0;
    vertices.witness = vec![(0, whole(1)), (2, whole(1))];
    assert!(vertices.verify(DEFAULT_FUEL).is_ok(), "0-chains have no boundary");

    let mut empty = cycle(4);
    empty.witness.clear();
    assert_eq!(empty.verify(DEFAULT_FUEL), Err(ComplexBroken::Empty));
}
