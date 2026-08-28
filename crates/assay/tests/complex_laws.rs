//! THE COMPLEX LAWS — the universal checker's whole suite in one
//! binary: declared universes, prescribed-boundary proofs, and
//! the Shape verdict-equivalence corpus.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod declared {


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
}

mod proof {


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
}

mod shape_equivalence {


    use assay::complex::{from_shape, DEFAULT_FUEL};
    use assay::shape::Shape;
    use assay::whole;
    use num_bigint::BigInt;
    use num_rational::Ratio;

    /// An exact p/q, test-local.
    fn ratio(p: i64, q: i64) -> assay::Exact {
        Ratio::new(BigInt::from(p), BigInt::from(q))
    }

    /// The equivalence predicate: compiled verdict == declared verdict.
    fn verdicts_agree(shape: &Shape) -> bool {
        let compiled = shape.admit().is_ok();
        let declared = from_shape(shape)
            .and_then(|claim| claim.verify(DEFAULT_FUEL))
            .is_ok();
        compiled == declared
    }

    fn triangle() -> Shape {
        let mut s = Shape::new(3);
        s.edge(0, 1, whole(1)).expect("edge");
        s.edge(1, 2, whole(2)).expect("edge");
        s.edge(0, 2, whole(-3)).expect("edge");
        s
    }

    #[test]
    fn the_corpus_reaches_the_same_verdicts() {
        // Valid shapes of different characters: cyclic, acyclic, star,
        // minimal, rationally charged.
        let mut path = Shape::new(4);
        path.edge(0, 1, whole(1)).expect("edge");
        path.edge(1, 2, whole(1)).expect("edge");
        path.edge(2, 3, whole(5)).expect("edge");

        let mut star = Shape::new(5);
        for leaf in 1..5 {
            star.edge(0, leaf, whole(leaf as i64)).expect("edge");
        }

        let mut single = Shape::new(2);
        single.edge(0, 1, whole(-7)).expect("edge");

        let mut rational = Shape::new(3);
        rational
            .edge(0, 1, ratio(22, 7))
            .expect("edge");
        rational.edge(1, 2, ratio(-1, 3)).expect("edge");

        // Invalid shapes the API can construct.
        let no_orbs = Shape::new(0);
        let no_edges = Shape::new(6);

        for (name, shape) in [
            ("triangle", &triangle()),
            ("path", &path),
            ("star", &star),
            ("single edge", &single),
            ("rational charges", &rational),
            ("no orbs", &no_orbs),
            ("no edges", &no_edges),
        ] {
            assert!(
                verdicts_agree(shape),
                "compiled and declared verdicts disagree on: {name}"
            );
        }
    }

    #[test]
    fn the_translation_is_deterministic_bytes() {
        // Same shape, same bytes — the declared re-expression keeps
        // content-addressing.
        let a = from_shape(&triangle()).expect("translates");
        let b = from_shape(&triangle()).expect("translates");
        assert_eq!(a.encode(), b.encode());
        assert_eq!(a.work_id(), b.work_id());
    }

    #[test]
    fn charges_survive_the_translation_exactly() {
        // The declared operator carries ±charge per endpoint — exact,
        // not normalized away. 22/7 stays 22/7.
        let claim = from_shape(&{
            let mut s = Shape::new(2);
            s.edge(0, 1, ratio(22, 7)).expect("edge");
            s
        })
        .expect("translates");
        let op = claim.complex.ops.first().expect("one operator");
        assert!(op.iter().any(|e| e.coeff == ratio(22, 7)));
        assert!(op.iter().any(|e| e.coeff == ratio(-22, 7)));
    }
}
