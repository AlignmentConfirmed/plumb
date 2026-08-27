//! UC4 + UC6 measurements: a court learns a discipline from the chain
//! alone; a definition lapses with its grant; the wrong universe
//! refuses; fuel is a priced budget and the refusal names it.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use assay::complex::{ComplexBroken, DeclaredClaim, DeclaredComplex, Entry, DEFAULT_FUEL};
use assay::whole;
use datum::domains::{self, DomainRefused};
use datum::extent::Extent;
use isthmus::deed::{Act, Ledger};
use isthmus::layout::Layout;

/// The hexagon universe: 6 vertices, 6 edges in a cycle.
fn hexagon_universe() -> DeclaredComplex {
    let n = 6u32;
    let mut op = Vec::new();
    for i in 0..n {
        let (source, target) = (i, (i + 1) % n);
        let mut pair = vec![
            Entry { row: target, col: i, coeff: whole(1) },
            Entry { row: source, col: i, coeff: whole(-1) },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    DeclaredComplex {
        cells: vec![n, n],
        ops: vec![op],
    }
}

fn cycle_claim(universe: &DeclaredComplex, transport: u64) -> DeclaredClaim {
    DeclaredClaim {
        transport,
        complex: universe.clone(),
        dim: 1,
        witness: (0..6).map(|i| (i, whole(1))).collect(),
    }
}

/// An edge where "geometer" holds a range and registered the hexagon
/// universe on its low tag.
fn registered_edge() -> (Ledger, u64) {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue("geometer", 16).expect("room");
    let tag = ledger
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == "geometer")
        .expect("issued")
        .low();
    ledger.record(Act::Declare {
        holder: "geometer".into(),
        tag,
        definition: hexagon_universe().encode(),
    });
    (ledger, tag)
}

#[test]
fn a_court_learns_a_discipline_from_the_chain_alone() {
    let (ledger, tag) = registered_edge();
    let body = cycle_claim(&hexagon_universe(), 1).encode();
    let spent = domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL)
        .expect("the registered universe judges the claim — no rebuild");
    assert!(spent > 0, "and the judging had a measured price");
}

#[test]
fn an_unregistered_tag_cannot_be_judged() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue("geometer", 16).expect("room");
    let body = cycle_claim(&hexagon_universe(), 1).encode();
    assert_eq!(
        domains::verify_registered(&ledger, 200, &body, DEFAULT_FUEL),
        Err(DomainRefused::Unregistered)
    );
}

#[test]
fn the_wrong_universe_refuses() {
    let (ledger, tag) = registered_edge();
    // A triangle universe — closes fine, but it is not what the chain
    // registered for this tag.
    let n = 3u32;
    let mut op = Vec::new();
    for i in 0..n {
        let (source, target) = (i, (i + 1) % n);
        let mut pair = vec![
            Entry { row: target, col: i, coeff: whole(1) },
            Entry { row: source, col: i, coeff: whole(-1) },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    let triangle = DeclaredComplex { cells: vec![n, n], ops: vec![op] };
    let body = DeclaredClaim {
        transport: 1,
        complex: triangle,
        dim: 1,
        witness: (0..3).map(|i| (i, whole(1))).collect(),
    }
    .encode();
    assert_eq!(
        domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL),
        Err(DomainRefused::WrongUniverse),
        "closing in your own private geometry proves nothing here"
    );
}

#[test]
fn a_definition_lapses_with_its_grant() {
    let (mut ledger, tag) = registered_edge();
    ledger.record(Act::Retire {
        holder: "geometer".into(),
    });
    let body = cycle_claim(&hexagon_universe(), 1).encode();
    assert_eq!(
        domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL),
        Err(DomainRefused::Unregistered),
        "a vocabulary does not outlive its grant"
    );
}

#[test]
fn a_later_declaration_supersedes_and_registration_is_not_trust() {
    let (mut ledger, tag) = registered_edge();
    // The holder republishes garbage over its own tag. Registration
    // succeeds — the chain records speech — and judgment refuses.
    ledger.record(Act::Declare {
        holder: "geometer".into(),
        tag,
        definition: vec![9, 9, 9],
    });
    let body = cycle_claim(&hexagon_universe(), 1).encode();
    assert!(matches!(
        domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL),
        Err(DomainRefused::BadDefinition(_)),

    ));
}

// ── UC6: fuel is a priced budget ────────────────────────────────────

#[test]
fn an_over_budget_evaluation_refuses_with_the_price_named() {
    let (ledger, tag) = registered_edge();
    let body = cycle_claim(&hexagon_universe(), 1).encode();

    // The space priced 10 units of fuel on its fuel axis.
    let price = Extent::new(vec![6, 10]);
    let budget = domains::fuel_budget(&price, 1);
    assert_eq!(budget, 10);

    match domains::verify_registered(&ledger, tag, &body, budget) {
        Err(DomainRefused::Broken(ComplexBroken::FuelExhausted { budget })) => {
            assert_eq!(budget, 10, "the refusal names the priced budget");
        }
        other => panic!("expected a priced fuel refusal, got {other:?}"),
    }

    // An unpriced space grants nothing at all.
    assert_eq!(domains::fuel_budget(&Extent::new(vec![6]), 1), 0);
}
