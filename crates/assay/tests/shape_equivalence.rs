//! UC5's measurement: Shape, re-expressed as a declared complex,
//! reaches the same verdict as the compiled domain — over every shape
//! the API can construct.
//!
//! The corpus is the constructible space: `Shape::edge` refuses
//! self-loops, out-of-range orbs, zero charges, and duplicates at the
//! builder, so the invalid shapes that can exist are the structural
//! ones (no orbs, no edges). The translation mirrors the builder's
//! refusals for bytes that arrive without passing the builder.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

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
