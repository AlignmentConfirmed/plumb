//! H4 — multi-axis credit vs deed-priced space.
//! H5 — settlement appends acts as a block.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use datum::block::BlockRefused;
use datum::extent::Extent;
use datum::negotiation::Position;
use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};
use datum::settle::{self, SettleRefused};
use datum::board::Application;
use isthmus::ratio::Exact;
use num_bigint::BigInt;

fn apply(applicant: &str, shape: Vec<u128>, work: u128) -> Application {
    let mut position = Position::new();
    let amount = Exact::from(BigInt::from(work));
    position.offer(isthmus::layout::TAG, amount.clone());
    for at in 1..=shape.len() {
        position.offer(&format!("{applicant}-d{at}"), amount.clone());
        position.offer(&format!("nova-d{at}"), amount.clone());
    }
    Application {
        applicant: applicant.into(),
        shape,
        position,
        witness: format!("{applicant} work_id credits"),
    }
}

/// One-axis closed boundary bodies (flux 1..=n) → credit [n].
fn boundary_stack(n: u128) -> Vec<Vec<u8>> {
    let mut bodies = Vec::new();
    for flux in 1..=n {
        let mut b = assay::Boundary::new(1);
        let f = assay::whole(flux as i64);
        assert!(b.face(assay::Facet::new(0, assay::Orientation::Low, f.clone())));
        assert!(b.face(assay::Facet::new(0, assay::Orientation::High, f)));
        bodies.push(assay::Claim::new(0, b).encode());
    }
    bodies
}

// ── H4 ─────────────────────────────────────────────────────────────

#[test]
fn deed_price_is_multi_axis_extent_not_a_product() {
    // H4 core: space price is Extent; product would call [2,8] == [4,4].
    let a = Extent::new(vec![2, 8]);
    let b = Extent::new(vec![4, 4]);
    assert_eq!(
        a.components().iter().product::<u128>(),
        b.components().iter().product::<u128>()
    );
    assert!(a.compare(&b).is_none());
    assert!(!a.fits_in(&b) && !b.fits_in(&a));

    // Survey of a multi-axis request returns a multi-component price
    // (galaxy or orbit), never a single folded scalar.
    let court = datum::ledger::authority().expect("authority");
    let app = apply("wide", vec![2, 8], 256);
    let proposal = datum::board::survey(&court, &app).expect("survey");
    assert!(
        proposal.price.axes() >= 2,
        "multi-axis grant must price per axis, got {:?}",
        proposal.price.components()
    );
}

#[test]
fn multi_axis_credit_must_cover_every_axis_of_price() {
    let mut book = RewardBook::new();
    // 2-axis credit [1,1]
    book.credit_claim(&closed_box_claim(1, 3).encode())
        .expect("credit");
    assert!(book.covers(&Extent::new(vec![1, 1])));
    // Cannot cover a taller first axis
    assert!(!book.covers(&Extent::new(vec![2, 1])));
    // Arity mismatch is not cover
    assert!(!book.covers(&Extent::new(vec![1])));
}

#[test]
fn underfunded_book_refuses_enact() {
    let court = datum::ledger::authority().expect("authority");
    let application = apply("thin", vec![8], 64);
    let proposal = datum::board::survey(&court, &application).expect("survey");
    let book = RewardBook::new();
    match settle::enact_if_funded(&book, &court, &proposal, &application.position) {
        Err(SettleRefused::Work(RewardRefused::Underfunded { .. })) => {}
        other => panic!("expected underfunded work, got {other:?}"),
    }
}

#[test]
fn funded_boundary_work_enacts_join() {
    let court = datum::ledger::authority().expect("authority");
    let application = apply("coral", vec![8], 64);
    let mut book = RewardBook::new();
    for body in boundary_stack(8) {
        book.credit_claim(&body).expect("credit");
    }
    assert_eq!(book.total().components(), &[8]);
    let proposal = datum::board::survey(&court, &application).expect("survey");
    assert_eq!(proposal.price, Extent::new(vec![8]));
    let grown = settle::enact_if_funded(&book, &court, &proposal, &application.position)
        .expect("enact funded");
    assert!(grown
        .deeds()
        .iter()
        .any(|d| d.live && d.holder == "coral"));
}

#[test]
fn shape_credit_is_standing_not_upsilon_token() {
    let mut book = RewardBook::new();
    let credit = book
        .credit_claim(&triangle_claim(0).encode())
        .expect("shape credit");
    assert!(credit.witness.is_none());
    assert_eq!(credit.axes.components(), &[1, 1, 1]);
    // standing stacks as multi-axis room
    assert!(book.covers(&Extent::new(vec![1, 1, 1])));
}

// ── H5 ─────────────────────────────────────────────────────────────

#[test]
fn land_appends_settlement_acts_as_a_block() {
    let court = datum::ledger::authority().expect("authority");
    let prior = court.acts().len();
    let application = apply("reef", vec![4], 64);
    let mut book = RewardBook::new();
    let bodies = boundary_stack(4);
    let refs: Vec<&[u8]> = bodies.iter().map(|b| b.as_slice()).collect();
    let (proposal, block, credits) =
        settle::land(&mut book, &court, &application, &refs).expect("land");
    assert_eq!(credits.len(), 4);
    assert!(!proposal.acts.is_empty());
    assert_eq!(block.batch_len(), proposal.acts.len());
    assert_eq!(block.prior_height, prior);
    assert_eq!(block.height, prior + proposal.acts.len());
    assert_eq!(block.landed_slice().len(), proposal.acts.len());
    // Court holds the estate
    assert!(block
        .court
        .deeds()
        .iter()
        .any(|d| d.live && d.holder == "reef"));
    // Second land of same applicant shape may still work if free space remains
    block.court.well_formed().expect("settled court well-formed");
}

#[test]
fn empty_block_is_not_a_settlement() {
    let court = datum::ledger::authority().expect("authority");
    assert!(matches!(
        datum::block::produce(&court, vec![]),
        Err(BlockRefused::Empty)
    ));
}

#[test]
fn join_with_work_stacks_and_enacts() {
    let court = datum::ledger::authority().expect("authority");
    let application = apply("lagoon", vec![4], 64);
    let mut book = RewardBook::new();
    let bodies = boundary_stack(4);
    let refs: Vec<&[u8]> = bodies.iter().map(|b| b.as_slice()).collect();
    let (_proposal, grown, credits) =
        settle::join_with_work(&mut book, &court, &application, &refs).expect("join");
    assert_eq!(credits.len(), 4);
    assert!(grown
        .deeds()
        .iter()
        .any(|d| d.live && d.holder == "lagoon"));
}
