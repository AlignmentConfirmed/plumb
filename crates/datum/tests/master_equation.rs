//! Master equation of multi-chain linkage estates — ENFORCED clauses.
//!
//! Canonical statement: `decide/linkage-estates.md`
//!
//! ```text
//! (S)  F ⊔ G = max per chain; concurrent when incomparable
//! (E)  Space = ∏ axes; Open adds dimension; no product-volume order
//! (C)  credit ≽ price  ⇔  ∀i credit_i ≥ price_i (same arity)
//! (W)  work_id = structure; credit once
//! (M)  Anchor grants ∅ ground; dim from Open not Anchor
//! ```
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use assay::whole;
use datum::extent::Extent;
use datum::onramp::{shape_body, shape_from_edges};
use datum::reward::{closed_box_claim, RewardBook};
use isthmus::deed::Ledger;
use isthmus::sphere::Frontier;
use isthmus::layout::Layout;
use std::cmp::Ordering;

// ─── (S) Sphere linkage ─────────────────────────────────────────────────────

#[test]
fn l_frontier_join_is_per_chain_max() {
    let mut f = Frontier::new();
    f.observe("a", 3);
    f.observe("b", 10);
    let mut g = Frontier::new();
    g.observe("a", 7);
    g.observe("b", 4);
    let j = f.join(&g);
    assert_eq!(j.height_of("a"), 7);
    assert_eq!(j.height_of("b"), 10);
}

#[test]
fn l_concurrent_frontiers_are_incomparable() {
    let mut f = Frontier::new();
    f.observe("a", 5);
    f.observe("b", 1);
    let mut g = Frontier::new();
    g.observe("a", 2);
    g.observe("b", 9);
    assert!(f.concurrent_with(&g));
    assert_eq!(f.compare(&g), None);
}

// ─── (E) Estate axes ─────────────────────────────────────────────────

#[test]
fn e_open_adds_dimension_without_granting_ground() {
    let mut court = Ledger::new(Layout::founding()).under("e");
    assert_eq!(court.axes().len(), 1, "founding line");
    court.open_axis("revision", 7);
    assert_eq!(court.axes().len(), 2, "Open multiplies directions");
    let live = court.deeds().into_iter().filter(|d| d.live).count();
    assert_eq!(live, 0, "Open grants nobody a live deed");
    court.well_formed().expect("open is lawful");
}

#[test]
fn e_incomparable_boxes_are_not_ordered_by_product() {
    // [2,8] and [4,4] both "product 16" if volume were used — forbidden.
    let a = Extent::new(vec![2, 8]);
    let b = Extent::new(vec![4, 4]);
    assert!(a.compare(&b).is_none());
    assert!(!a.fits_in(&b));
    assert!(!b.fits_in(&a));
}

// ─── (C) Capacity ────────────────────────────────────────────────────

#[test]
fn c_credit_covers_price_componentwise_same_arity() {
    let mut book = RewardBook::new();
    book.credit_claim(&closed_box_claim(0, 1).encode())
        .expect("credit");
    assert!(book.covers(&Extent::new(vec![1, 1])));
    assert!(!book.covers(&Extent::new(vec![2, 1])));
    // Arity mismatch: not covers
    assert!(!book.covers(&Extent::new(vec![1])));
}

// ─── (W) Work identity ───────────────────────────────────────────────

#[test]
fn w_work_id_is_structure_credit_once() {
    let shape = shape_from_edges(2, [(0, 1, whole(1))]).expect("shape");
    let a = shape_body(1, shape.clone()).expect("body");
    let b = shape_body(99, shape).expect("body");
    let mut book = RewardBook::new();
    book.credit_claim(&a).expect("first");
    assert!(matches!(
        book.credit_claim(&b),
        Err(datum::reward::RewardRefused::Replay { .. })
    ));
}

// ─── (M) Anchor grants no ground; dim independent of vertical ───────

#[test]
fn m_anchor_extends_frontier_not_estate_axes() {
    let mut court = Ledger::new(Layout::founding()).under("north");
    let axes_before = court.axes().len();
    court.anchor("south", 15, &[1, 2, 3, 4, 5, 6, 7, 8], "session");
    assert_eq!(court.axes().len(), axes_before, "Anchor is not Open");
    assert_eq!(court.frontier().height_of("south"), 15);
    assert_eq!(court.deeds().into_iter().filter(|d| d.live).count(), 0);
    // Still well-formed after vertical only.
    court.well_formed().expect("anchor-only chain is lawful");
}

#[test]
fn m_uplink_loop_is_knowledge_not_space_merge() {
    // A sees B@3; B sees A@2 — cycle of frontiers, independent spaces.
    let mut a = Ledger::new(Layout::founding()).under("A");
    a.anchor("B", 3, &[0u8; 8], "up");
    let mut b = Ledger::new(Layout::founding()).under("B");
    b.anchor("A", 2, &[0u8; 8], "down");

    assert_eq!(a.frontier().height_of("B"), 3);
    assert_eq!(b.frontier().height_of("A"), 2);
    // Neither absorbed the other's axes.
    assert_eq!(a.axes().len(), 1);
    assert_eq!(b.axes().len(), 1);
    // Join of frontiers is well-defined and larger in both components.
    let j = a.frontier().join(&b.frontier());
    assert_eq!(j.height_of("A"), a.frontier().height_of("A").max(2));
    assert_eq!(j.height_of("B"), 3);
    assert_eq!(a.frontier().compare(&j), Some(Ordering::Less));
}

#[test]
fn m_high_d_estate_path_is_not_capped_at_two_axes() {
    // Dimensional freedom: Open can add axes beyond 2 (full 11-D is board).
    let mut court = Ledger::new(Layout::founding()).under("mesh");
    for name in ["d1", "d2", "d3", "d4"] {
        court.open_axis(name, 3);
    }
    assert_eq!(
        court.axes().len(),
        5,
        "1 founding + 4 Open = 5-D space, not 2-D"
    );
    court.well_formed().expect("5-D open is lawful");
}
