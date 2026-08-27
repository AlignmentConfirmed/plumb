//! THE NEGOTIATION LAWS — and the measured destruction of the scalar.
//!
//! Ruled: a scalar cannot be in a negotiation and a boolean gate
//! cannot hold it — at light speed both are destroyed. The laws below
//! hold the replacement to its properties, and one test **constructs
//! the destruction**: the old scalar-boolean fold, fed the same deltas
//! in two orders, produces divergent verdict traces — two separated
//! parties would each be right and the negotiation would be gone.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use datum::negotiation::{balance, comparable, Ask, Position};
use isthmus::ratio::Exact;
use num_bigint::BigInt;

fn n(v: i64) -> Exact {
    Exact::from(BigInt::from(v))
}

fn q(numer: i64, denom: i64) -> Exact {
    Exact::new(BigInt::from(numer), BigInt::from(denom))
}

/// Deltas: (pole, amount) offers, the unit that crosses the wire.
fn deltas() -> Vec<(&'static str, Exact)> {
    vec![
        ("convergence", n(3)),
        ("transition", q(7, 2)),
        ("convergence", n(5)), // raises the earlier offer
        ("colour", q(1, 3)),
        ("transition", n(2)), // lower than standing: absorbed, not a retreat
    ]
}

fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.is_empty() {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for at in 0..items.len() {
        let mut rest = items.to_vec();
        let head = rest.remove(at);
        for mut tail in permutations(&rest) {
            tail.insert(0, head.clone());
            out.push(tail);
        }
    }
    out
}

// ===================================================================
// The merge laws: merge is commutative, associative, idempotent
// ===================================================================

#[test]
fn merge_is_associative_commutative_idempotent() {
    let mut positions = Vec::new();
    for a in [0i64, 2, 5] {
        for b in [0i64, 3] {
            let mut p = Position::new();
            if a > 0 {
                p.offer("convergence", n(a));
            }
            if b > 0 {
                p.offer("transition", n(b));
            }
            positions.push(p);
        }
    }

    for a in &positions {
        for b in &positions {
            // Commutative.
            let mut ab = a.clone();
            ab.merge(b);
            let mut ba = b.clone();
            ba.merge(a);
            assert_eq!(ab, ba, "merge is not commutative");

            // Idempotent.
            let mut aa = a.clone();
            aa.merge(a);
            assert_eq!(&aa, a, "merge is not idempotent");

            for c in &positions {
                // Associative.
                let mut left = a.clone();
                left.merge(b);
                left.merge(c);
                let mut bc = b.clone();
                bc.merge(c);
                let mut right = a.clone();
                right.merge(&bc);
                assert_eq!(left, right, "merge is not associative");
            }
        }
    }
}

// ===================================================================
// The light-speed law: every arrival order, one position
// ===================================================================

/// All 120 orders of five deltas — with a duplication injected, since
/// light speed also means retransmission — fold to the identical
/// position and the identical balance.
#[test]
fn every_arrival_order_folds_to_the_same_position() {
    let mut ask = Ask::default();
    ask.demand("convergence", n(4));
    ask.demand("transition", n(3));

    let orders = permutations(&deltas());
    assert_eq!(orders.len(), 120);

    let mut folded = None;
    for order in orders {
        let mut position = Position::new();
        for (pole, amount) in &order {
            position.offer(pole, amount.clone());
        }
        // Retransmit the first delta of this order — a duplicate on
        // the wire must change nothing.
        if let Some((pole, amount)) = order.first() {
            position.offer(pole, amount.clone());
        }

        let b = balance(&position, &ask);
        match &folded {
            None => folded = Some((position, b)),
            Some((p0, b0)) => {
                assert_eq!(&position, p0, "arrival order changed the position");
                assert_eq!(&b, b0, "arrival order changed the balance");
            }
        }
    }
}

// ===================================================================
// THE DESTRUCTION, measured: the scalar-boolean fold diverges
// ===================================================================

/// The old structure — a running scalar and a gate `sum >= price`
/// firing the moment it holds — fed the SAME deltas in two orders.
/// The verdict traces diverge: one order grants at delta two, the
/// other at delta three, and the "amount at grant" differs. Two
/// parties separated by delay each hold a defensible, different
/// account of what was negotiated. That is the destruction, as a
/// measurement rather than a warning.
#[test]
fn the_scalar_boolean_gate_diverges_under_reordering() {
    let price = 6i64;
    let stream_a = [n(3), n(4), n(2)]; // gate fires at index 1, sum 7
    let stream_b = [n(2), n(3), n(4)]; // gate fires at index 2, sum 9

    let trace = |stream: &[Exact]| -> (usize, Exact) {
        let mut sum = Exact::from(BigInt::from(0));
        for (at, delta) in stream.iter().enumerate() {
            sum += delta.clone();
            if sum >= n(price) {
                return (at, sum); // the gate fires on ITS instant
            }
        }
        (usize::MAX, sum)
    };

    let (fired_a, granted_a) = trace(&stream_a);
    let (fired_b, granted_b) = trace(&stream_b);

    // Same multiset of deltas, different instants, different amounts
    // bound at the grant.
    assert_ne!(fired_a, fired_b, "the divergence must be real to matter");
    assert_ne!(granted_a, granted_b);

    // The position fold over the same two streams: identical, both
    // ways, because there is no instant to disagree about.
    let mut ask = Ask::default();
    ask.demand("convergence", n(price));
    let fold = |stream: &[Exact]| {
        let mut p = Position::new();
        for (at, delta) in stream.iter().enumerate() {
            // Deltas as standing offers: each is the party's total
            // offer so far from ITS own account, so reordering cannot
            // manufacture or lose value.
            let so_far: Exact = stream
                .iter()
                .take(at + 1)
                .cloned()
                .fold(Exact::from(BigInt::from(0)), |a, b| a + b);
            let _ = delta;
            p.offer("convergence", so_far);
        }
        balance(&p, &ask)
    };
    assert_eq!(fold(&stream_a).clears(), fold(&stream_b).clears());
}

// ===================================================================
// Incomparability: short here, long there, and no order to force
// ===================================================================

/// A party short on one pole and long on another is **incomparable**
/// with a cleared position — not below it. The board's answer is the
/// counter naming both sides: the material of a trade, standing on the
/// docket, refused by nothing.
#[test]
fn short_here_long_there_is_incomparable_and_counters_name_both_sides() {
    let mut ask = Ask::default();
    ask.demand("convergence", n(10));
    ask.demand("transition", n(2));

    let mut trader = Position::new();
    trader.offer("convergence", n(4)); // short 6
    trader.offer("transition", n(9)); // long 7

    let mut covered = Position::new();
    covered.offer("convergence", n(10));
    covered.offer("transition", n(2));

    let traded = balance(&trader, &ask);
    let cleared = balance(&covered, &ask);

    assert!(!traded.clears());
    assert!(cleared.clears());
    assert_eq!(
        comparable(&traded, &cleared),
        None,
        "short-here-long-there was totally ordered — a gate could then \
         hold it, and the ruling says it must not"
    );

    let counter = traded.counter().expect("a standing counter");
    assert_eq!(counter.short, vec![("convergence".to_string(), n(6))]);
    assert_eq!(counter.long, vec![("transition".to_string(), n(7))]);
}

// ===================================================================
// The docket flow: counter, delta, fixpoint, land
// ===================================================================

/// End to end on the real court: the survey attaches the ask, the
/// short position gets a counter (and the proposal DOES NOT DIE), a
/// later delta merges, the fold clears, and enactment witnesses the
/// fixpoint it never conducted.
#[test]
fn the_docket_holds_the_counter_until_the_position_clears() {
    let court = datum::ledger::authority().expect("no authority");

    let mut application = datum::board::Application {
        applicant: "patient".into(),
        shape: vec![8],
        position: Position::new(),
        witness: "held for #43".into(),
    };
    // The ask is per axis now, so the offer is too: the founding
    // court is a line, and its axis is named `tag`.
    application.position.offer(isthmus::layout::TAG, n(3));

    // Geometry answers regardless of funding: the proposal exists.
    let proposal = datum::board::survey(&court, &application).expect("space exists");
    assert_eq!(proposal.price, datum::extent::Extent::new(vec![8]));

    // The fold does not clear; the answer is a counter, not a death.
    let counter = datum::board::clears(&proposal, &application.position)
        .expect_err("3 against 8 cannot clear");
    assert_eq!(counter.short, vec![(isthmus::layout::TAG.to_string(), n(5))]);
    assert!(matches!(
        datum::board::enact(&court, &proposal, &application.position),
        Err(datum::board::EnactRefused::NotCleared(_))
    ));

    // A later delta arrives — in any order, from any path.
    application.position.offer(isthmus::layout::TAG, n(8));
    datum::board::clears(&proposal, &application.position).expect("the fixpoint");

    let grown = datum::board::enact(&court, &proposal, &application.position)
        .expect("witnessed, validated, landed");
    grown.well_formed().expect("lawful history");
    assert!(grown.deeds().iter().any(|d| d.live && d.holder == "patient"));
}