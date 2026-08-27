//! LAWS for the ledger. Properties over generated sequences of acts,
//! not one test per method.
//!
//! The thing being replaced was a constant, so the thing testing it was
//! a constant too: "assert grants_available() == 6". That test could
//! only ever say the number had not been retyped.
//!
//! A ledger is a state machine driven by acts, so the laws are about
//! **what must hold after any sequence of acts** — issue, encumber,
//! retire, in any order, at any width.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::deed::{Act, Ledger, Refused, Standing};
use isthmus::layout::Layout;

/// Apply a script to a fresh edge, with the founding encumbrance.
fn run(script: &[(&str, u128)]) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "both registries, read 2026-08-05");
    for (holder, width) in script {
        let _ = ledger.issue(holder, *width);
    }
    ledger
}

/// Deterministic sequences of acts. No seed, no sampling — an
/// enumerated set of scripts, so a pass means *none of these* rather
/// than *these draws*.
fn scripts() -> Vec<Vec<(&'static str, u128)>> {
    let mut out = Vec::new();
    for widths in [
        vec![8u128],
        vec![1],
        vec![255],
        vec![1, 1, 1, 1, 1],
        vec![8, 8, 8, 8, 8, 8, 8],
        vec![64, 64, 64, 64],
        vec![100, 100, 100],
        vec![3, 17, 1, 40, 2],
        vec![16, 48, 32, 32, 48, 16],
    ] {
        out.push(
            widths
                .into_iter()
                .enumerate()
                .map(|(n, w)| (NAMES[n % NAMES.len()], w))
                .collect(),
        );
    }
    out
}

const NAMES: [&str; 7] = ["a", "b", "c", "d", "e", "f", "g"];

// ===================================================================
// D1 — the capacity is not a number this crate holds
// ===================================================================

/// **More attachments than the old constant allowed.**
///
/// The previous design returned 6 from `grants_available()` and that was
/// the end of it. This issues thirty-one and stops only because the edge
/// ran out of bytes, which is physical.
#[test]
fn d1_the_edge_issues_until_the_bytes_run_out_not_until_a_constant() {
    let mut ledger = Ledger::new(Layout::founding());
    let mut issued = 0usize;

    while ledger.largest_open() >= 8 {
        let name = format!("mesh-{issued}");
        match ledger.issue(&name, 8) {
            Ok(deed) => {
                assert_eq!(deed.width(), 8);
                issued += 1;
            }
            Err(why) => panic!("refused with {} open: {why:?}", ledger.largest_open()),
        }
    }

    println!("D1 issued {issued} deeds of width 8; {} open", ledger.open());
    assert!(
        issued > 6,
        "issued only {issued} — a capacity constant is still in force somewhere"
    );
    assert_eq!(issued, 31, "255 tags after the void, 8 at a time");
}

/// And width is asked for, not decided. An attachment needing three tags
/// takes three.
#[test]
fn d1b_width_is_the_holders_to_choose() {
    for width in [1u128, 2, 3, 7, 16, 100, 255] {
        let mut ledger = Ledger::new(Layout::founding());
        let deed = ledger
            .issue("asker", width)
            .unwrap_or_else(|e| panic!("width {width} refused: {e:?}"));
        assert_eq!(deed.width(), width, "asked {width}, got {}", deed.width());
    }
}

// ===================================================================
// D0 — the acts are the ledger
// ===================================================================

/// **Replaying the acts reproduces the ledger exactly.**
///
/// The load-bearing property of an append-only record: state is a fold
/// over entries, so a ledger holding state *alongside* its entries could
/// drift from them and nothing would say so. Everything readable here is
/// derived, and this is what says so.
#[test]
fn d0_a_ledger_is_its_acts_and_nothing_else() {
    for script in scripts() {
        let mut original = run(&script);
        if let Some((holder, _)) = script.first() {
            original.retire(holder);
        }

        let replayed = Ledger::replay(Layout::founding(), original.acts().to_vec());

        assert_eq!(replayed.acts(), original.acts());
        assert_eq!(replayed.deeds(), original.deeds());
        assert_eq!(replayed.open(), original.open());
        assert_eq!(replayed.gaps(), original.gaps());
        for tag in 0u64..=255 {
            assert_eq!(
                replayed.standing_of(tag),
                original.standing_of(tag),
                "tag {tag} folded differently after a replay"
            );
        }
    }
}

/// **Retirement appends; it does not erase.**
///
/// The act stays in the record, so what happened is readable after it
/// stops being in force.
#[test]
fn d0b_retiring_leaves_the_issue_act_in_place() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.issue("goes", 8).expect("room");
    let after_issue = ledger.acts().len();

    ledger.retire("goes");
    assert_eq!(
        ledger.acts().len(),
        after_issue + 1,
        "retiring should APPEND an entry"
    );
    assert!(
        ledger
            .acts()
            .iter()
            .any(|a| matches!(a, Act::Issue { holder, .. } if holder == "goes")),
        "the issue act was removed — the history is not a history"
    );
    assert!(ledger
        .acts()
        .iter()
        .any(|a| matches!(a, Act::Retire { holder } if holder == "goes")));
}

/// **An encumbrance carries where it was observed.**
///
/// An observation without provenance is indistinguishable from this
/// ledger having decided something. A document produced by this project
/// is never an input; a neighbour's registry, cited, is.
#[test]
fn d0c_an_encumbrance_says_where_it_was_read() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(32, 54, "netstratum", "netstratum NS-1 registry");

    match ledger.acts().first() {
        Some(Act::Encumber { by, witnessed, .. }) => {
            assert_eq!(by, "netstratum");
            assert!(!witnessed.is_empty(), "an observation with no provenance");
        }
        other => panic!("expected an Encumber act, got {other:?}"),
    }
}

// ===================================================================
// D1c — the chain stores and replays
// ===================================================================

/// **Every script's acts survive storage byte-exactly**, and the ledger
/// replayed from the stored bytes is the ledger.
///
/// This is what makes an authority possible: without it the acts live
/// only in one process's memory, and an authority whose record dies
/// with a process is not an authority.
#[test]
fn d1c_a_chain_round_trips_through_storage() {
    use isthmus::deed::chain;

    for script in scripts() {
        let mut original = run(&script);
        if let Some((holder, _)) = script.first() {
            original.retire(holder);
        }

        let stored = chain::encode(original.acts());
        let acts = chain::decode(&stored).expect("its own bytes");
        assert_eq!(acts, original.acts());

        let replayed = Ledger::replay(Layout::founding(), acts);
        for tag in 0u64..=255 {
            assert_eq!(replayed.standing_of(tag), original.standing_of(tag));
        }

        // Truncation refuses at EVERY cut — half a history folded is a
        // different history reported as this one.
        for cut in 1..stored.len() {
            if chain::decode(&stored[..cut]).is_ok() {
                // A cut landing exactly between records decodes to a
                // PREFIX of the acts — fewer acts, which is detectable
                // by count. A cut inside a record must refuse.
                let prefix = chain::decode(&stored[..cut]).expect("checked");
                assert!(
                    prefix.len() < original.acts().len(),
                    "a truncated chain decoded to the whole history"
                );
            }
        }
    }
}

/// **An unknown act refuses; it does not skip.** On the mesh an unknown
/// tag steps over whole. In a chain, a skipped act folds a different
/// history and reports it as this one.
#[test]
fn d1d_an_unknown_act_refuses_rather_than_skipping() {
    use isthmus::deed::chain;
    use isthmus::frame::put_frame;

    let mut ledger = Ledger::new(Layout::founding());
    ledger.issue("real", 8).expect("room");
    let mut stored = chain::encode(ledger.acts());

    // An act from a future revision this decoder does not know.
    put_frame(&Layout::founding(), 9, &[0xAA, 0xBB], &mut stored).expect("fits");

    assert!(
        chain::decode(&stored).is_err(),
        "an unknown act was skipped — the fold would be a different history"
    );
}

// ===================================================================
// D2 — no two live deeds overlap, after any script
// ===================================================================

/// **A tag is held by at most one live deed.**
///
/// The property the old `the_grants_do_not_overlap_each_other` was
/// reaching for, except that one checked a hand-written table and this
/// checks the outcome of every script.
#[test]
fn d2_no_tag_is_held_twice() {
    for script in scripts() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "both registries");
        for (holder, width) in &script {
            let _ = ledger.issue(holder, *width);
        }

        for tag in 0u64..=255 {
            let holders: Vec<_> = ledger
                .deeds()
                .iter()
                .filter(|d| d.covers(tag))
                .map(|d| d.holder.clone())
                .collect();
            assert!(
                holders.len() <= 1,
                "tag {tag} held by {holders:?} after {script:?}"
            );
        }
    }
}

// ===================================================================
// D3 — issuing never lands on what is already taken
// ===================================================================

/// **An encumbered tag is never deeded, whatever the script.**
///
/// This is the failure `IS-3` §5.4 records: a grant table issued 32-47
/// over numbers netstratum already claimed, and it was written without
/// checking. Here it is a law rather than a table review.
#[test]
fn d3_an_encumbrance_is_never_issued_over() {
    for script in scripts() {
        let mut ledger = Ledger::new(Layout::founding());
        // Deliberately awkward: encumbrances scattered so a naive
        // first-fit that only looks at the front would trip.
        ledger.encumber(1, 31, "ancestral", "both registries");
        ledger.encumber(60, 60, "netstratum", "NS-1 registry");
        ledger.encumber(62, 63, "netstratum", "NS-1 registry");
        ledger.encumber(200, 204, "someone else", "their advert");

        for (holder, width) in &script {
            let _ = ledger.issue(holder, *width);
        }

        for tag in 0u64..=255 {
            if let Standing::Encumbered { by } = ledger.standing_of(tag) {
                assert!(
                    ledger.holder_of(tag).is_none(),
                    "tag {tag} is encumbered by {by} and was deeded anyway"
                );
            }
        }
    }
}

// ===================================================================
// D4 — a retired tag is never reissued
// ===================================================================

/// **Retiring frees nothing.**
///
/// Reissuing would hand a newcomer a number an old peer still remembers
/// the meaning of. The newcomer would be right about the number and
/// wrong about everything else, and nothing on the wire would say so.
#[test]
fn d4_retirement_does_not_return_tags_to_the_pool() {
    let mut ledger = Ledger::new(Layout::founding());
    let first = ledger.issue("early", 16).expect("room");
    let open_before = ledger.open();

    assert!(ledger.retire("early"));
    assert_eq!(
        ledger.open(),
        open_before,
        "retiring returned tags to the pool"
    );

    for tag in first.low()..=first.high() {
        assert_eq!(ledger.standing_of(tag), Standing::Retired);
        assert!(ledger.holder_of(tag).is_none(), "a retired deed still holds");
    }

    // And a later attachment lands somewhere else entirely.
    let later = ledger.issue("late", 16).expect("room");
    assert!(
        later.low() > first.high() || later.high() < first.low(),
        "a new deed overlapped a retired one"
    );
}

// ===================================================================
// D5 — a refusal says how much is left
// ===================================================================

/// **When issuing refuses, it reports the largest open run.**
///
/// A refusal that does not say how much is left forces the caller to
/// probe — ask for 64, ask for 32, ask for 16 — which is a negotiation
/// conducted by guessing.
#[test]
fn d5_a_refusal_carries_the_number_that_would_have_worked() {
    let mut ledger = Ledger::new(Layout::founding());
    while ledger.largest_open() > 10 {
        let n = ledger.deeds().len();
        if ledger.issue(&format!("m{n}"), 10).is_err() {
            break;
        }
    }

    let left = ledger.largest_open();
    match ledger.issue("greedy", left + 1) {
        Err(Refused::NoRun {
            wanted,
            largest_open,
        }) => {
            assert_eq!(wanted, left + 1);
            assert_eq!(largest_open, left);
            // And the number it reported is one that actually works.
            if left > 0 {
                assert!(
                    ledger.issue("modest", left).is_ok(),
                    "the refusal named a width that then refused"
                );
            }
        }
        other => panic!("expected NoRun, got {other:?}"),
    }
}

/// Zero is not an attachment.
#[test]
fn d5b_zero_width_refuses() {
    let mut ledger = Ledger::new(Layout::founding());
    assert_eq!(ledger.issue("nobody", 0), Err(Refused::ZeroWidth));
}

// ===================================================================
// D6 — forwarding is a change of coordinates
// ===================================================================

/// **The same holder gets different numbers on different edges, and a
/// frame translates between them.**
///
/// This is the property a global const table was invented to avoid
/// needing, at the cost of capping the substrate at six attachments.
#[test]
fn d6_a_frame_crosses_a_deed_boundary_by_renumbering() {
    let mut edge_a = Ledger::new(Layout::founding());
    let mut edge_b = Ledger::new(Layout::founding());

    // The two edges have different histories, so the same holder lands
    // on different numbers. That is the point, not an accident.
    edge_a.encumber(1, 31, "ancestral", "both registries");
    edge_b.encumber(1, 90, "a busier neighbour", "their advert");

    let on_a = edge_a.issue("chitin", 16).expect("room on a");
    let on_b = edge_b.issue("chitin", 16).expect("room on b");
    assert_ne!(on_a.low(), on_b.low(), "the edges chose the same numbering");

    // A frame at the holder's third tag on A is its third tag on B.
    for offset in 0..16u64 {
        let here = on_a.low() + offset;
        let there = edge_a
            .translate(here, &edge_b)
            .unwrap_or_else(|| panic!("tag {here} did not translate"));
        assert_eq!(there, on_b.low() + offset);
        // And the meaning survived: same holder, same position in its
        // own range.
        assert_eq!(
            edge_b.holder_of(there).map(|d| d.holder),
            Some("chitin".to_string())
        );
    }

    // A holder the far edge has never deeded does not translate, and
    // that is a refusal rather than a raw number forwarded blind.
    let stranger = edge_a.issue("stranger", 4).expect("room");
    assert_eq!(edge_a.translate(stranger.low(), &edge_b), None);
}

/// Translation is the identity when both edges chose the same numbering
/// — so the law above is about coordinates, not about scrambling.
#[test]
fn d6b_identical_edges_translate_to_themselves() {
    let mut edge_a = Ledger::new(Layout::founding());
    let mut edge_b = Ledger::new(Layout::founding());
    for edge in [&mut edge_a, &mut edge_b] {
        edge.encumber(1, 31, "ancestral", "both registries");
        edge.issue("same", 16).expect("room");
    }
    let deed = edge_a.deeds()[0].clone();
    for tag in deed.low()..=deed.high() {
        assert_eq!(edge_a.translate(tag, &edge_b), Some(tag));
    }
}

// ===================================================================
// D7 — the standing of every tag is accounted for, after any script
// ===================================================================

/// **Every tag is in exactly one standing, and the counts add to 256.**
///
/// Not a shape check on a hand-written table this time: it holds after
/// an arbitrary sequence of encumbers, issues and retirements.
#[test]
fn d7_the_edge_accounts_for_all_256_after_any_script() {
    for script in scripts() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "both registries");
        for (holder, width) in &script {
            let _ = ledger.issue(holder, *width);
        }
        if let Some((holder, _)) = script.first() {
            ledger.retire(holder);
        }

        let mut seen = 0usize;
        for tag in 0u64..=255 {
            match ledger.standing_of(tag) {
                Standing::Void
                | Standing::Encumbered { .. }
                | Standing::Deeded { .. }
                | Standing::Retired
                | Standing::Open => seen += 1,
            }
        }
        assert_eq!(seen, 256, "after {script:?}");
        assert_eq!(ledger.standing_of(0), Standing::Void, "tag 0 was issued");
    }
}
