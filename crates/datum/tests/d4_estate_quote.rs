//! Diamond D-L4 — multi-axis estate quote (no product fold).
//!
//! ```bash
//! cargo test --test d4_estate_quote
//! ```

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use datum::board::quote;
use datum::extent::Extent;
use isthmus::deed::Ledger;
use isthmus::layout::Layout;
use std::cmp::Ordering;

fn founding_wide() -> Ledger {
    let mut court = Ledger::new(Layout::founding()).under("quote-court");
    court.issue("founder", 64).expect("issue");
    court
}

#[test]
fn requested_shapes_are_incomparable_not_product_equal() {
    // The anti-fold pin: product both 16, multi-axis need is incomparable.
    let need_28 = Extent::new(vec![2, 8]);
    let need_44 = Extent::new(vec![4, 4]);
    assert_eq!(need_28.compare(&need_44), None);
    assert!(!need_28.fits_in(&need_44));
    assert!(!need_44.fits_in(&need_28));
}

#[test]
fn quote_exposes_multi_axis_need_and_price() {
    let court = founding_wide();
    let q28 = quote(&court, "a", vec![2, 8]).expect("quote 2x8");
    let q44 = quote(&court, "b", vec![4, 4]).expect("quote 4x4");

    assert_eq!(q28.need.components(), &[2, 8]);
    assert_eq!(q44.need.components(), &[4, 4]);
    assert_eq!(q28.need.compare(&q44.need), None);

    // Price is multi-axial (not a single u128 field).
    assert!(
        q28.price.axes() >= 1 && !q28.price.is_empty(),
        "price must be an Extent, not a fold"
    );
    assert!(
        q44.price.axes() >= 1 && !q44.price.is_empty(),
        "price must be an Extent, not a fold"
    );
    // Distinct needs produce quotes that still carry distinct need vectors.
    assert_ne!(q28.need.components(), q44.need.components());
}

#[test]
fn quote_same_shape_need_is_equal() {
    let court = founding_wide();
    let a = quote(&court, "a", vec![3, 3]).expect("a");
    let b = quote(&court, "b", vec![3, 3]).expect("b");
    assert_eq!(a.need.compare(&b.need), Some(Ordering::Equal));
    assert!(a.need.fits_in(&b.need));
}

#[test]
fn quote_shapeless_refuses() {
    let court = founding_wide();
    assert!(quote(&court, "x", vec![]).is_err());
    assert!(quote(&court, "x", vec![0, 1]).is_err());
}

#[test]
fn quote_returns_estate_and_ask() {
    let court = founding_wide();
    let q = quote(&court, "orbiter", vec![2, 2]).expect("orbit");
    // Estate is classified (Run / Orbit / Parcel / Galaxy — any is fine).
    let _ = format!("{:?}", q.estate);
    assert!(!q.price.is_empty());
    // Ask names poles for nonzero price components.
    let poles: Vec<_> = q.ask.poles().cloned().collect();
    assert!(
        !poles.is_empty(),
        "ask should name poles for multi-axis demand"
    );
}
