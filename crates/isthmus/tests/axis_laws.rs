//! LAWS for the axes: growth stops being linear progression.
//!
//! Before these, the deed space was one line. Issuance marched along
//! it, and when the line filled, the only move was *further along the
//! line* — and past the last tag, nothing. The space could grow in
//! exactly one direction, which is no choice of direction at all.
//!
//! [`Act::Open`] is recorded in the chain like every other act, so an
//! axis is history, not configuration: two edges that opened different
//! axes are different edges, and the difference is readable.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::deed::{chain, cocycle, Ledger, Refused, Standing};
use isthmus::layout::Layout;

/// A one-byte-tag edge: 255 issuable tags, small enough to exhaust.
fn small_edge() -> Ledger {
    Ledger::new(Layout::with_tag_width(1))
}

// ===================================================================
// A9 — WHY A1..A8 DID NOT DISCRIMINATE, and the laws that do
//
// A replayed `Act::Open` doubled the axis count and multiplied the
// volume (`IS-6` §8.1), and every law below passed identically before
// and after the fix. Eight laws, and none of them noticed.
//
// The reason is structural rather than careless. A1..A8 are SCENARIO
// laws: each builds one edge by hand and asserts about that edge. They
// discriminate sharply against defects that change the scenarios they
// build — the mirror mutation died on A5, the shifted crossing on A7 —
// and they are blind to any state they never build. Every one of them
// opens axes with DISTINCT names, so the two-opens-one-name state was
// unreachable from this file.
//
// A law that holds for every state a suite constructs, when the suite
// constructs a proper subset of the reachable states, is a law that
// cannot fail on the rest. That is the same shape as a gate that
// cannot fail, hiding one level up in the choice of fixtures.
//
// So the laws below are quantified over GENERATED scripts, and the
// scripts deliberately include the shapes A1..A8 never build: an axis
// opened twice, an axis opened three times, opens interleaved with
// issuance, and a name reused after other axes exist.
// ===================================================================

/// One scripted move. Enough to reach the states the scenario laws
/// miss, and no more — a generator that can build anything is a
/// generator nobody can reason about.
#[derive(Debug, Clone)]
enum Move {
    Open(&'static str, u64),
    Issue(&'static str, u128),
    Retire(&'static str),
    Encumber(u64, u64),
}

/// Deterministic scripts. Enumerated, not sampled: a pass means *none
/// of these*, which is a claim, rather than *these draws*, which is a
/// report.
///
/// The last four are the ones that matter — they are the states
/// `A1..A8` cannot reach.
fn scripts() -> Vec<(&'static str, Vec<Move>)> {
    use Move::{Encumber, Issue, Open, Retire};
    vec![
        ("bare", vec![]),
        ("one line", vec![Issue("a", 8)]),
        ("one axis", vec![Open("rev", 3), Issue("a", 8)]),
        (
            "two axes",
            vec![Open("rev", 3), Open("hue", 5), Issue("a", 8)],
        ),
        (
            "axes around issuance",
            vec![
                Encumber(1, 31),
                Issue("a", 8),
                Open("rev", 3),
                Issue("b", 8),
                Open("hue", 5),
                Retire("a"),
            ],
        ),
        // ---- the states A1..A8 never build ----
        ("one axis, opened twice", vec![Open("rev", 3), Open("rev", 3)]),
        (
            "one axis, opened three times, with issuance between",
            vec![
                Open("rev", 3),
                Issue("a", 8),
                Open("rev", 3),
                Issue("b", 8),
                Open("rev", 3),
            ],
        ),
        (
            "a name reused after other axes exist",
            vec![
                Open("rev", 3),
                Open("hue", 5),
                Open("rev", 3),
                Issue("a", 8),
            ],
        ),
        (
            "every axis opened twice",
            vec![
                Open("rev", 3),
                Open("hue", 5),
                Open("rev", 3),
                Open("hue", 5),
                Issue("a", 8),
            ],
        ),
    ]
}

fn run(script: &[Move]) -> Ledger {
    let mut edge = small_edge();
    for step in script {
        match step {
            Move::Open(name, max) => edge.open_axis(name, *max),
            Move::Issue(holder, width) => {
                // On a multi-axis edge a line issue is still lawful; it
                // pins to the zero slice. Refusals are fine here — the
                // laws are about the SPACE, not about who got room.
                let _ = edge.issue(holder, *width);
            }
            Move::Retire(holder) => {
                edge.retire(holder);
            }
            Move::Encumber(low, high) => edge.encumber(*low, *high, "neighbour", "a script"),
        }
    }
    edge
}

/// **The axis count is the number of DISTINCT names opened**, plus the
/// layout's own axis.
///
/// This is the law the defect violated, stated as a law rather than as
/// a scenario. Under the un-deduped fold, "one axis, opened twice"
/// reports three axes for two names.
#[test]
fn a9_the_axis_count_is_the_number_of_distinct_names() {
    for (name, script) in scripts() {
        let edge = run(&script);

        let distinct: std::collections::BTreeSet<&str> = script
            .iter()
            .filter_map(|step| match step {
                Move::Open(axis, _) => Some(*axis),
                _ => None,
            })
            .collect();

        assert_eq!(
            edge.axes().len(),
            distinct.len() + 1,
            "{name}: {} axes for {} distinct names plus the layout's",
            edge.axes().len(),
            distinct.len(),
        );

        // And no axis is named twice, which is the same claim read off
        // the answer rather than off the input.
        let mut seen = std::collections::BTreeSet::new();
        for axis in edge.axes() {
            assert!(
                seen.insert(axis.name.clone()),
                "{name}: axis {:?} appears twice in the space",
                axis.name,
            );
        }
    }
}

/// **The volume is the product of the axes, and nothing else moves it.**
///
/// Stated separately from `a9` because a fold could get the count right
/// and the extents wrong. Under the un-deduped fold a doubled axis
/// multiplied the volume by its extent again — space appearing from a
/// repeated declaration.
/// **The axes' extents are exactly the distinct declarations, in
/// order.**
///
/// This was `a9_the_volume_is_the_product_of_the_distinct_axes`, and
/// `Ledger::volume()` is struck — it was a product across axes, and a
/// product calls `[2, 8]` and `[4, 4]` the same space. The property it
/// was reaching for is per axis and is stated that way: each axis
/// carries the extent its `Open` declared, first declaration winning.
///
/// Computed from the SCRIPT, never from `axes()`. An expectation
/// derived from the answer is not an expectation — the volume version
/// of this law folded `edge.axes()` into its own expectation and passed
/// under the un-deduped mutation.
#[test]
fn a9_each_axis_carries_the_extent_it_was_declared_with() {
    for (name, script) in scripts() {
        let edge = run(&script);

        // Distinct names, first declaration winning — the fold's rule,
        // restated independently rather than consulted.
        let mut declared: Vec<(&str, u64)> = Vec::new();
        for step in &script {
            if let Move::Open(axis, max) = step {
                if !declared.iter().any(|(seen, _)| seen == axis) {
                    declared.push((*axis, *max));
                }
            }
        }

        let axes = edge.axes();
        assert_eq!(
            axes.len(),
            declared.len() + 1,
            "{name}: axis count disagrees with the declarations",
        );
        // Axis 0 is the layout's own.
        assert_eq!(
            axes[0].max,
            Layout::with_tag_width(1).max_tag().unwrap_or(0),
            "{name}: axis 0 is not the layout's",
        );
        for (at, (axis, max)) in declared.iter().enumerate() {
            let held = &axes[at + 1];
            assert_eq!(held.name, *axis, "{name}: axis {} is misnamed", at + 1);
            assert_eq!(
                held.max, *max,
                "{name}: axis {axis} carries the wrong extent",
            );
        }

        // Issuance never changes an axis's extent — only opening does.
        let opens_only: Vec<Move> = script
            .iter()
            .filter(|s| matches!(s, Move::Open(..)))
            .cloned()
            .collect();
        assert_eq!(
            run(&opens_only).axes(),
            axes,
            "{name}: something other than an Open changed an extent",
        );
    }
}

/// **Every fold agrees about the shape of the space.**
///
/// Four separate folds walk the acts counting axes, and before
/// `axes_timeline` each kept its own counter. A law that only ever
/// asked `axes()` could not see the other three drift away from it, and
/// a fold reading a different dimensionality than `axes()` reports
/// would pad regions to the wrong width — silently, because padding is
/// total.
///
/// The arity comes from the **script**, for the reason the volume law
/// records: taken from `axes()`, every fold could be consistently wrong
/// and still agree with itself.
#[test]
fn a9_every_fold_reads_the_same_dimensionality() {
    for (name, script) in scripts() {
        let edge = run(&script);
        let distinct: std::collections::BTreeSet<&str> = script
            .iter()
            .filter_map(|step| match step {
                Move::Open(axis, _) => Some(*axis),
                _ => None,
            })
            .collect();
        let axes = distinct.len() + 1;

        // `deeds()` pads to the final shape.
        for deed in edge.deeds() {
            assert_eq!(
                deed.region.len(),
                axes,
                "{name}: {}'s region has {} coordinates in a {axes}-axis space",
                deed.holder,
                deed.region.len(),
            );
        }

        // `standing_at` answers at that arity, and the void runs
        // through every axis at the origin.
        let origin = vec![0u64; axes];
        assert_eq!(edge.standing_at(&origin), Standing::Void, "{name}: the void moved");

        // `taken_regions` feeds `open()`/`gaps()`; if it read a
        // different width than `deeds()`, open space and deeded space
        // would disagree about the same points.
        for deed in edge.deeds().into_iter().filter(|d| d.live) {
            for tag in deed.low()..=deed.high() {
                let mut point = vec![0u64; axes];
                if let Some(first) = point.first_mut() {
                    *first = tag;
                }
                assert_eq!(
                    edge.standing_at(&point),
                    Standing::Deeded {
                        holder: deed.holder.clone()
                    },
                    "{name}: {point:?} is deeded to {} by deeds() and not by \
                     the standing fold",
                    deed.holder,
                );
            }
        }
    }
}

/// **The generator reaches the states the scenario laws do not.**
///
/// Without this, `a9` could be quantified over scripts that all happen
/// to open distinct names, and it would be exactly as blind as the
/// eight laws it was written to answer for.
#[test]
fn a9_the_scripts_actually_build_the_missed_states() {
    let mut with_repeats = 0usize;
    let mut max_repeat = 0usize;
    for (_, script) in scripts() {
        let mut counts: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for step in &script {
            if let Move::Open(axis, _) = step {
                *counts.entry(*axis).or_insert(0) += 1;
            }
        }
        let repeated = counts.values().filter(|c| **c > 1).count();
        if repeated > 0 {
            with_repeats += 1;
        }
        max_repeat = max_repeat.max(counts.values().copied().max().unwrap_or(0));
    }
    assert!(
        with_repeats >= 4,
        "only {with_repeats} scripts open an axis twice — the generator \
         does not reach the state that made this section necessary",
    );
    assert!(
        max_repeat >= 3,
        "no script opens one axis three times; two is not evidence that \
         the fold dedups rather than counting to two",
    );
}

// ===================================================================
// A1 — the refutation of linear progression
// ===================================================================

/// **A full line is not a full space.** Exhaust the line so that
/// attachment refuses; open one axis; the same attachment succeeds —
/// and nobody was displaced to make the room.
#[test]
fn a1_a_full_edge_grows_by_opening_an_axis_not_by_displacing_anyone() {
    let mut edge = small_edge();

    // Exhaust the line.
    let mut filled = 0usize;
    while edge.issue(&format!("m{filled}"), 16).is_ok() {
        filled += 1;
    }
    assert!(filled > 0);
    assert!(
        matches!(edge.issue("late", 16), Err(Refused::NoRun { .. })),
        "the line should be exhausted"
    );
    let held_before: Vec<_> = edge.deeds();
    let axes_before = edge.axes();

    // Open an axis. The space gains a DIRECTION — stated as the new
    // component rather than as a product, because a product would call
    // this the same as widening an existing axis fourfold and it is
    // not the same space.
    edge.open_axis("revision", 3);
    let axes_after = edge.axes();
    assert_eq!(axes_after.len(), axes_before.len() + 1);
    assert_eq!(&axes_after[..axes_before.len()], &axes_before[..],
        "opening a direction moved an existing axis");
    assert_eq!(axes_after[axes_before.len()].name, "revision");
    assert_eq!(axes_after[axes_before.len()].max, 3, "the new axis's extent");

    // The same attachment that refused now lands — off the line.
    let deed = edge
        .issue_box("late", &[16, 1])
        .expect("a full line refused this; a space must not");
    assert!(
        deed.region[1].0 >= 1,
        "the new deed landed on the zero slice, which was full — \
         the axis opened nothing"
    );

    // And nobody moved. Every deed from the line era still holds
    // exactly its points on the zero slice.
    for old in &held_before {
        for tag in old.low()..=old.high() {
            assert_eq!(
                edge.standing_at(&[tag, 0]),
                Standing::Deeded {
                    holder: old.holder.clone()
                },
                "opening an axis moved {} off tag {tag}",
                old.holder
            );
        }
    }
}

// ===================================================================
// A2 — acts before an axis pin to the zero slice
// ===================================================================

/// **The line the edge was is the zero slice of the space it becomes.**
/// An old deed covers `(tag, 0)` and nothing above it — so the opened
/// coordinates are genuinely open, and opening an axis grants nobody
/// anything.
#[test]
fn a2_the_old_line_is_the_zero_slice_and_the_rest_is_open() {
    let mut edge = small_edge();
    let deed = edge.issue("liner", 8).expect("room");
    edge.open_axis("colour", 10);

    for tag in deed.low()..=deed.high() {
        assert_eq!(
            edge.standing_at(&[tag, 0]),
            Standing::Deeded {
                holder: "liner".into()
            }
        );
        for colour in 1..=10u64 {
            assert_eq!(
                edge.standing_at(&[tag, colour]),
                Standing::Open,
                "({tag}, {colour}) should be open — the deed predates the axis"
            );
        }
    }

    // Encumbrances pin the same way: an observation of a line is an
    // observation of a line, not of a space that did not exist yet.
    let mut observed = small_edge();
    observed.encumber(1, 31, "ancestral", "both registries");
    observed.open_axis("colour", 10);
    assert!(matches!(
        observed.standing_at(&[20, 0]),
        Standing::Encumbered { .. }
    ));
    assert_eq!(observed.standing_at(&[20, 5]), Standing::Open);
}

// ===================================================================
// A3 — boxes do not overlap, and the void is a line of points
// ===================================================================

/// **No point is held twice**, across mixed line-era and box-era
/// issuance, and the void extends through every axis.
#[test]
fn a3_no_point_is_held_twice_and_the_void_runs_through_every_axis() {
    let mut edge = small_edge();
    let _ = edge.issue("first", 40);
    edge.open_axis("revision", 3);
    let _ = edge.issue_box("second", &[40, 2]);
    let _ = edge.issue_box("third", &[100, 4]);
    let _ = edge.issue("fourth", 40); // line issuance still works after axes

    // A LINE act recorded AFTER the axis opened still pins to the zero
    // slice. This is the assertion the first draft of this law lacked:
    // a mutation expanding low-dimension acts across new axes survived,
    // because every deed was disjoint on axis 0 and the overlap check
    // below cannot see an expansion that overlaps nobody.
    let fourth = edge
        .deeds()
        .into_iter()
        .find(|d| d.holder == "fourth")
        .expect("issued");
    for tag in fourth.low()..=fourth.high() {
        for revision in 1..=3u64 {
            assert_eq!(
                edge.standing_at(&[tag, revision]),
                Standing::Open,
                "a 1-D act expanded into the revision axis at ({tag}, {revision})"
            );
        }
    }

    let deeds = edge.deeds();
    for tag in 0..=255u64 {
        for revision in 0..=3u64 {
            let point = [tag, revision];
            let holders: Vec<_> = deeds
                .iter()
                .filter(|d| d.covers_at(&point))
                .map(|d| d.holder.clone())
                .collect();
            assert!(
                holders.len() <= 1,
                "({tag}, {revision}) held by {holders:?}"
            );
            if tag == 0 {
                assert_eq!(
                    edge.standing_at(&point),
                    Standing::Void,
                    "the void must run through every axis — a zero-filled \
                     buffer decodes to tag 0 at ANY coordinate above it"
                );
            }
        }
    }
}

// ===================================================================
// A4 — the chain carries the axes
// ===================================================================

/// **A space round-trips through storage.** Opens, boxes and line acts
/// together; the replayed fold answers identically at every probed
/// point.
#[test]
fn a4_a_chain_with_axes_round_trips() {
    let mut edge = small_edge();
    edge.encumber(1, 31, "ancestral", "both registries");
    let _ = edge.issue("liner", 16);
    edge.open_axis("revision", 7);
    let _ = edge.issue_box("boxer", &[32, 4]);
    edge.open_axis("colour", 11);
    let _ = edge.issue_box("volumetric", &[8, 2, 3]);
    edge.retire("liner");

    let stored = chain::encode(edge.acts());
    let acts = chain::decode(&stored).expect("its own bytes");
    assert_eq!(acts, edge.acts());

    let replayed = Ledger::replay(Layout::with_tag_width(1), acts);
    assert_eq!(replayed.axes(), edge.axes());

    for tag in [0u64, 1, 20, 32, 40, 100, 255] {
        for revision in [0u64, 1, 4, 7] {
            for colour in [0u64, 2, 11] {
                let point = [tag, revision, colour];
                assert_eq!(
                    replayed.standing_at(&point),
                    edge.standing_at(&point),
                    "the replay answers differently at {point:?}"
                );
            }
        }
    }
}

/// **An old reader refuses a chain with axes rather than misfolding
/// it.** The founding chain (tags 1–3) decodes everywhere; a chain
/// carrying an Open is a history an old fold cannot hold, and D1d's
/// unknown-act rule is what keeps it from folding the line as if the
/// axis never happened.
#[test]
fn a4b_the_new_acts_are_additive_and_unknown_to_nobody_silently() {
    // A line-era chain: only tags 1-3 appear in its storage.
    let mut line = small_edge();
    line.encumber(1, 31, "old", "somewhere");
    let _ = line.issue("liner", 8);
    let stored = chain::encode(line.acts());
    let mut tags_used: Vec<u64> = Vec::new();
    let mut reader = isthmus::frame::Reader::new(&stored);
    while !reader.is_done() {
        let (tag, _) = reader.frame(&Layout::founding()).expect("well-formed");
        tags_used.push(tag);
    }
    assert!(
        tags_used.iter().all(|t| *t <= 3),
        "a line-era chain used a box-era tag: {tags_used:?}"
    );

    // And the box-era acts occupy 4-6, which an old decoder's
    // unknown-act rule refuses whole. Checked live in d1d; here we only
    // pin that the new acts actually use the new numbers.
    let mut spatial = small_edge();
    spatial.open_axis("revision", 3);
    let spatial_stored = chain::encode(spatial.acts());
    let mut reader = isthmus::frame::Reader::new(&spatial_stored);
    let (tag, _) = reader.frame(&Layout::founding()).expect("well-formed");
    assert_eq!(tag, chain::OPEN);
}

// ===================================================================
// A5 — translation is per-axis
// ===================================================================

/// **A point crosses edges coordinate by coordinate.** Two edges with
/// different histories deed the same holder different boxes of the same
/// shape; a point maps by per-axis offset, and a point outside the box
/// does not map at all.
#[test]
fn a5_a_point_translates_across_edges_by_offset_per_axis() {
    let mut edge_a = small_edge();
    let mut edge_b = small_edge();

    edge_a.open_axis("revision", 7);
    edge_b.encumber(1, 90, "a busier neighbour", "their advert");
    edge_b.open_axis("revision", 7);

    let on_a = edge_a.issue_box("chitin", &[16, 4]).expect("room");
    let on_b = edge_b.issue_box("chitin", &[16, 4]).expect("room");
    assert_ne!(on_a.region, on_b.region, "different histories, same box");

    for dt in 0..16u64 {
        for dr in 0..4u64 {
            let here = [on_a.region[0].0 + dt, on_a.region[1].0 + dr];
            let there = edge_a
                .translate_at(&here, &edge_b)
                .unwrap_or_else(|| panic!("{here:?} did not translate"));
            assert_eq!(there, vec![on_b.region[0].0 + dt, on_b.region[1].0 + dr]);
        }
    }

    // Outside the box: no deed, no translation, no guess.
    assert_eq!(edge_a.translate_at(&[200, 0], &edge_b), None);
}

// ===================================================================
// A6 — a transduction is only sound if the return trip is the identity
// ===================================================================

/// **`B(A(p)) = p` for every point in the deed, both directions.**
///
/// ## Measured: this law and A5 catch DIFFERENT malformations
///
/// A mirror mutation — offsets reflected within the box, consistently,
/// in range — **passed this law and failed A5**. A mirror is an
/// involution: applying it on both crossings cancels, so the round trip
/// is the identity while every single crossing is wrong. **Zero
/// holonomy does not mean no distortion; it means the distortions
/// cancel** — the circulation wall's lesson, one layer up.
///
/// So neither law subsumes the other:
///
/// - **A5 is anchored**: it knows the expected coordinates, so any
///   change to any mapped point fails it — but it needs ground truth
///   about both numberings, which two live peers do not share.
/// - **A6 is a cycle law**: it needs no ground truth at all — two
///   peers can check it against each other with nothing but the wire —
///   but it is blind to involutions, exactly as a vanishing holonomy is
///   blind to a gauge that cancels itself.
///
/// The pair separates what neither alone can. A lossy or shifting
/// transducer fails A6; a self-inverse one fails A5; a sound one fails
/// neither.
#[test]
fn a6_translation_round_trips_to_the_identity() {
    let mut edge_a = small_edge();
    let mut edge_b = small_edge();

    edge_a.open_axis("revision", 7);
    edge_b.encumber(1, 90, "a busier neighbour", "their advert");
    edge_b.open_axis("revision", 7);

    let on_a = edge_a.issue_box("chitin", &[16, 4]).expect("room");
    let on_b = edge_b.issue_box("chitin", &[16, 4]).expect("room");
    assert_ne!(on_a.region, on_b.region, "the trip must actually move");

    let mut crossed = 0usize;
    for dt in 0..16u64 {
        for dr in 0..4u64 {
            let here = vec![on_a.region[0].0 + dt, on_a.region[1].0 + dr];

            let there = edge_a
                .translate_at(&here, &edge_b)
                .unwrap_or_else(|| panic!("{here:?} did not cross"));
            let back = edge_b
                .translate_at(&there, &edge_a)
                .unwrap_or_else(|| panic!("{there:?} did not return"));

            assert_eq!(back, here, "the round trip moved the point");
            crossed += 1;
        }
    }
    assert_eq!(crossed, 64, "the whole box crossed and returned");
}

// ===================================================================
// A7 — cycles are run in cocycles: verification is per edge
// ===================================================================

/// **Every crossing satisfies the cocycle condition on its own edge.**
///
/// Each node computes its potential — holder and in-box offset — from
/// its own deeds alone. The mirror mutation that PASSED the round trip
/// (A6) fails here on a **single crossing**, because a mirror moves the
/// offset and the offset is checked per edge, where cancellation has
/// nowhere to happen.
#[test]
fn a7_every_crossing_verifies_against_the_potentials_alone() {
    let mut edge_a = small_edge();
    let mut edge_b = small_edge();

    edge_a.open_axis("revision", 7);
    edge_b.encumber(1, 90, "a busier neighbour", "their advert");
    edge_b.open_axis("revision", 7);

    let on_a = edge_a.issue_box("chitin", &[16, 4]).expect("room");
    let _on_b = edge_b.issue_box("chitin", &[16, 4]).expect("room");

    for dt in 0..16u64 {
        for dr in 0..4u64 {
            let here = vec![on_a.region[0].0 + dt, on_a.region[1].0 + dr];
            let there = edge_a
                .translate_at(&here, &edge_b)
                .unwrap_or_else(|| panic!("{here:?} did not cross"));

            assert!(
                cocycle(&edge_a, &here, &edge_b, &there),
                "the crossing {here:?} -> {there:?} fails the cocycle \
                 condition on its own edge"
            );
        }
    }

    // The gate separates: a crossing that lands one off fails, and a
    // point outside any deed verifies with nothing.
    let here = vec![on_a.region[0].0, on_a.region[1].0];
    let there = edge_a.translate_at(&here, &edge_b).expect("crosses");
    let mut shifted = there.clone();
    shifted[0] += 1;
    assert!(!cocycle(&edge_a, &here, &edge_b, &shifted));
    assert!(!cocycle(&edge_a, &[200, 0], &edge_b, &there));
}

// ===================================================================
// A8 — the loop identity is a corollary, never a separate trust
// ===================================================================

/// **Per-edge cocycle verification around a 3-node cycle, and the loop
/// closes as a consequence.**
///
/// A → B → C → A, three edges with three different histories. Every hop
/// is verified on its own edge against the potentials; nothing checks
/// the loop total — and the loop total is the identity anyway, because
/// a composition of offset-preserving maps preserves the offset. The
/// cycle is run in cocycles.
#[test]
fn a8_a_three_node_cycle_closes_because_every_edge_verifies() {
    let mut node_a = small_edge();
    let mut node_b = small_edge();
    let mut node_c = small_edge();

    node_a.open_axis("revision", 7);
    node_b.encumber(1, 90, "a busier neighbour", "their advert");
    node_b.open_axis("revision", 7);
    node_c.encumber(1, 40, "someone else", "their advert");
    node_c.open_axis("revision", 7);
    // A line-era encumbrance pins to the zero slice, so it does not
    // shift a box that hops off the slice — B and C would land on the
    // SAME origin, measured, and a cycle with two identical frames is a
    // weaker cycle. A full-height spacer forces C's frame elsewhere.
    //
    // The first spacer was [30, 8] on an axis whose max is 7 — refused,
    // and a `let _` swallowed the refusal, so C landed on B's origin
    // anyway. The extent is 8 coordinates as 1..=7 plus the zero slice;
    // asking for 8 ABOVE the slice cannot fit. Asserted now, not
    // discarded.
    node_c
        .issue_box("spacer", &[30, 7])
        .expect("the spacer must land or the cycle is degenerate");

    let on_a = node_a.issue_box("chitin", &[16, 4]).expect("room");
    let on_b = node_b.issue_box("chitin", &[16, 4]).expect("room");
    let on_c = node_c.issue_box("chitin", &[16, 4]).expect("room");
    assert_ne!(on_a.region, on_b.region);
    assert_ne!(on_b.region, on_c.region);

    let mut closed = 0usize;
    for dt in [0u64, 3, 15] {
        for dr in [0u64, 1, 3] {
            let p = vec![on_a.region[0].0 + dt, on_a.region[1].0 + dr];

            let q = node_a.translate_at(&p, &node_b).expect("A -> B");
            let r = node_b.translate_at(&q, &node_c).expect("B -> C");
            let back = node_c.translate_at(&r, &node_a).expect("C -> A");

            // Verification is PER EDGE. No hop consults any other.
            assert!(cocycle(&node_a, &p, &node_b, &q), "edge A-B");
            assert!(cocycle(&node_b, &q, &node_c, &r), "edge B-C");
            assert!(cocycle(&node_c, &r, &node_a, &back), "edge C-A");

            // And the loop identity FOLLOWS. It is asserted here as the
            // corollary it is, not because anything needed to check it.
            assert_eq!(back, p, "three verified edges did not close");
            closed += 1;
        }
    }
    assert_eq!(closed, 9);
    // Same potential all the way round — the invariant that survives
    // every frame on the cycle.
    let p = vec![on_a.region[0].0 + 5, on_a.region[1].0 + 2];
    let (holder, offsets) = node_a.potential_at(&p).expect("in the deed");
    assert_eq!(holder, "chitin");
    assert_eq!(offsets, vec![5, 2]);
}
