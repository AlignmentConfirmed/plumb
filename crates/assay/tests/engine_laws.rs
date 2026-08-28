//! ENGINE LAWS — the whole physics in one binary: convergence,
//! shapes, portable work, and the universal checker.
//! (isolation.rs stays standalone: it is the crate's standing
//! claim, referenced by name.)

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod complex_laws {


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
}

mod convergence_laws {

    use assay::{
        assess, exact, whole, zero, Boundary, Convergence, Extent, Facet, Orientation, Upsilon,
    };

    /// A closed box: on every axis, the two faces carry the same flux, so
    /// the signed sum cancels exactly.
    fn closed(axes: usize) -> Boundary {
        let mut boundary = Boundary::new(axes);
        for axis in 0..axes {
            let flux = whole(i64::try_from(axis).unwrap_or(0) + 1);
            assert!(boundary.face(Facet::new(axis, Orientation::Low, flux.clone())));
            assert!(boundary.face(Facet::new(axis, Orientation::High, flux)));
        }
        boundary
    }

    // ===================================================================
    // V1 — the witness is minted only from a real, complete measurement
    // ===================================================================

    /// **A closed boundary mints, at every dimensionality.**
    #[test]
    fn v1_a_closed_boundary_mints_the_witness() {
        for axes in 1..=11usize {
            let verdict = assess(&closed(axes));
            assert!(
                matches!(verdict, Convergence::Closed(_)),
                "a closed {axes}-axis boundary did not close: {verdict:?}",
            );
            assert!(verdict.witness().is_some(), "no witness at {axes} axes");
            assert!(
                verdict.residue().is_none(),
                "a closed boundary reported a residue",
            );
        }
    }

    /// **Nothing measured is not closure**, and **half a surface is not a
    /// surface.** Both would be gates that cannot fail.
    #[test]
    fn v1_an_unmeasured_or_half_described_boundary_mints_nothing() {
        // No faces at all — the divergence is trivially zero on every axis.
        for axes in 0..=5usize {
            let verdict = assess(&Boundary::new(axes));
            assert_eq!(
                verdict,
                Convergence::Unmeasured,
                "an empty {axes}-axis boundary was assessed as something else",
            );
            assert!(verdict.witness().is_none(), "nothing measured minted a witness");
        }

        // One face on an axis. Its flux cancels against nothing.
        for missing in [Orientation::Low, Orientation::High] {
            let mut boundary = Boundary::new(2);
            assert!(boundary.face(Facet::new(0, Orientation::Low, whole(3))));
            assert!(boundary.face(Facet::new(0, Orientation::High, whole(3))));
            assert!(boundary.face(Facet::new(1, missing.opposite(), zero())));

            let verdict = assess(&boundary);
            assert_eq!(
                verdict,
                Convergence::Incomplete { axis: 1, missing },
                "a boundary missing the {missing:?} face of axis 1 was accepted",
            );
            assert!(verdict.witness().is_none());
        }

        // A facet on an axis the boundary does not span is refused at the
        // door rather than widening it.
        let mut narrow = Boundary::new(1);
        assert!(!narrow.face(Facet::new(4, Orientation::High, whole(1))));
        assert!(narrow.is_empty(), "a refused facet was still recorded");
    }

    // ===================================================================
    // V2 — the verdict is structural, never a boolean
    // ===================================================================

    /// **Every open verdict carries its residue.** Rule 4: a threshold that
    /// answers yes or no forces the caller to guess what it was near.
    #[test]
    fn v2_every_open_verdict_carries_the_residue_that_produced_it() {
        let mut open = Boundary::new(3);
        for axis in 0..3usize {
            assert!(open.face(Facet::new(axis, Orientation::Low, whole(1))));
            assert!(open.face(Facet::new(axis, Orientation::High, whole(4))));
        }
        let verdict = assess(&open);

        let residue = verdict
            .residue()
            .unwrap_or_else(|| panic!("an open verdict carried no residue: {verdict:?}"));
        assert_eq!(residue.axes(), 3, "the residue lost an axis");
        for axis in 0..3 {
            assert_eq!(
                residue.component(axis),
                Some(&whole(3)),
                "axis {axis}'s residue is not 4 − 1",
            );
        }
        assert!(matches!(verdict, Convergence::Open { .. }));
        assert!(verdict.witness().is_none());
    }

    /// **Fractions survive.** Rule 2: the flux is exact, so a boundary that
    /// closes only when thirds are kept exactly still closes.
    #[test]
    fn v2_a_boundary_that_closes_only_in_exact_arithmetic_closes() {
        let third = exact(1, 3).unwrap_or_else(zero);
        let mut boundary = Boundary::new(1);
        // Three thirds against one whole. In floating point this is a
        // rounding away from closure; here it is closure.
        for _ in 0..3 {
            assert!(boundary.face(Facet::new(0, Orientation::High, third.clone())));
        }
        assert!(boundary.face(Facet::new(0, Orientation::Low, whole(1))));

        assert!(
            matches!(assess(&boundary), Convergence::Closed(_)),
            "three exact thirds did not cancel one whole",
        );

        // And a boundary one third away from closing does not close.
        let mut off = Boundary::new(1);
        assert!(off.face(Facet::new(0, Orientation::High, third.clone())));
        assert!(off.face(Facet::new(0, Orientation::Low, whole(1))));
        match assess(&off) {
            Convergence::Open { residue } => {
                assert_eq!(residue.component(0), Some(&(third - whole(1))));
            }
            other => panic!("a boundary one third open answered {other:?}"),
        }
    }

    // ===================================================================
    // V3 — THE TOTAL IS NOT THE TEST
    // ===================================================================

    /// **A boundary whose axes cancel each other is not closed.**
    ///
    /// `+1` on one axis against `−1` on another would total to zero while
    /// the manifold is open in two directions. An engine that summed the
    /// axes before comparing to zero would mint a witness here — so this is
    /// the law rule 3 exists for. It lands in `Open`, carrying the residue
    /// that says exactly which axes moved and by how much.
    #[test]
    fn v3_axes_that_cancel_each_other_do_not_close() {
        let mut boundary = Boundary::new(2);
        // Axis 0 diverges by +2.
        assert!(boundary.face(Facet::new(0, Orientation::Low, whole(1))));
        assert!(boundary.face(Facet::new(0, Orientation::High, whole(3))));
        // Axis 1 diverges by −2.
        assert!(boundary.face(Facet::new(1, Orientation::Low, whole(3))));
        assert!(boundary.face(Facet::new(1, Orientation::High, whole(1))));

        let verdict = assess(&boundary);
        let residue = verdict.residue().expect("an open verdict has a residue");

        // The trap, made explicit WITHOUT taking the total: the two
        // components are exact negatives of each other, so anything that
        // added them would see zero. Stated as a relation between the
        // components rather than as a sum, because the crate no longer has
        // a `sum()` and should not: adding flux across axes is the
        // flattening it exists to refuse.
        assert_eq!(
            residue.component(0).map(|c| -c.clone()),
            residue.component(1).cloned(),
            "this law's premise needs the two axes to be exact negatives",
        );
        assert!(!residue.is_zero(), "and the axes must NOT cancel");

        assert!(
            matches!(verdict, Convergence::Open { .. }),
            "axes cancelling each other answered {verdict:?}",
        );
        assert!(
            verdict.witness().is_none(),
            "A WITNESS WAS MINTED FOR A MANIFOLD OPEN IN TWO DIRECTIONS",
        );

        // And a boundary open the same way on both axes lands in the same
        // arm, carrying a different residue. The residue is the
        // distinction; a label computed from a fold was not.
        let mut plain = Boundary::new(2);
        assert!(plain.face(Facet::new(0, Orientation::Low, whole(1))));
        assert!(plain.face(Facet::new(0, Orientation::High, whole(3))));
        assert!(plain.face(Facet::new(1, Orientation::Low, whole(1))));
        assert!(plain.face(Facet::new(1, Orientation::High, whole(3))));
        assert!(matches!(assess(&plain), Convergence::Open { .. }));
    }

    /// **`Extent::is_zero` is per component and empty is not zero.**
    ///
    /// The closure predicate itself, checked directly — if it folded, `v3`
    /// would be the only thing standing between a cancelling boundary and a
    /// witness, and one law is not enough for the property the whole crate
    /// is about.
    #[test]
    fn v3_the_closure_predicate_is_per_component() {
        assert!(Extent::zeroed(3).is_zero(), "three exact zeros are zero");
        assert!(!Extent::new(vec![]).is_zero(), "nothing measured is not zero");
        assert!(
            !Extent::new(vec![whole(1), whole(-1)]).is_zero(),
            "components that cancel in total are not each zero",
        );
        // And they are exact negatives — the trap — stated without adding
        // them, because `sum()` is struck.
        let trap = Extent::new(vec![whole(1), whole(-1)]);
        assert_eq!(
            trap.component(0).map(|c| -c.clone()),
            trap.component(1).cloned(),
            "the two components are not negatives — the trap is not set",
        );

        // Extents of different lengths do not add: padding would assert a
        // measurement nobody took.
        assert_eq!(Extent::zeroed(2).add(&Extent::zeroed(3)), None);
        assert_eq!(
            Extent::new(vec![whole(2)]).add(&Extent::new(vec![whole(-2)])),
            Some(Extent::new(vec![zero()])),
        );
    }

    // ===================================================================
    // V4 — gauge invariance
    // ===================================================================

    /// **Only disagreement is observable — on a balanced boundary.**
    ///
    /// Re-gauging moves both faces of an axis together, so the signed sum
    /// is unchanged and the verdict is invariant. The same property the
    /// substrate's cocycle verification rests on.
    ///
    /// The condition is real and was measured: a first version of this law
    /// claimed invariance unconditionally and failed against an axis with
    /// two high faces and one low, where the shift enters the sum twice
    /// positively and once negatively. `v4b` holds that case, with the
    /// exact amount the gauge becomes visible by.
    #[test]
    fn v4_a_re_gauge_changes_no_verdict_on_a_balanced_boundary() {
        let shifts = [whole(0), whole(1), whole(-7), exact(5, 3).unwrap_or_else(zero)];

        for axes in 1..=4usize {
            // Closed, and open-but-balanced: one face at each end of every
            // axis, with axis 0's ends disagreeing.
            let mut open = closed(axes);
            open = open.regauged(usize::MAX, &zero()); // a no-op; keeps the shape
            let mut divergent = Boundary::new(axes);
            for axis in 0..axes {
                assert!(divergent.face(Facet::new(axis, Orientation::Low, whole(1))));
                assert!(divergent.face(Facet::new(
                    axis,
                    Orientation::High,
                    whole(if axis == 0 { 6 } else { 1 }),
                )));
            }

            for boundary in [closed(axes), open, divergent] {
                assert!(boundary.is_balanced(), "this law's premise is a balanced boundary");
                let before = assess(&boundary);
                for axis in 0..axes {
                    for shift in &shifts {
                        let moved = boundary.regauged(axis, shift);
                        assert_eq!(
                            assess(&moved),
                            before,
                            "re-gauging axis {axis} by {shift} changed the verdict",
                        );
                    }
                }
            }
        }
    }

    /// **On an unbalanced axis the gauge IS observable, by exactly the
    /// count difference.**
    ///
    /// Stated as a law rather than left as a caveat, because an engine that
    /// promised invariance it does not have would let a caller re-gauge its
    /// way from divergent to closed — and this is the arithmetic that says
    /// how far.
    #[test]
    fn v4b_an_unbalanced_axis_moves_by_the_face_count_difference() {
        // Axis 0: two high faces, one low. Axis 1: balanced.
        let mut boundary = Boundary::new(2);
        assert!(boundary.face(Facet::new(0, Orientation::Low, whole(2))));
        assert!(boundary.face(Facet::new(0, Orientation::High, whole(1))));
        assert!(boundary.face(Facet::new(0, Orientation::High, whole(1))));
        assert!(boundary.face(Facet::new(1, Orientation::Low, whole(4))));
        assert!(boundary.face(Facet::new(1, Orientation::High, whole(4))));

        assert!(!boundary.is_balanced(), "this law's premise is an UNbalanced axis");
        assert_eq!(boundary.faces_on(0), (1, 2));
        assert_eq!(boundary.faces_on(1), (1, 1));

        // As built it closes: 1 + 1 − 2 = 0 on axis 0, 4 − 4 = 0 on axis 1.
        assert!(
            matches!(assess(&boundary), Convergence::Closed(_)),
            "the premise needs a boundary that starts closed",
        );

        for shift in [whole(1), whole(-3), exact(1, 2).unwrap_or_else(zero)] {
            let moved = boundary.regauged(0, &shift);
            let residue = assess(&moved)
                .residue()
                .cloned()
                .unwrap_or_else(|| panic!("re-gauging an unbalanced axis by {shift} stayed closed"));

            // high − low = 2 − 1 = 1, so the divergence moves by exactly
            // one shift.
            assert_eq!(
                residue.component(0),
                Some(&shift),
                "axis 0 moved by something other than (high − low) × shift",
            );
            assert_eq!(
                residue.component(1),
                Some(&zero()),
                "a re-gauge on axis 0 moved axis 1",
            );
        }

        // And the balanced axis stays invariant under its own re-gauge.
        for shift in [whole(1), whole(-9)] {
            assert!(
                matches!(assess(&boundary.regauged(1, &shift)), Convergence::Closed(_)),
                "re-gauging the BALANCED axis changed the verdict",
            );
        }
    }

    /// **And the gauge is real**: it moves the facets it is applied to, so
    /// `v4` is not passing because `regauged` is the identity.
    #[test]
    fn v4_the_re_gauge_actually_moves_the_boundary() {
        let boundary = closed(2);
        let moved = boundary.regauged(0, &whole(5));
        assert_ne!(boundary, moved, "regauged returned the boundary unchanged");

        let touched = moved
            .facets()
            .iter()
            .zip(boundary.facets().iter())
            .filter(|(after, before)| after.flux != before.flux)
            .count();
        assert_eq!(touched, 2, "a re-gauge on one axis must move exactly its two faces");
    }

    // ===================================================================
    // V5 — the witness carries nothing and cannot be built
    // ===================================================================

    /// **Zero-sized, and every one equal to every other.**
    ///
    /// Rule 5: no token, no identifier, no payload. Two witnesses that
    /// differed would be two witnesses somebody could tell apart, and a
    /// proof you can distinguish is a proof you can select.
    #[test]
    fn v5_the_witness_is_zero_sized_and_carries_nothing() {
        assert_eq!(
            std::mem::size_of::<Upsilon>(),
            0,
            "Upsilon is not zero-sized — it carries something",
        );

        let one = assess(&closed(3)).witness().expect("a closed boundary mints");
        let two = assess(&closed(7)).witness().expect("a closed boundary mints");
        assert_eq!(one, two, "two witnesses are distinguishable");

        // It carries nothing about the manifold it came from: the debug
        // rendering of a proof from a 3-axis box and from a 7-axis one are
        // the same string, because there is nothing in them to differ.
        assert_eq!(format!("{one:?}"), format!("{two:?}"));
    }

    // `Upsilon(())` cannot be written here, and that is the mechanism.
    //
    // The field is private, so this file — an integration test, compiled
    // as a separate crate — cannot construct one. The line
    //
    //     let forged = Upsilon(());
    //
    // does not compile: "cannot initialize a tuple struct which contains
    // private fields". It is left as a comment rather than a
    // `compile_fail` doctest because the crate's own doctests run inside
    // the crate, where it WOULD compile, and a gate that passes for the
    // wrong reason is worse than none.
    //
    // What is asserted instead is the consequence: the only way to obtain
    // one is `assess`, and `assess` is the function the laws above pin.
}

mod shape_laws {

    use assay::shape::{triangle_claim, Shape, ShapeBroken, ShapeClaim};
    use assay::whole;
    use assay::work::{WorkBody, DOMAIN_BOUNDARY};

    #[test]
    fn triangle_admits_and_round_trips() {
        let claim = triangle_claim(7);
        claim.verify().expect("admit");
        let bytes = claim.encode();
        let back = ShapeClaim::decode(&bytes).expect("decode");
        assert_eq!(back.transport, 7);
        assert_eq!(back.shape.orbs(), 3);
        assert_eq!(back.shape.edges().len(), 3);
        assert_eq!(back.shape.credit_axes(), vec![1, 1, 1]);
    }

    #[test]
    fn work_id_ignores_transport() {
        let a = triangle_claim(1).work_id();
        let b = triangle_claim(99).work_id();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_shape_is_not_useful() {
        let s = Shape::new(3);
        assert!(matches!(s.admit(), Err(ShapeBroken::Empty)));
        assert!(s.credit_axes().is_empty());
    }

    #[test]
    fn self_loop_and_zero_charge_refuse() {
        let mut s = Shape::new(2);
        assert!(matches!(
            s.edge(0, 0, whole(1)),
            Err(ShapeBroken::BadEdge { .. })
        ));
        assert!(matches!(
            s.edge(0, 1, whole(0)),
            Err(ShapeBroken::ZeroCharge { .. })
        ));
    }

    #[test]
    fn work_body_dispatches_domains() {
        let shape = triangle_claim(0).encode();
        match WorkBody::parse(&shape).expect("shape") {
            WorkBody::Shape(s) => assert!(s.verify().is_ok()),
            other => panic!("expected shape, got {other:?}"),
        }
        // boundary domain still works
        let mut b = assay::Boundary::new(1);
        assert!(b.face(assay::Facet::new(
            0,
            assay::Orientation::Low,
            whole(1)
        )));
        assert!(b.face(assay::Facet::new(
            0,
            assay::Orientation::High,
            whole(1)
        )));
        let bound = assay::Claim::new(0, b).encode();
        assert_eq!(bound[0], DOMAIN_BOUNDARY);
        match WorkBody::parse(&bound).expect("boundary") {
            WorkBody::Boundary(c) => assert!(c.verify().is_some()),
            other => panic!("expected boundary, got {other:?}"),
        }
    }

    #[test]
    fn edge_order_normalised_for_work_id() {
        let mut s1 = Shape::new(2);
        s1.edge(0, 1, whole(3)).unwrap();
        let mut s2 = Shape::new(2);
        s2.edge(1, 0, whole(3)).unwrap(); // swapped endpoints
        assert_eq!(
            ShapeClaim::new(0, s1).work_id(),
            ShapeClaim::new(0, s2).work_id()
        );
    }
}

mod work_laws {

    use assay::work::{credit_axes, Claim};
    use assay::{whole, Boundary, Facet, Orientation};

    fn closed_box(nonce: u64) -> Claim {
        let mut b = Boundary::new(2);
        let f = whole(3);
        assert!(b.face(Facet::new(0, Orientation::Low, f.clone())));
        assert!(b.face(Facet::new(0, Orientation::High, f.clone())));
        assert!(b.face(Facet::new(1, Orientation::Low, f.clone())));
        assert!(b.face(Facet::new(1, Orientation::High, f)));
        Claim::new(nonce, b)
    }

    #[test]
    fn a_closed_claim_mints_and_round_trips() {
        let claim = closed_box(7);
        assert!(claim.produce().is_some());
        let bytes = claim.encode();
        let back = Claim::decode(&bytes).expect("decode");
        assert_eq!(back.nonce, 7);
        assert!(back.verify().is_some());
        assert_eq!(credit_axes(&back), vec![1, 1]);
    }

    #[test]
    fn an_open_claim_earns_nothing() {
        let mut b = Boundary::new(1);
        // both faces, unequal flux → open
        assert!(b.face(Facet::new(0, Orientation::Low, whole(1))));
        assert!(b.face(Facet::new(0, Orientation::High, whole(2))));
        let claim = Claim::new(1, b);
        assert!(claim.produce().is_none());
        assert!(credit_axes(&claim).is_empty());
    }

    #[test]
    fn unmeasured_earns_nothing() {
        let claim = Claim::new(2, Boundary::new(2));
        assert!(claim.produce().is_none());
        assert!(credit_axes(&claim).is_empty());
    }

    #[test]
    fn hostile_trailing_bytes_refuse() {
        let mut bytes = closed_box(3).encode();
        bytes.push(0xff);
        assert!(Claim::decode(&bytes).is_err());
    }

    #[test]
    fn work_id_is_structure_not_transport() {
        let a = closed_box(1);
        let b = closed_box(999);
        assert_eq!(a.work_id(), b.work_id());
        // different flux → different structure
        let mut boundary = Boundary::new(2);
        let f = whole(9);
        assert!(boundary.face(Facet::new(0, Orientation::Low, f.clone())));
        assert!(boundary.face(Facet::new(0, Orientation::High, f.clone())));
        assert!(boundary.face(Facet::new(1, Orientation::Low, f.clone())));
        assert!(boundary.face(Facet::new(1, Orientation::High, f)));
        let other = Claim::new(1, boundary);
        assert_ne!(a.work_id(), other.work_id());
    }
}

mod rewrite_laws {
    //! SQ3: the calculus compiles soundly — every 1-cell is a legal
    //! rewrite (swept programmatically), and an illegal inference
    //! FAILS TO EXIST as a cell rather than merely being refused.

    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
    #![allow(clippy::indexing_slicing)]

    use assay::complex::DEFAULT_FUEL;
    use assay::rewrite::{Presentation, RewriteBroken};

    /// The sorting monoid: ⟨a, b | ba → ab⟩.
    fn sorting() -> Presentation {
        Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![(vec![b'b', b'a'], vec![b'a', b'b'])],
        }
    }

    #[test]
    fn every_compiled_cell_is_a_legal_rewrite_swept_independently() {
        let compiled = sorting().compile(3).expect("compiles");
        assert_eq!(compiled.words.len(), 15, "ε + 2 + 4 + 8");
        assert_eq!(compiled.steps.len(), 5, "the five occurrences of 'ba'");

        // The sweep re-derives every step by plain string surgery —
        // an implementation-independent check of the whole universe.
        for step in &compiled.steps {
            let from = &compiled.words[step.from];
            let to = &compiled.words[step.to];
            assert_eq!(&from[step.at..step.at + 2], b"ba", "the rule matched");
            let mut expected = from.clone();
            expected[step.at] = b'a';
            expected[step.at + 1] = b'b';
            assert_eq!(to, &expected, "and produced exactly the rewrite");
        }

        // And the compiled universe is a lawful complex.
        compiled.complex.admit(DEFAULT_FUEL).expect("a complex");
    }

    #[test]
    fn an_illegal_inference_fails_to_exist_as_a_cell() {
        let compiled = sorting().compile(3).expect("compiles");
        let ab = compiled.word(b"ab").expect("in universe");
        let ba = compiled.word(b"ba").expect("in universe");

        // The licensed direction exists…
        assert!(compiled.step_between(ba, ab).is_some());
        // …its REVERSE does not — not refused, ABSENT. The calculus
        // cannot express the illegal inference at all.
        assert!(compiled.step_between(ab, ba).is_none());
        assert_eq!(
            compiled.derive(&[ab, ba]),
            Err(RewriteBroken::NoLicensedStep { from: ab, to: ba })
        );
    }

    #[test]
    fn the_derivation_closes_onto_its_theorem() {
        let compiled = sorting().compile(3).expect("compiles");
        let bba = compiled.word(b"bba").expect("axiom");
        let bab = compiled.word(b"bab").expect("midpoint");
        let abb = compiled.word(b"abb").expect("theorem");

        let proof = assay::complex::ProofClaim {
            transport: 0,
            complex: compiled.complex.clone(),
            dim: 1,
            target: compiled.target(bba, abb).expect("target"),
            witness: compiled.derive(&[bba, bab, abb]).expect("two licensed steps"),
            deps: Vec::new(),
        };
        proof.verify(DEFAULT_FUEL).expect("bba → bab → abb, watertight");

        // The gappy derivation refuses at the evaluator, as SQ1 law.
        let mut gappy = proof;
        gappy.witness = compiled.derive(&[bba, bab]).expect("one step");
        assert!(gappy.verify(DEFAULT_FUEL).is_err(), "half a proof is no proof");
    }
}

mod confluence {
    //! SQ6: two derivations of one lemma verifiably commute — the
    //! diamond is a compiled 2-cell, and the commutation certificate
    //! is verified by the same evaluator as everything else, one
    //! dimension up.

    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use assay::complex::{ProofClaim, DEFAULT_FUEL};
    use assay::rewrite::Presentation;
    use assay::whole;

    #[test]
    fn the_baba_diamond_commutes_by_certificate() {
        let compiled = Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![(vec![b'b', b'a'], vec![b'a', b'b'])],
        }
        .compile(4)
        .expect("compiles")
        .with_confluences()
        .expect("confluences");

        assert_eq!(compiled.complex.cells.len(), 3, "the third dimension exists");
        let diamonds = *compiled.complex.cells.get(2).expect("count");
        assert!(diamonds >= 1, "baba branches and rejoins");
        compiled.complex.admit(DEFAULT_FUEL).expect("still a complex: dd = 0 held");

        // The two derivations baba → abab, one through abba and one
        // through baab.
        let (baba, abba, baab, abab) = (
            compiled.word(b"baba").expect("w"),
            compiled.word(b"abba").expect("w"),
            compiled.word(b"baab").expect("w"),
            compiled.word(b"abab").expect("w"),
        );
        let left = compiled.derive(&[baba, abba, abab]).expect("left path");
        let right = compiled.derive(&[baba, baab, abab]).expect("right path");
        assert_ne!(left, right, "genuinely different derivations");

        // Their difference is filled by a compiled diamond: the
        // commutation certificate, verified as a proof claim one
        // dimension up.
        let mut difference: std::collections::BTreeMap<u32, assay::Exact> =
            std::collections::BTreeMap::new();
        for (cell, coeff) in &left {
            *difference.entry(*cell).or_insert_with(|| whole(0)) += coeff.clone();
        }
        for (cell, coeff) in &right {
            *difference.entry(*cell).or_insert_with(|| whole(0)) -= coeff.clone();
        }
        let target: Vec<(u32, assay::Exact)> = difference
            .into_iter()
            .filter(|(_, c)| !num_traits::Zero::is_zero(c))
            .collect();

        // Find WHICH diamond fills it by asking the evaluator.
        let commutes = (0..diamonds).any(|square| {
            ProofClaim {
                transport: 0,
                complex: compiled.complex.clone(),
                dim: 2,
                target: target.clone(),
                witness: vec![(square, whole(1))],
                deps: Vec::new(),
            }
            .verify(DEFAULT_FUEL)
            .is_ok()
        });
        assert!(commutes, "a compiled diamond fills the difference exactly");
    }
}

mod solve {
    //! Task #36: `DeclaredComplex::solve` finds a witness via linear
    //! algebra over `∂` (Smith Normal Form) rather than walking
    //! anything. Every witness it returns still has to pass the same
    //! `closes_to`/`ProofClaim::verify` as any other — these tests
    //! check that independently, never trusting `solve` on its own
    //! say-so.

    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use assay::complex::{DeclaredComplex, Entry, ProofClaim, SolveRefused, DEFAULT_FUEL};
    use assay::rewrite::Presentation;
    use assay::whole;

    #[test]
    fn a_simple_two_point_boundary_solves_at_dimension_one() {
        // 0 -> 1 -> 2, the same shape sdk::derivation's BFS handles —
        // solve() must agree a witness exists here too.
        let edge = |from: u32, to: u32, col: u32| {
            let mut pair = vec![
                Entry { row: from, col, coeff: whole(-1) },
                Entry { row: to, col, coeff: whole(1) },
            ];
            pair.sort_by_key(|e| (e.col, e.row));
            pair
        };
        let mut op = Vec::new();
        op.extend(edge(0, 1, 0));
        op.extend(edge(1, 2, 1));
        let complex = DeclaredComplex {
            cells: vec![3, 2],
            ops: vec![op],
        };
        let target = vec![(0u32, whole(-1)), (2u32, whole(1))];
        let witness = complex.solve(1, &target).expect("0->1->2 is licensed");
        complex
            .closes_to(1, &witness, &target, DEFAULT_FUEL)
            .expect("solve()'s own witness must independently verify");
    }

    #[test]
    fn disjoint_components_refuse_with_no_integral_solution() {
        let mut op = Vec::new();
        op.extend([
            Entry { row: 0, col: 0, coeff: whole(-1) },
            Entry { row: 1, col: 0, coeff: whole(1) },
        ]);
        let complex = DeclaredComplex {
            cells: vec![4, 1], // cells 2,3 have no edge touching them at all
            ops: vec![op],
        };
        let target = vec![(0u32, whole(-1)), (2u32, whole(1))];
        assert_eq!(complex.solve(1, &target), Err(SolveRefused::NoIntegralSolution));
    }

    #[test]
    fn a_nonexistent_dimension_refuses_by_name() {
        let complex = DeclaredComplex {
            cells: vec![1],
            ops: vec![],
        };
        assert_eq!(complex.solve(1, &[]), Err(SolveRefused::NoSuchDimension));
    }

    /// The whole point of task #36 (over `sdk::derivation`'s BFS,
    /// which is hard-scoped to dimension 1): `solve` works at
    /// dimension 2, over the confluence cells `with_confluences`
    /// compiles — a boundary matrix that is not a graph incidence
    /// matrix at all, and total unimodularity no longer applies.
    /// A directed walk over 1-cells cannot even pose this question.
    #[test]
    fn solve_finds_the_filling_diamond_at_dimension_two() {
        let compiled = Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![(vec![b'b', b'a'], vec![b'a', b'b'])],
        }
        .compile(4)
        .expect("compiles")
        .with_confluences()
        .expect("confluences");

        let (baba, abba, baab, abab) = (
            compiled.word(b"baba").expect("w"),
            compiled.word(b"abba").expect("w"),
            compiled.word(b"baab").expect("w"),
            compiled.word(b"abab").expect("w"),
        );
        let left = compiled.derive(&[baba, abba, abab]).expect("left path");
        let right = compiled.derive(&[baba, baab, abab]).expect("right path");

        let mut difference: std::collections::BTreeMap<u32, assay::Exact> =
            std::collections::BTreeMap::new();
        for (cell, coeff) in &left {
            *difference.entry(*cell).or_insert_with(|| whole(0)) += coeff.clone();
        }
        for (cell, coeff) in &right {
            *difference.entry(*cell).or_insert_with(|| whole(0)) -= coeff.clone();
        }
        let target: Vec<(u32, assay::Exact)> = difference
            .into_iter()
            .filter(|(_, c)| !num_traits::Zero::is_zero(c))
            .collect();

        let witness = compiled
            .complex
            .solve(2, &target)
            .expect("a diamond fills the difference — linear algebra, not a brute-force scan");

        ProofClaim {
            transport: 0,
            complex: compiled.complex.clone(),
            dim: 2,
            target,
            witness,
            deps: Vec::new(),
        }
        .verify(DEFAULT_FUEL)
        .expect("solve()'s own dimension-2 witness independently verifies");
    }

    /// Task #37: `solve_sparsest` genuinely finds a sparser witness
    /// than the SNF particular solution `solve` returns — the whole
    /// reason Basis Pursuit exists, demonstrated on the same
    /// underdetermined shape `assay::simplex`'s own unit test uses,
    /// now wired through a real `DeclaredComplex` and independently
    /// re-verified by `closes_to`, never trusted on `solve_sparsest`'s
    /// own say-so.
    #[test]
    fn solve_sparsest_beats_the_naive_particular_solution() {
        // 4 nodes, 4 edges: a 3-hop path 0->1->2->3, plus one direct
        // shortcut edge 0->3. SNF's particular solution (pivot order
        // is an algebraic accident, not a shortest-path search) picks
        // up the whole 3-edge path; the sparsest closing chain is the
        // single shortcut edge alone. Confirmed empirically, not
        // assumed — this is exactly the gap #37 exists to close.
        let edge = |from: u32, to: u32, col: u32| {
            vec![
                Entry { row: from, col, coeff: whole(-1) },
                Entry { row: to, col, coeff: whole(1) },
            ]
        };
        let mut op = Vec::new();
        op.extend(edge(0, 1, 0));
        op.extend(edge(1, 2, 1));
        op.extend(edge(2, 3, 2));
        op.extend(edge(0, 3, 3)); // the shortcut
        let complex = DeclaredComplex {
            cells: vec![4, 4],
            ops: vec![op],
        };
        let target = vec![(0u32, whole(-1)), (3u32, whole(1))];

        let naive = complex.solve(1, &target).expect("Q-solvable, and this graph is TU");
        let sparsest = complex.solve_sparsest(1, &target).expect("feasible");

        complex
            .closes_to(1, &sparsest, &target, DEFAULT_FUEL)
            .expect("solve_sparsest's own witness independently verifies");

        assert!(
            sparsest.len() < naive.len(),
            "solve_sparsest ({sparsest:?}) must beat the naive particular solution ({naive:?})"
        );
        assert_eq!(sparsest, vec![(3, whole(1))], "the shortcut edge alone closes it");
        assert_eq!(naive, vec![(0, whole(1)), (1, whole(1)), (2, whole(1))]);
    }

    #[test]
    fn solve_sparsest_refuses_when_no_rational_solution_exists_at_all() {
        let op = vec![
            Entry { row: 0, col: 0, coeff: whole(1) },
            Entry { row: 1, col: 1, coeff: whole(1) },
        ];
        let complex = DeclaredComplex {
            cells: vec![4, 2], // cells 2,3 touch no declared 1-cell at all
            ops: vec![op],
        };
        let target = vec![(0u32, whole(-1)), (2u32, whole(1))];
        assert_eq!(
            complex.solve_sparsest(1, &target),
            Err(SolveRefused::NoRationalSolution)
        );
    }

    /// #42: `solve_forward` — the kernel's replacement for BFS. On the
    /// reference dihedral conjecture (dim-1, totally unimodular) it
    /// returns the same integral forward path BFS found, and the
    /// witness independently verifies. This is the "aligned physics":
    /// the producer computes the court's own incidence algebra.
    #[test]
    fn solve_forward_matches_the_forward_path_on_the_dihedral_conjecture() {
        // The dihedral group of order 6 (≅ S3), built inline — assay is
        // a leaf and cannot reach datum::corpus, so the same
        // presentation is reconstructed here from assay::rewrite.
        let compiled = Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![
                (b"aaa".to_vec(), Vec::new()),
                (b"bb".to_vec(), Vec::new()),
                (b"ba".to_vec(), b"aab".to_vec()),
            ],
        }
        .compile(6)
        .expect("confluent, compiles");
        let axiom = compiled.word(b"bab").expect("in universe");
        let theorem = compiled.word(b"aa").expect("in universe");
        let target = compiled.target(axiom, theorem).expect("a real instance");
        let universe = compiled.complex;

        let witness = universe.solve_forward(1, &target).expect("bab = aa is forward-derivable");
        // Every coefficient is a positive whole number — a real forward
        // application count, never a fractional or backward step.
        for (_, coeff) in &witness {
            assert!(*coeff > whole(0), "forward witness coefficients are positive: {coeff:?}");
        }
        universe
            .closes_to(1, &witness, &target, DEFAULT_FUEL)
            .expect("solve_forward's own witness independently verifies");
    }

    /// The direction guard, made concrete: a target that names the
    /// REVERSE of the only licensed edge has an integer solution (sign-
    /// free `solve` finds it, using a −1 coefficient) but NO forward
    /// solution — `solve_forward` correctly refuses rather than
    /// returning a backward-edge "derivation".
    #[test]
    fn solve_forward_refuses_a_backward_target_that_sign_free_solve_accepts() {
        let op = vec![
            Entry { row: 0, col: 0, coeff: whole(-1) },
            Entry { row: 1, col: 0, coeff: whole(1) },
        ];
        let complex = DeclaredComplex {
            cells: vec![2, 1],
            ops: vec![op],
        };
        // The reverse boundary: 0 - 1, i.e. "derive node-0 from node-1"
        // against a single forward edge 0->1.
        let backward = vec![(0u32, whole(1)), (1u32, whole(-1))];
        // sign-free solve accepts it (coefficient -1 on the forward edge)...
        assert!(complex.solve(1, &backward).is_ok(), "sign-free solve uses the edge backward");
        // ...but solve_forward will not run a rewrite backward.
        assert_eq!(
            complex.solve_forward(1, &backward),
            Err(SolveRefused::NoRationalSolution),
            "no forward derivation exists, and solve_forward says so"
        );
    }
}
