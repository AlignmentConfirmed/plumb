//! LAWS for the convergence engine.
//!
//! Rules 3, 4 and 5 of the ruling, as properties over generated
//! boundaries rather than one test per method.
//!
//! The law that matters most is `v3`: **the total is not the test, and
//! there is no total.** A boundary whose axes are exact negatives of
//! each other is open in two directions at once, and an engine that
//! summed them would mint a witness for it.
//!
//! The crate briefly *named* that state by computing the sum — an arm
//! called `Cancelling`, split from `Divergent` by a fold. Struck. The
//! residue `[+1, −1]` says it without adding anything, and adding flux
//! across axes that are not commensurable is the flattening the crate
//! exists to refuse. You fold a sheet of paper, not a sphere.
//!
//! Tests may panic. A test that cannot reach its subject must say so
//! loudly rather than pass quietly.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

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
