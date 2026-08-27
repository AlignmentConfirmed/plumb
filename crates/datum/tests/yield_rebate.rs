//! O1's measurements: on the same demand-posed universe, the leaner
//! witness captures more of the same bounty; a self-posed universe
//! earns nothing however well it closes; over-budget refuses with the
//! price named; escrow bounds every payout.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{ComplexBroken, DeclaredClaim, DeclaredComplex, Entry};
use assay::whole;
use datum::bounty::{settle_answer, AnswerRefused, Bounty};
use datum::query::{Guarantee, Query};
use datum::reward::RewardBook;

/// The theta universe: two vertices, THREE parallel edges, each with
/// boundary v1 − v0. Its cycle space is two-dimensional, so it holds
/// genuinely leaner and fatter closures — which is what a rebate
/// needs to select between.
fn theta() -> DeclaredComplex {
    let mut op = Vec::new();
    for edge in 0..3u32 {
        op.push(Entry { row: 0, col: edge, coeff: whole(-1) });
        op.push(Entry { row: 1, col: edge, coeff: whole(1) });
    }
    DeclaredComplex {
        cells: vec![2, 3],
        ops: vec![op],
    }
}

fn posted() -> (Query, Bounty) {
    let query = Query {
        poser: "agent-7".into(),
        shape: vec![2, 3],
        domain_tag: 82,
        guarantee: Guarantee::Rederivation,
        statement: theta().encode(), // the POSER fixes the universe
    };
    let bounty = Bounty {
        query_id: query.query_id(),
        max_fuel: 200,
        max_bytes: 400,
        base: 1_000,
        per_saved_fuel: 10,
        per_saved_byte: 3,
    };
    (query, bounty)
}

fn answer(witness: Vec<(u32, assay::Exact)>, transport: u64) -> Vec<u8> {
    DeclaredClaim {
        transport,
        complex: theta(),
        dim: 1,
        witness,
    }
    .encode()
}

#[test]
fn the_leaner_witness_captures_more_of_the_same_bounty() {
    let (query, bounty) = posted();
    let mut book = RewardBook::new();

    // Lean: two edges cancel (e0 − e1). Fat: three edges with a
    // doubled coefficient (e0 + e1 − 2·e2) — also a perfect cycle,
    // also credited, measurably more expensive to check.
    let lean = settle_answer(&bounty, &query, &answer(vec![(0, whole(1)), (1, whole(-1))], 1), &mut book)
        .expect("the lean cycle closes");
    let fat = settle_answer(
        &bounty,
        &query,
        &answer(vec![(0, whole(1)), (1, whole(1)), (2, whole(-2))], 1),
        &mut book,
    )
    .expect("the fat cycle closes too");

    assert!(lean.spent_fuel < fat.spent_fuel, "the meter tells them apart");
    assert!(lean.spent_bytes < fat.spent_bytes);
    assert!(
        lean.payout > fat.payout,
        "same universe, same bounty: elegance is the difference — \
         lean {} vs fat {}",
        lean.payout,
        fat.payout
    );

    // Both are bounded by the escrow, which is what makes the market
    // underwritable.
    assert!(lean.payout <= bounty.escrow_bound());
    assert!(fat.payout <= bounty.escrow_bound());
}

#[test]
fn a_self_posed_universe_earns_no_rebate_however_well_it_closes() {
    let (query, bounty) = posted();
    let mut book = RewardBook::new();

    // A beautiful hexagon — in the solver's OWN universe.
    let own = datum::domains::demo_hexagon_claim(1).encode();
    assert_eq!(
        settle_answer(&bounty, &query, &own, &mut book),
        Err(AnswerRefused::NotThePosersUniverse),
        "a node authoring its own task solves it for free — so it is \
         not paid here"
    );
    assert_eq!(book.act_len(), 0, "and nothing touched the book");
}

#[test]
fn over_budget_refuses_with_the_price_named() {
    let (query, mut bounty) = posted();
    let mut book = RewardBook::new();
    let body = answer(vec![(0, whole(1)), (1, whole(-1))], 1);

    // Fuel: the evaluator's own named refusal.
    bounty.max_fuel = 3;
    match settle_answer(&bounty, &query, &body, &mut book) {
        Err(AnswerRefused::Broken(ComplexBroken::FuelExhausted { budget })) => {
            assert_eq!(budget, 3);
        }
        other => panic!("expected a priced fuel refusal, got {other:?}"),
    }

    // Bytes: the byte budget names itself the same way.
    bounty.max_fuel = 200;
    bounty.max_bytes = 10;
    match settle_answer(&bounty, &query, &body, &mut book) {
        Err(AnswerRefused::Oversized { max_bytes, got }) => {
            assert_eq!(max_bytes, 10);
            assert!(got > 10);
        }
        other => panic!("expected Oversized, got {other:?}"),
    }
}

#[test]
fn the_replay_law_reaches_the_bounty_market() {
    let (query, bounty) = posted();
    let mut book = RewardBook::new();
    let body = answer(vec![(0, whole(1)), (1, whole(-1))], 1);
    settle_answer(&bounty, &query, &body, &mut book).expect("first settles");

    // The same structure under a new transport: same work, no pay.
    let copy = answer(vec![(0, whole(1)), (1, whole(-1))], 99);
    assert!(
        matches!(
            settle_answer(&bounty, &query, &copy, &mut book),
            Err(AnswerRefused::Book(datum::reward::RewardRefused::Replay { .. }))
        ),
        "T2 is untouched: the rebate never pays twice for one object"
    );
}
