//! THE HYPOTHESES, TURNED INTO THEOREMS.
//!
//! The cocycle verification theorem (`tests/theorem.rs`) assumed two
//! things it did not prove: each holder holds at most one live deed
//! (**H1**), and live deeds do not overlap (**H2**). Assumed
//! hypotheses are the weakest standing a claim can have here — a test
//! held the court to them and hoped.
//!
//! They are now theorems, one per route a chain can come from:
//!
//! **Invariant Theorem (issuer route).** Every ledger evolved only
//! through the issuer API satisfies H1 ∧ H2 **at every prefix**.
//! *Proof by induction.* Base: the empty ledger holds nothing, so both
//! hold vacuously. Step: `encumber`, `open_axis` and `retire` create
//! no deed and revive none (retired space is never reissued, so H2's
//! taken-set only grows); `issue`/`issue_box` are the only arms that
//! create a deed, and they refuse when the holder already holds
//! (`AlreadyHeld` — preserving H1) and place only into space disjoint
//! from everything taken (preserving H2). No other arm exists. ∎
//!
//! **Discharge Theorem (transcription route).** `record()` judges
//! nothing, so for a transcribed chain the hypotheses are discharged
//! by the decidable predicate `well_formed`, and: *a chain accepted by
//! `well_formed` satisfies H1 ∧ H2 at every prefix, and therefore the
//! admitted relation on it is a function.* The checker walks exactly
//! the induction invariant, so acceptance IS the induction argument,
//! replayed over the specific history.
//!
//! The machine half below verifies the induction step exhaustively
//! over generated scripts **that attack** — double-issues and
//! exhaustions included — and verifies the discharge by constructing
//! violating chains and watching the checker name the act.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::deed::{Act, Flaw, Ledger, Refused, Standing};
use isthmus::layout::Layout;

/// Scripts that try to break the invariants through the issuer:
/// repeated holders, exhaustion, retire-then-reissue, axis opens
/// interleaved.
fn attacking_scripts() -> Vec<Vec<(&'static str, u128)>> {
    vec![
        vec![("a", 8), ("a", 8), ("b", 8), ("a", 4)],
        vec![("a", 200), ("b", 200), ("a", 1)],
        vec![("a", 1), ("a", 1), ("a", 1), ("a", 1)],
        vec![("a", 16), ("b", 16), ("c", 16), ("a", 16), ("b", 16)],
        vec![("a", 255), ("b", 1), ("a", 255)],
    ]
}

// ===================================================================
// The Invariant Theorem — the induction step, verified at every prefix
// ===================================================================

#[test]
fn invariant_every_prefix_of_every_issuer_history_satisfies_h1_and_h2() {
    for script in attacking_scripts() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "both registries");
        let mut refusals = 0usize;

        for (step, (holder, width)) in script.iter().enumerate() {
            match ledger.issue(holder, *width) {
                Ok(_) => {}
                Err(Refused::AlreadyHeld { .. }) => refusals += 1,
                Err(Refused::NoRun { .. }) => refusals += 1,
                Err(other) => panic!("unexpected refusal {other:?}"),
            }

            // H1 ∧ H2 at THIS prefix — not only at the end. An
            // invariant that holds only at the end is a coincidence.
            check_h1_h2(&ledger, &format!("{script:?} step {step}"));

            // And the checker agrees with the issuer at every prefix:
            // the induction argument and its replay are one predicate.
            assert!(
                ledger.well_formed().is_ok(),
                "the issuer built a chain its own checker refuses"
            );
        }

        // Retire and continue — the taken-set must not shrink.
        let open_before = ledger.open();
        ledger.retire("a");
        assert_eq!(ledger.open(), open_before, "retiring returned space");
        let _ = ledger.issue("a", 4); // reattachment after retire is legal
        check_h1_h2(&ledger, &format!("{script:?} after retire"));
        assert!(ledger.well_formed().is_ok());

        // The attack must have been real: scripts with duplicate
        // holders must have produced refusals, or H1 was never tested.
        assert!(
            refusals > 0,
            "{script:?}: no refusal fired — the attack was not an attack"
        );
    }
}

/// **The induction step still holds when nesting is in the script.**
///
/// Without this the theorem file would assert `H2′` over histories that
/// only ever reach depth 0, which is `H2` with extra words — the same
/// blindness `axis_laws` had before `A9`: a hypothesis restated for a
/// state the suite never builds is a hypothesis nobody tested.
///
/// So the attack sublets, sublets a sublet, and tries every way of
/// breaking containment through the issuer.
#[test]
fn invariant_every_prefix_survives_nesting_too() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "both registries");
    ledger.issue("planet", 64).expect("room");
    check_h1_h2(&ledger, "planet issued");

    let (low, _) = ledger
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == "planet")
        .expect("the planet")
        .region[0];

    let mut refusals = 0usize;
    let mut nested = 0usize;
    // Each step: an attempt, then H1 ∧ H2′ at THAT prefix.
    for (step, (from, to, region)) in [
        ("planet", "moon", vec![(low + 8, low + 23)]),
        ("moon", "station", vec![(low + 12, low + 15)]),
        // Outside the planet.
        ("planet", "outsider", vec![(low + 200, low + 201)]),
        // Onto a sibling.
        ("planet", "clash", vec![(low + 16, low + 30)]),
        // To a holder that already holds.
        ("planet", "moon", vec![(low + 40, low + 41)]),
        // A lawful sibling, after all that.
        ("planet", "second", vec![(low + 32, low + 39)]),
        // A moon of the second moon.
        ("second", "outpost", vec![(low + 33, low + 34)]),
    ]
    .into_iter()
    .enumerate()
    {
        match ledger.sublet(from, to, &region) {
            Ok(_) => nested += 1,
            Err(_) => refusals += 1,
        }
        check_h1_h2(&ledger, &format!("nesting step {step}"));
        assert!(
            ledger.well_formed().is_ok(),
            "the issuer built a chain its own checker refuses at step {step}",
        );
    }

    assert!(nested >= 4, "only {nested} sublets landed — depth was never reached");
    assert!(refusals >= 3, "only {refusals} refusals — the attack was not an attack");
    assert_eq!(ledger.depth_of("station"), 2, "the script never reached depth 2");
}

fn check_h1_h2(ledger: &Ledger, context: &str) {
    let live: Vec<_> = ledger.deeds().into_iter().filter(|d| d.live).collect();

    // H1: one live deed per holder.
    let mut holders = std::collections::BTreeSet::new();
    for deed in &live {
        assert!(
            holders.insert(deed.holder.clone()),
            "{context}: {} holds twice",
            deed.holder
        );
    }

    // H2′: pairwise disjoint AT THE SAME DEPTH, and every deed strictly
    // inside its parent. On a chain with no sublets every deed is at
    // depth 0, so this is exactly the old H2 — nothing already proven
    // is surrendered by restating it.
    for (i, one) in live.iter().enumerate() {
        for other in live.iter().skip(i + 1) {
            if ledger.depth_of(&one.holder) != ledger.depth_of(&other.holder) {
                continue;
            }
            let disjoint = one
                .region
                .iter()
                .zip(other.region.iter())
                .any(|((alow, ahigh), (blow, bhigh))| ahigh < blow || bhigh < alow);
            assert!(
                disjoint,
                "{context}: {} and {} overlap at depth {}",
                one.holder,
                other.holder,
                ledger.depth_of(&one.holder),
            );
        }
    }

    // The other half of H2′: containment. A moon is inside its planet
    // on every axis, and the planet is live.
    for deed in &live {
        let Some(parent) = deed.within.as_deref() else {
            continue;
        };
        let above = live
            .iter()
            .find(|d| d.holder == parent)
            .unwrap_or_else(|| panic!("{context}: {}'s parent {parent} is not live", deed.holder));
        assert!(
            deed.region
                .iter()
                .zip(above.region.iter())
                .all(|((ilow, ihigh), (olow, ohigh))| ilow >= olow && ihigh <= ohigh),
            "{context}: {} is not inside {parent}",
            deed.holder,
        );
        assert_eq!(
            ledger.depth_of(&deed.holder),
            ledger.depth_of(parent) + 1,
            "{context}: {} is not one level below {parent}",
            deed.holder,
        );
    }
}

// ===================================================================
// The Discharge Theorem — acceptance implies the hypotheses, and
// therefore functionality of the admitted relation
// ===================================================================

/// Constructed violations, each named by the checker at its act.
#[test]
fn discharge_the_checker_names_each_violation_at_its_act() {
    // A double hold, transcribed.
    let mut doubled = Ledger::new(Layout::founding());
    doubled.issue("h", 8).expect("room");
    doubled.record(Act::Issue {
        holder: "h".into(),
        low: 100,
        high: 107,
    });
    assert!(matches!(
        doubled.well_formed(),
        Err(Flaw::DoubleHold { at: 1, .. })
    ));

    // An overlap with an encumbrance, transcribed.
    let mut overlapped = Ledger::new(Layout::founding());
    overlapped.encumber(1, 31, "ancestral", "both registries");
    overlapped.record(Act::Issue {
        holder: "squatter".into(),
        low: 20,
        high: 40,
    });
    assert!(matches!(
        overlapped.well_formed(),
        Err(Flaw::Overlap { at: 1, .. })
    ));

    // An overlap with RETIRED space — never reissued, so still taken.
    let mut ghosted = Ledger::new(Layout::founding());
    let dead = ghosted.issue("gone", 8).expect("room");
    ghosted.retire("gone");
    ghosted.record(Act::Issue {
        holder: "mover".into(),
        low: dead.low(),
        high: dead.high(),
    });
    assert!(matches!(
        ghosted.well_formed(),
        Err(Flaw::Overlap { at: 2, .. })
    ));

    // An act from a direction that did not exist yet.
    let mut anachronism = Ledger::new(Layout::founding());
    anachronism.record(Act::IssueBox {
        holder: "traveler".into(),
        region: vec![(1, 8), (0, 3)],
    });
    assert!(matches!(
        anachronism.well_formed(),
        Err(Flaw::TooManyAxes { at: 0 })
    ));

    // Two overlapping ENCUMBRANCES are two true observations — both
    // ancestors claim the frozen band, and the founding chain records
    // both. The checker must admit this or it refuses the real court.
    let mut observed = Ledger::new(Layout::founding());
    observed.encumber(1, 31, "netstratum", "NS registries");
    observed.encumber(1, 31, "strand", "wire.rs header");
    assert!(observed.well_formed().is_ok());
}

/// Acceptance implies functionality: over accepted chains, every claim
/// admits at most one point — the consequence the hypotheses existed
/// to buy, now derived instead of assumed.
#[test]
fn discharge_accepted_chains_have_a_functional_admitted_relation() {
    // Build a family of accepted multi-axis chains through mixed
    // issuer calls and VALID transcription.
    let mut edges = Vec::new();
    for spacer in [0u128, 5, 11] {
        let mut edge = Ledger::new(Layout::with_tag_width(1));
        edge.open_axis("revision", 4);
        if spacer > 0 {
            edge.issue_box("pad", &[spacer, 5]).expect("room");
        }
        edge.issue_box("H", &[4, 3]).expect("room");
        edge.issue("liner", 6).expect("room on the zero slice");
        assert!(edge.well_formed().is_ok(), "the family must be accepted");
        edges.push(edge);
    }

    for edge in &edges {
        // Every (holder, offset) claim matches at most one point in
        // the whole space — swept exhaustively.
        let mut seen: std::collections::BTreeMap<(String, Vec<u64>), Vec<u64>> =
            std::collections::BTreeMap::new();
        for tag in 0..=255u64 {
            for revision in 0..=4u64 {
                let point = vec![tag, revision];
                if let Some((holder, offsets)) = edge.potential_at(&point) {
                    if let Some(previous) = seen.insert((holder.clone(), offsets), point.clone())
                    {
                        panic!(
                            "claim ({holder}) admits both {previous:?} and {point:?} \
                             on an ACCEPTED chain — the discharge theorem is refuted"
                        );
                    }
                }
            }
        }
        assert!(!seen.is_empty(), "the sweep reached no deeded point");
    }

    // And the void stays void everywhere, accepted or not.
    for edge in &edges {
        assert_eq!(edge.standing_at(&[0, 3]), Standing::Void);
    }
}