//! THE DEED SUITE — every chain/estate law in one binary: axes,
//! binds, containment, deeds, derivations, replays, spheres,
//! and the theorems.

#![allow(clippy::arithmetic_side_effects, clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod axis_laws {


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

        let on_a = edge_a.issue_box("kernel-a", &[16, 4]).expect("room");
        let on_b = edge_b.issue_box("kernel-a", &[16, 4]).expect("room");
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

        let on_a = edge_a.issue_box("kernel-a", &[16, 4]).expect("room");
        let on_b = edge_b.issue_box("kernel-a", &[16, 4]).expect("room");
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

        let on_a = edge_a.issue_box("kernel-a", &[16, 4]).expect("room");
        let _on_b = edge_b.issue_box("kernel-a", &[16, 4]).expect("room");

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

        let on_a = node_a.issue_box("kernel-a", &[16, 4]).expect("room");
        let on_b = node_b.issue_box("kernel-a", &[16, 4]).expect("room");
        let on_c = node_c.issue_box("kernel-a", &[16, 4]).expect("room");
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
        assert_eq!(holder, "kernel-a");
        assert_eq!(offsets, vec![5, 2]);
    }
}

mod bind_laws {


    use isthmus::deed::{chain, Act, Ledger};
    use isthmus::layout::Layout;

    fn bind(holder: &str, key_byte: u8, from: u64, until: u64) -> Act {
        Act::Bind {
            holder: holder.into(),
            scheme: 0x01,
            key: vec![key_byte; 32],
            from_epoch: from,
            until_epoch: until,
        }
    }

    #[test]
    fn a_bind_round_trips_the_chain_codec() {
        let acts = vec![
            Act::Encumber {
                low: 1,
                high: 31,
                by: "ancestral".into(),
                witnessed: "founding registries".into(),
            },
            Act::Issue {
                holder: "kernel-a".into(),
                low: 64,
                high: 79,
            },
            bind("kernel-a", 7, 0, 100),
        ];
        let bytes = chain::encode(&acts);
        let back = chain::decode(&bytes).expect("its own bytes");
        assert_eq!(back, acts, "byte-identical history, bind included");
    }

    #[test]
    fn the_last_bind_wins_and_history_is_kept() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.record(bind("kernel-a", 1, 0, 10));
        ledger.record(bind("kernel-a", 2, 11, 20));

        let binding = ledger.binding_of("kernel-a").expect("bound");
        assert_eq!(binding.key, vec![2u8; 32], "rotation superseded");
        assert_eq!((binding.from_epoch, binding.until_epoch), (11, 20));

        // The first key is still in the acts — rotation appended, nothing
        // was rewritten.
        let binds = ledger
            .acts()
            .iter()
            .filter(|a| matches!(a, Act::Bind { .. }))
            .count();
        assert_eq!(binds, 2);
    }

    #[test]
    fn an_unbound_holder_is_visibly_unbound() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger.issue("legacy-kernel", 16).expect("room");
        assert_eq!(
            ledger.binding_of("legacy-kernel"),
            None,
            "a keyless grant reads as legacy, never as a random key"
        );
    }

    #[test]
    fn a_bind_covers_no_ground() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger.issue("kernel-a", 16).expect("room");
        let held_before: Vec<_> = ledger.deeds();
        ledger.record(bind("kernel-a", 9, 0, u64::MAX));
        assert_eq!(
            ledger.deeds(),
            held_before,
            "binding is identity, not ground — no deed moved"
        );
        assert!(
            ledger.well_formed().is_ok(),
            "no horizontal rule trips on a vertical fact"
        );
    }

    #[test]
    fn an_old_reader_refuses_a_bound_chain_rather_than_misfolding() {
        // There is no old reader to link, so the property is pinned at the
        // codec seam: the bind's chain tag is outside the founding trio
        // and inside this decoder's table. A reader without the arm
        // refuses (unknown act), which is IS-6's rule: in a chain, skip
        // would fold a different history and report it as this one.
        let bytes = chain::encode(&[bind("kernel-a", 3, 0, 1)]);
        let back = chain::decode(&bytes).expect("this reader speaks IS-6/4");
        assert!(matches!(back.first(), Some(Act::Bind { .. })));
    }
}

/// IS-6/7 — a holder's stake as a chain fact (Phase 5 Fork B).
mod escrow_laws {
    use isthmus::deed::{chain, Act, Ledger};
    use isthmus::layout::Layout;

    fn escrow(holder: &str, amount: u128) -> Act {
        Act::Escrow {
            holder: holder.to_owned(),
            amount,
        }
    }

    #[test]
    fn escrow_of_folds_forward_locks_add_release_zeroes_slash_subtracts() {
        let mut ledger = Ledger::new(Layout::founding());
        assert_eq!(ledger.escrow_of("kernel-a"), 0, "nothing locked yet");

        ledger.record(escrow("kernel-a", 100));
        ledger.record(escrow("kernel-a", 50));
        assert_eq!(ledger.escrow_of("kernel-a"), 150, "locks accumulate");

        ledger.record(Act::Slash {
            holder: "kernel-a".to_owned(),
            amount: 30,
        });
        assert_eq!(ledger.escrow_of("kernel-a"), 120, "a slash subtracts");

        // A different holder's stake is independent.
        ledger.record(escrow("kernel-b", 9));
        assert_eq!(ledger.escrow_of("kernel-b"), 9);
        assert_eq!(ledger.escrow_of("kernel-a"), 120, "holders do not mix");

        ledger.record(Act::Release {
            holder: "kernel-a".to_owned(),
        });
        assert_eq!(ledger.escrow_of("kernel-a"), 0, "release returns to zero");
        assert_eq!(ledger.escrow_of("kernel-b"), 9, "only the named holder");
    }

    #[test]
    fn slashed_total_survives_a_release_but_locked_does_not() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.record(escrow("kernel-a", 100));
        ledger.record(Act::Slash {
            holder: "kernel-a".to_owned(),
            amount: 40,
        });
        ledger.record(Act::Release {
            holder: "kernel-a".to_owned(),
        });
        // Locked resets on release; slashed value is gone for good, so a
        // court's balance_of can subtract it from earned permanently.
        assert_eq!(ledger.escrow_of("kernel-a"), 0);
        assert_eq!(ledger.slashed_of("kernel-a"), 40);
    }

    #[test]
    fn a_slash_cannot_drive_locked_below_zero() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.record(escrow("kernel-a", 10));
        ledger.record(Act::Slash {
            holder: "kernel-a".to_owned(),
            amount: 999,
        });
        assert_eq!(
            ledger.escrow_of("kernel-a"),
            0,
            "saturating: a stake cannot go negative"
        );
    }

    #[test]
    fn a_stake_covers_no_ground() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger.issue("kernel-a", 16).expect("room");
        let held_before = ledger.deeds();
        ledger.record(escrow("kernel-a", 1_000));
        ledger.record(Act::Release {
            holder: "kernel-a".to_owned(),
        });
        assert_eq!(
            ledger.deeds(),
            held_before,
            "a stake is economics, not ground — no deed moved"
        );
        assert!(
            ledger.well_formed().is_ok(),
            "no horizontal rule trips on a balance fact"
        );
    }

    #[test]
    fn a_stake_round_trips_and_a_huge_amount_survives_the_u128_field() {
        let acts = vec![
            escrow("kernel-a", u128::MAX),
            Act::Slash {
                holder: "kernel-a".to_owned(),
                amount: 1,
            },
            Act::Release {
                holder: "kernel-a".to_owned(),
            },
        ];
        let bytes = chain::encode(&acts);
        let back = chain::decode(&bytes).expect("its own bytes");
        assert_eq!(back, acts, "byte-identical, full u128 amount included");
    }
}

mod containment_laws {


    use isthmus::deed::{chain, Act, Flaw, Ledger, Refused, Standing};
    use isthmus::layout::Layout;

    /// A planet, on a one-byte edge, with room around it.
    fn with_planet() -> Ledger {
        let mut edge = Ledger::new(Layout::with_tag_width(1)).under("here");
        edge.encumber(1, 31, "ancestral", "these laws");
        edge.issue("planet", 64).expect("room for a planet");
        edge
    }

    fn planet_region(edge: &Ledger) -> Vec<(u64, u64)> {
        edge.deeds()
            .into_iter()
            .find(|d| d.live && d.holder == "planet")
            .expect("the planet")
            .region
    }

    // ===================================================================
    // C1 — a sublet nests, and a cession transfers
    // ===================================================================

    /// **The owner keeps every point.** This is the whole difference from
    /// [`Ledger::cede`], and stating it as a law is what stops a sublet
    /// from silently becoming a cession with extra words.
    #[test]
    fn c1_the_owner_keeps_its_estate_and_a_cession_does_not() {
        let mut edge = with_planet();
        let before = planet_region(&edge);
        let (low, high) = before[0];

        edge.sublet("planet", "moon", &[(low + 8, low + 15)])
            .expect("a moon inside the planet");

        // The planet is unchanged, point for point.
        assert_eq!(planet_region(&edge), before, "the sublet shrank the owner");
        for tag in low..=high {
            assert!(
                edge.deeds()
                    .iter()
                    .any(|d| d.live && d.holder == "planet" && d.covers(tag)),
                "the planet lost tag {tag} to its own moon",
            );
        }

        // A cession over the same ground DOES shrink it — the contrast is
        // the point, and without it "the owner keeps its estate" could be
        // true of a fold that never shrinks anybody.
        let mut sold = with_planet();
        sold.cede("planet", "buyer", &[(low, low + 15)])
            .expect("a slab flush against the low edge");
        assert_ne!(
            planet_region(&sold),
            before,
            "a cession left the owner's estate unchanged",
        );
    }

    /// **The moon answers for its own points, the planet for the rest.**
    #[test]
    fn c1_the_deepest_holder_answers() {
        let mut edge = with_planet();
        let (low, high) = planet_region(&edge)[0];
        edge.sublet("planet", "moon", &[(low + 8, low + 15)])
            .expect("a moon");

        for tag in low..=high {
            let expected = if (low + 8..=low + 15).contains(&tag) {
                "moon"
            } else {
                "planet"
            };
            assert_eq!(
                edge.standing_of(tag),
                Standing::Deeded {
                    holder: expected.to_owned()
                },
                "tag {tag}",
            );
            assert_eq!(
                edge.holder_of(tag).map(|d| d.holder),
                Some(expected.to_owned()),
                "holder_of disagrees with the standing at tag {tag}",
            );
        }

        // And the planet still holds the moon's ground, one level up —
        // both answers are true, and `contained_in` is where the other one
        // lives.
        let moons = edge.contained_in("planet");
        assert_eq!(moons.len(), 1);
        assert_eq!(moons[0].holder, "moon");
        assert_eq!(edge.depth_of("planet"), 0);
        assert_eq!(edge.depth_of("moon"), 1);
    }

    // ===================================================================
    // C2 — H2', both halves, refused through the issuer
    // ===================================================================

    /// **A moon must be inside its planet, and clear of its siblings.**
    /// Both refusals, and an admitted case for each — a checker that
    /// refused every sublet would pass a refusal table.
    #[test]
    fn c2_the_issuer_refuses_what_h2_prime_forbids() {
        let mut edge = with_planet();
        let (low, high) = planet_region(&edge)[0];

        // Admitted: inside.
        edge.sublet("planet", "moon", &[(low + 8, low + 15)])
            .expect("inside the planet must be admitted");

        // Refused: outside the planet.
        assert!(
            matches!(
                edge.sublet("planet", "outsider", &[(high + 1, high + 8)]),
                Err(Refused::NotContained { .. })
            ),
            "a moon outside the planet was granted",
        );

        // Refused: overlapping a sibling moon.
        assert!(
            matches!(
                edge.sublet("planet", "clash", &[(low + 12, low + 20)]),
                Err(Refused::NoBox { .. })
            ),
            "two moons were granted the same ground",
        );

        // Admitted: a sibling that does not overlap.
        edge.sublet("planet", "second", &[(low + 16, low + 23)])
            .expect("a disjoint sibling must be admitted");

        // Refused: to a holder that already holds (H1 survives nesting).
        assert!(matches!(
            edge.sublet("planet", "moon", &[(low + 24, low + 25)]),
            Err(Refused::AlreadyHeld { .. })
        ));
        // Refused: to itself.
        assert!(matches!(
            edge.sublet("planet", "planet", &[(low + 26, low + 27)]),
            Err(Refused::SelfDeal)
        ));
        // Refused: from a holder with no estate.
        assert!(matches!(
            edge.sublet("nobody", "someone", &[(low, low + 1)]),
            Err(Refused::NoSuchEstate { .. })
        ));

        assert!(edge.well_formed().is_ok(), "the issuer built a flawed chain");
    }

    /// **The checker names the same violations in a transcribed chain**,
    /// and accepts the lawful one. `record()` judges nothing, so this is
    /// the only thing standing between a hostile history and the theorems.
    #[test]
    fn c2_the_checker_discharges_h2_prime_on_a_transcribed_chain() {
        let lawful = |acts: Vec<Act>| {
            let mut edge = Ledger::new(Layout::with_tag_width(1));
            for act in acts {
                edge.record(act);
            }
            edge
        };
        let planet = || Act::Issue {
            holder: "planet".to_owned(),
            low: 32,
            high: 95,
        };

        // Admitted.
        assert!(lawful(vec![
            planet(),
            Act::Sublet {
                from: "planet".to_owned(),
                to: "moon".to_owned(),
                region: vec![(40, 47)],
            },
        ])
        .well_formed()
        .is_ok());

        // Not inside the planet.
        assert_eq!(
            lawful(vec![
                planet(),
                Act::Sublet {
                    from: "planet".to_owned(),
                    to: "moon".to_owned(),
                    region: vec![(100, 107)],
                },
            ])
            .well_formed(),
            Err(Flaw::BadSublet { at: 1 }),
        );

        // Overlapping a sibling.
        assert_eq!(
            lawful(vec![
                planet(),
                Act::Sublet {
                    from: "planet".to_owned(),
                    to: "moon".to_owned(),
                    region: vec![(40, 47)],
                },
                Act::Sublet {
                    from: "planet".to_owned(),
                    to: "clash".to_owned(),
                    region: vec![(44, 51)],
                },
            ])
            .well_formed(),
            Err(Flaw::BadSublet { at: 2 }),
        );

        // From nobody.
        assert_eq!(
            lawful(vec![Act::Sublet {
                from: "ghost".to_owned(),
                to: "moon".to_owned(),
                region: vec![(40, 47)],
            }])
            .well_formed(),
            Err(Flaw::BadSublet { at: 0 }),
        );
    }

    // ===================================================================
    // C3 — the property H2 was only ever a way of getting
    // ===================================================================

    /// **The containment chain over every point is totally ordered, so
    /// `holder_at` is a function.**
    ///
    /// Checked point by point over a three-deep nesting: at each point the
    /// deeds covering it have *distinct* depths, and the deepest is the one
    /// `holder_at` answers. Distinct depths is exactly `H2′` — same-depth
    /// deeds are disjoint, so no point is covered twice at one level.
    #[test]
    fn c3_the_containment_chain_over_a_point_is_totally_ordered() {
        let mut edge = with_planet();
        let (low, _) = planet_region(&edge)[0];
        edge.sublet("planet", "moon", &[(low + 8, low + 23)])
            .expect("a moon");
        edge.sublet("moon", "station", &[(low + 12, low + 15)])
            .expect("a station on the moon");
        edge.sublet("planet", "sibling", &[(low + 32, low + 39)])
            .expect("a second moon, disjoint");

        assert_eq!(edge.depth_of("station"), 2, "three levels deep");

        for tag in 0u64..=255 {
            let covering: Vec<_> = edge
                .deeds()
                .into_iter()
                .filter(|d| d.covers(tag))
                .collect();

            // Distinct depths — H2' read off the answer.
            let mut depths: Vec<usize> =
                covering.iter().map(|d| edge.depth_of(&d.holder)).collect();
            depths.sort_unstable();
            let mut unique = depths.clone();
            unique.dedup();
            assert_eq!(
                depths, unique,
                "tag {tag} is covered twice at one depth by {:?}",
                covering.iter().map(|d| &d.holder).collect::<Vec<_>>(),
            );

            // And the answer is the deepest of them.
            let deepest = covering
                .iter()
                .max_by_key(|d| edge.depth_of(&d.holder))
                .map(|d| d.holder.clone());
            assert_eq!(edge.holder_of(tag).map(|d| d.holder), deepest, "tag {tag}");
        }
    }

    /// **Buying out of a moon does not lift the ground out of the planet.**
    ///
    /// A cession from a sublessee conveys the seller's containment with it.
    /// Without that, a moon could sell its ground to a stranger and the
    /// stranger would hold it at depth 0 — an escape hatch out of every
    /// estate, reachable in two acts.
    #[test]
    fn c3_a_cession_from_a_moon_stays_inside_the_planet() {
        let mut edge = with_planet();
        let (low, _) = planet_region(&edge)[0];
        edge.sublet("planet", "moon", &[(low + 8, low + 23)])
            .expect("a moon");

        let bought = edge
            .cede("moon", "buyer", &[(low + 8, low + 11)])
            .expect("a slab of the moon, flush against its low edge");

        assert_eq!(
            bought.within.as_deref(),
            Some("planet"),
            "the buyer escaped the planet by buying from its moon",
        );
        assert_eq!(edge.depth_of("buyer"), 1, "the buyer is still one level in");
        assert!(edge.well_formed().is_ok());
    }

    // ===================================================================
    // C4 — the cascade structure, and the chain that carries it
    // ===================================================================

    /// **Containment is walkable one level at a time**, which is what the
    /// compensation cascade needs: displacing a planet displaces every moon
    /// in it, and what is owed is owed *through* each level.
    ///
    /// This builds the structure and asserts its shape. It prices nothing —
    /// pricing is the board's, and the lab's `decide/arbitration.md` rules it must
    /// not be a scalar.
    #[test]
    fn c4_the_cascade_is_walkable_level_by_level() {
        let mut edge = with_planet();
        let (low, _) = planet_region(&edge)[0];
        edge.sublet("planet", "moon", &[(low + 8, low + 23)])
            .expect("a moon");
        edge.sublet("planet", "sibling", &[(low + 32, low + 39)])
            .expect("a second moon");
        edge.sublet("moon", "station", &[(low + 12, low + 15)])
            .expect("a station");

        assert_eq!(
            edge.contained_in("planet")
                .into_iter()
                .map(|d| d.holder)
                .collect::<Vec<_>>(),
            vec!["moon".to_owned(), "sibling".to_owned()],
        );
        assert_eq!(
            edge.contained_in("moon")
                .into_iter()
                .map(|d| d.holder)
                .collect::<Vec<_>>(),
            vec!["station".to_owned()],
        );
        assert!(edge.contained_in("station").is_empty());
        assert!(edge.contained_in("nobody").is_empty());

        // Walking the whole subtree reaches everything below the planet,
        // and reaches it in containment order.
        let mut reached = Vec::new();
        let mut frontier = vec!["planet".to_owned()];
        while let Some(holder) = frontier.pop() {
            for moon in edge.contained_in(&holder) {
                reached.push(moon.holder.clone());
                frontier.push(moon.holder);
            }
        }
        reached.sort();
        assert_eq!(reached, vec!["moon", "sibling", "station"]);

        // Every moon's volume is inside its parent's, so a cascade that
        // priced by volume could not owe more than the parent occupies.
        for holder in ["moon", "sibling", "station"] {
            let deed = edge
                .deeds()
                .into_iter()
                .find(|d| d.live && d.holder == holder)
                .expect("live");
            let parent = deed.within.clone().expect("a moon has a parent");
            let above = edge
                .deeds()
                .into_iter()
                .find(|d| d.live && d.holder == parent)
                .expect("the parent is live");
            // PER AXIS, which is strictly stronger than the product it
            // replaced: `[2, 8]` has the same volume as `[4, 4]` and fits
            // inside neither, so a volume comparison would have admitted a
            // moon that leaves its planet on one axis and re-enters on
            // another.
            assert_eq!(deed.region.len(), above.region.len(), "{holder} and {parent} differ in arity");
            for (at, ((mine_low, mine_high), (their_low, their_high))) in
                deed.region.iter().zip(above.region.iter()).enumerate()
            {
                assert!(
                    mine_low >= their_low && mine_high <= their_high,
                    "{holder} leaves {parent} on axis {at}: {mine_low}-{mine_high} \
                     outside {their_low}-{their_high}",
                );
            }
        }
    }

    /// **The nesting survives storage.** Containment is folded from the
    /// acts, so a replayed chain reconstructs it without carrying it.
    #[test]
    fn c4_containment_round_trips_through_the_chain() {
        let mut edge = with_planet();
        let (low, _) = planet_region(&edge)[0];
        edge.sublet("planet", "moon", &[(low + 8, low + 23)])
            .expect("a moon");
        edge.sublet("moon", "station", &[(low + 12, low + 15)])
            .expect("a station");
        edge.open_axis("revision", 3);

        let stored = chain::encode(edge.acts());
        let acts = chain::decode(&stored).expect("its own bytes");
        assert_eq!(acts, edge.acts(), "the sublet did not survive the wire");

        let replayed = Ledger::replay(Layout::with_tag_width(1), acts);
        for holder in ["planet", "moon", "station"] {
            assert_eq!(
                replayed.depth_of(holder),
                edge.depth_of(holder),
                "{holder} changed depth in storage",
            );
        }
        for tag in 0u64..=255 {
            assert_eq!(
                replayed.standing_of(tag),
                edge.standing_of(tag),
                "the replay answers differently at tag {tag}",
            );
        }
        assert!(replayed.well_formed().is_ok());
    }

    /// **A cycle in containment cannot be built, and does not hang the fold
    /// if one is transcribed.**
    ///
    /// The issuer cannot make one — a sublessee must hold nothing, so it
    /// can never already be somebody's parent. A hostile chain can write
    /// one, and `depth_of` walks `within`, so a loop there would be an
    /// infinite loop in a total crate.
    #[test]
    fn c4_a_transcribed_containment_cycle_terminates() {
        let mut edge = Ledger::new(Layout::with_tag_width(1));
        edge.record(Act::Issue {
            holder: "a".to_owned(),
            low: 32,
            high: 95,
        });
        edge.record(Act::Sublet {
            from: "a".to_owned(),
            to: "b".to_owned(),
            region: vec![(40, 63)],
        });
        // b now contains a: a cycle the issuer would refuse.
        edge.record(Act::Sublet {
            from: "b".to_owned(),
            to: "a".to_owned(),
            region: vec![(44, 47)],
        });

        // Terminates. That is the assertion — reaching this line at all.
        let depth = edge.depth_of("a");
        assert!(depth <= 3, "depth_of walked a cycle and kept counting: {depth}");

        // And the checker names it rather than letting it stand: `a`
        // already holds, so the second sublet is refused.
        assert_eq!(edge.well_formed(), Err(Flaw::BadSublet { at: 2 }));
    }
}

mod deed_laws {


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
        ledger.encumber(32, 54, "external-registry", "external registry");

        match ledger.acts().first() {
            Some(Act::Encumber { by, witnessed, .. }) => {
                assert_eq!(by, "external-registry");
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
    /// over numbers an external registry already claimed, and it was written without
    /// checking. Here it is a law rather than a table review.
    #[test]
    fn d3_an_encumbrance_is_never_issued_over() {
        for script in scripts() {
            let mut ledger = Ledger::new(Layout::founding());
            // Deliberately awkward: encumbrances scattered so a naive
            // first-fit that only looks at the front would trip.
            ledger.encumber(1, 31, "ancestral", "both registries");
            ledger.encumber(60, 60, "external-registry", "external registry");
            ledger.encumber(62, 63, "external-registry", "external registry");
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

        let on_a = edge_a.issue("kernel-a", 16).expect("room on a");
        let on_b = edge_b.issue("kernel-a", 16).expect("room on b");
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
                Some("kernel-a".to_string())
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
}

mod deed_proper {


    use isthmus::deed::{Flaw, Ledger, Refused, Standing};
    use isthmus::layout::Layout;
    use isthmus::Verdict;

    // ===================================================================
    // Maturation: a claim growing into a deed, and only its own claim
    // ===================================================================

    #[test]
    fn a_claim_matures_into_its_claimants_deed_and_nobody_elses() {
        let mut edge = Ledger::new(Layout::founding());
        edge.encumber(55, 56, "mesh-a", "external registry, read today");
        edge.encumber(32, 54, "external-registry", "external registries");

        // The admitted arm: the claimant matures their own claim.
        let deed = edge.mature("mesh-a", 55, 56).expect("the claim is theirs");
        assert_eq!((deed.low(), deed.high()), (55, 56));
        assert_eq!(edge.standing_of(55), Standing::Deeded { holder: "mesh-a".into() });
        assert_eq!(edge.standing_of(56), Standing::Deeded { holder: "mesh-a".into() });
        edge.well_formed().expect("maturation is lawful history");

        // Potentials now exist over the matured ground — which is the whole
        // point: the gauge-invariant reading needs a deed to read against.
        let (holder, offsets) = edge.potential_at(&[56]).expect("deeded ground");
        assert_eq!(holder, "mesh-a");
        assert_eq!(offsets, vec![1]);

        // The refused arms, each named:
        // somebody else's claim is not yours to mature —
        let mut thief = Ledger::new(Layout::founding());
        thief.encumber(55, 56, "mesh-a", "their claim");
        assert!(matches!(
            thief.mature("kernel-a", 55, 56),
            Err(Refused::NotYourClaim { .. })
        ));
        // open ground is not a claim at all —
        assert!(matches!(
            thief.mature("mesh-a", 200, 207),
            Err(Refused::NotYourClaim { .. })
        ));
        // and H1 is not suspended for maturation.
        let mut greedy = Ledger::new(Layout::founding());
        greedy.encumber(55, 56, "mesh-a", "claim");
        greedy.issue("mesh-a", 8).expect("room");
        assert!(matches!(
            greedy.mature("mesh-a", 55, 56),
            Err(Refused::AlreadyHeld { .. })
        ));
    }

    /// The checker agrees: an issue over the holder's OWN encumbrance is
    /// well-formed history; over anybody else's it stays an Overlap.
    #[test]
    fn well_formed_admits_maturation_and_still_refuses_squatting() {
        let mut matured = Ledger::new(Layout::founding());
        matured.encumber(55, 56, "mesh-a", "claim");
        matured.record(isthmus::deed::Act::Issue {
            holder: "mesh-a".into(),
            low: 55,
            high: 56,
        });
        matured.well_formed().expect("a matured claim is lawful");

        let mut squatted = Ledger::new(Layout::founding());
        squatted.encumber(55, 56, "mesh-a", "claim");
        squatted.record(isthmus::deed::Act::Issue {
            holder: "squatter".into(),
            low: 55,
            high: 56,
        });
        assert!(matches!(
            squatted.well_formed(),
            Err(Flaw::Overlap { at: 1, .. })
        ));
    }

    // ===================================================================
    // Facets: the boundary a closure proof walks
    // ===================================================================

    #[test]
    fn facets_bound_the_box_with_alternating_orientation() {
        let mut edge = Ledger::new(Layout::with_tag_width(1));
        edge.open_axis("revision", 7);
        let deed = edge.issue_box("H", &[4, 3]).expect("room");

        let facets = deed.facets();
        assert_eq!(facets.len(), 4, "2n facets for n axes");

        for axis in 0..2usize {
            let pair: Vec<_> = facets.iter().filter(|f| f.axis == axis).collect();
            assert_eq!(pair.len(), 2);
            let total: i8 = pair.iter().map(|f| f.orientation).sum();
            assert_eq!(total, 0, "opposite faces must cancel — ∂∂ = 0");

            for facet in pair {
                // The face is flat on its axis and full on every other.
                let (low, high) = facet.region[axis];
                assert_eq!(low, high, "a facet is flat on its own axis");
                for other in 0..2usize {
                    if other != axis {
                        assert_eq!(facet.region[other], deed.region[other]);
                    }
                }
                // And it lies on the deed's boundary, not inside it.
                assert!(
                    low == deed.region[axis].0 || low == deed.region[axis].1,
                    "a facet strayed off the boundary"
                );
            }
        }
    }

    // ===================================================================
    // Recognised: the fifth verdict, and its lawful degradation
    // ===================================================================

    #[test]
    fn the_seam_recognises_deeded_tags_and_degrades_without_a_court() {
        let mut court = Ledger::new(Layout::founding());
        court.encumber(55, 56, "mesh-a", "claim");
        court.mature("mesh-a", 55, 56).expect("the deed proper");

        let mine = |tag: u64| (64..=79).contains(&tag);
        let bound = 1 << 16;

        // A record under mesh-a's deed, framed whole.
        let mut wire = Vec::new();
        isthmus::frame::put_frame(&Layout::founding(), 56, &[0xAA, 0xBB], &mut wire)
            .expect("fits");

        // With the court: RECOGNISED — shape confirmed, payload opaque,
        // the whole record named for unfractured delivery.
        assert_eq!(
            isthmus::verdict(&Layout::founding(), &wire, bound, mine, Some(&court)),
            Verdict::Recognised { tag: 56, whole: 7 }
        );

        // Without the court: the SAME bytes lawfully degrade to Skip —
        // forwarded instead of delivered. Economy lost, correctness never.
        assert_eq!(
            isthmus::verdict(&Layout::founding(), &wire, bound, mine, None),
            Verdict::Skip { tag: 56, whole: 7 }
        );

        // The neighbours, so Recognised sits strictly BETWEEN them:
        // a tag nobody holds skips even with the court —
        let mut unknown = Vec::new();
        isthmus::frame::put_frame(&Layout::founding(), 200, &[0xCC], &mut unknown)
            .expect("fits");
        assert!(matches!(
            isthmus::verdict(&Layout::founding(), &unknown, bound, mine, Some(&court)),
            Verdict::Skip { tag: 200, .. }
        ));
        // one this reader owns accepts, court or none —
        let mut owned = Vec::new();
        isthmus::frame::put_frame(&Layout::founding(), 64, &[], &mut owned).expect("fits");
        assert_eq!(
            isthmus::verdict(&Layout::founding(), &owned, bound, mine, Some(&court)),
            Verdict::Accept
        );
        // and the shape gates still precede everything: torn stays refused,
        // short stays waiting, court or none.
        let mut torn = vec![56u8];
        torn.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            isthmus::verdict(&Layout::founding(), &torn, bound, mine, Some(&court)),
            Verdict::Refuse(_)
        ));
        assert_eq!(
            isthmus::verdict(&Layout::founding(), &wire[..3], bound, mine, Some(&court)),
            Verdict::Wait
        );
    }
}

mod derivation_laws {


    use isthmus::deed::{Deed, Ledger};
    use isthmus::layout::Layout;

    /// Record kinds from the protocols actually in this environment, plus
    /// enough others to exercise a vocabulary.
    const KINDS: [&str; 12] = [
        "mesh.head",
        "mesh.refusal",
        "hello",
        "relation",
        "manifold",
        "closures",
        "witness",
        "receipt",
        "aperture",
        "grammar",
        "lesson",
        "chronicle.segment",
    ];

    fn deed_on(edge: &mut Ledger, holder: &str, width: u128) -> Deed {
        edge.issue(holder, width).expect("room for the deed")
    }

    // ===================================================================
    // D1 — determinism
    // ===================================================================

    /// **A function of the kind and the deed alone.** Called twice, called
    /// on a clone, called after unrelated acts land — the same answer.
    #[test]
    fn d1_the_same_kind_on_the_same_deed_is_always_the_same_tag() {
        let mut edge = Ledger::new(Layout::founding());
        edge.encumber(1, 31, "ancestral", "these laws");
        let deed = deed_on(&mut edge, "peer", 16);

        for kind in KINDS {
            let once = deed.tag_for(kind);
            assert!(once.is_some(), "{kind} derived nothing on a 16-wide deed");
            for _ in 0..8 {
                assert_eq!(deed.tag_for(kind), once, "{kind} moved between calls");
            }
            assert_eq!(deed.clone().tag_for(kind), once, "{kind} moved on a clone");
        }

        // Unrelated history does not move it: the derivation reads the
        // deed, not the ledger.
        let before: Vec<_> = KINDS.iter().map(|k| deed.tag_for(k)).collect();
        edge.encumber(200, 210, "somebody", "later");
        let _ = edge.issue("another", 8);
        let after: Vec<_> = KINDS.iter().map(|k| deed.tag_for(k)).collect();
        assert_eq!(before, after, "later acts moved a derived tag");
    }

    /// **Every derived tag is inside its own deed.** This is the property
    /// that makes cross-holder collision structurally impossible rather
    /// than merely unlikely.
    #[test]
    fn d1_a_derived_tag_never_leaves_its_deed() {
        let mut edge = Ledger::new(Layout::founding());
        edge.encumber(1, 31, "ancestral", "these laws");

        for width in [1u128, 2, 3, 16, 48, 100] {
            let mut fresh = Ledger::new(Layout::founding());
            fresh.encumber(1, 31, "ancestral", "these laws");
            let deed = deed_on(&mut fresh, "peer", width);
            for kind in KINDS {
                let tag = deed.tag_for(kind).expect("a deed with width derives");
                assert!(
                    tag >= deed.low() && tag <= deed.high(),
                    "{kind} derived {tag}, outside {}-{} on a {width}-wide deed",
                    deed.low(),
                    deed.high(),
                );
            }
        }
        let _ = edge;
    }

    // ===================================================================
    // D2 — the collision that started this cannot happen
    // ===================================================================

    /// **Two holders cannot derive the same tag**, whatever they call their
    /// records — because deeds are disjoint and a derivation never leaves
    /// its deed.
    ///
    /// This is the answer to `MESH_HEAD_TAG = 64`. Both parties may keep
    /// the name `head`; they cannot keep the number, and they no longer
    /// need to agree on one.
    #[test]
    fn d2_two_holders_deriving_the_same_names_never_collide() {
        let mut edge = Ledger::new(Layout::founding());
        edge.encumber(1, 31, "ancestral", "these laws");
        let mine = deed_on(&mut edge, "isthmus", 16);
        let theirs = deed_on(&mut edge, "ns-mesh", 16);

        // The worst case on purpose: identical vocabularies.
        for kind in KINDS {
            let here = mine.tag_for(kind).expect("derives");
            let there = theirs.tag_for(kind).expect("derives");
            assert_ne!(
                here, there,
                "{kind} derived {here} for both holders — the deeds overlap",
            );
        }

        // And the same name means the same THING on both, because the
        // offset is what identifies it. The number is the frame; the offset
        // is the invariant.
        for kind in KINDS {
            assert_eq!(
                mine.offset_for(kind),
                theirs.offset_for(kind),
                "{kind} has a different offset on two equal-width deeds — \
                 then the same record kind is not the same record kind",
            );
        }
    }

    /// **The gate fires.** If the deeds overlapped, `d2` would be asserting
    /// something false — so overlapping regions must produce a shared tag,
    /// proving the disjointness is what the law rests on.
    #[test]
    fn d2_overlapping_regions_would_collide_which_is_why_deeds_are_disjoint() {
        let overlapping = |low: u64, high: u64| Deed {
            holder: "constructed".to_owned(),
            region: vec![(low, high)],
            live: true,
            within: None,
        };
        let a = overlapping(64, 79);
        let b = overlapping(64, 79);

        let mut shared = 0usize;
        for kind in KINDS {
            if a.tag_for(kind) == b.tag_for(kind) {
                shared += 1;
            }
        }
        assert_eq!(
            shared,
            KINDS.len(),
            "two identical regions derived different tags — the derivation \
             is not a function of the region",
        );
    }

    // ===================================================================
    // D3 — stable under growth
    // ===================================================================

    /// **Declaring a new record kind never moves an existing one.**
    ///
    /// An assignment that probed for a free slot would pack tighter and
    /// would move tags when the vocabulary grew — a wire break dressed as
    /// an optimisation. This derivation does not depend on what else is in
    /// the vocabulary, so it cannot.
    #[test]
    fn d3_growing_the_vocabulary_moves_nothing() {
        let mut edge = Ledger::new(Layout::founding());
        edge.encumber(1, 31, "ancestral", "these laws");
        let deed = deed_on(&mut edge, "peer", 48);

        let base: Vec<(String, u64)> = KINDS
            .iter()
            .map(|k| ((*k).to_owned(), deed.tag_for(k).expect("derives")))
            .collect();

        // Add kinds one at a time; nothing already there may move.
        let mut vocabulary: Vec<&str> = KINDS.to_vec();
        for extra in ["vent", "docket", "uplink", "sublet", "anchor", "strike"] {
            vocabulary.push(extra);
            for (kind, was) in &base {
                assert_eq!(
                    deed.tag_for(kind),
                    Some(*was),
                    "{kind} moved from {was} when {extra} was declared",
                );
            }
        }
        assert_eq!(vocabulary.len(), KINDS.len() + 6);
    }

    /// **A collision inside one vocabulary is reported, not resolved.**
    ///
    /// The cost of `d3`: two names can land on one tag. The author renames
    /// one. Resolving it silently would trade a visible refusal for a
    /// property nobody could rely on.
    #[test]
    fn d3_a_vocabulary_collision_is_named() {
        let mut edge = Ledger::new(Layout::founding());
        edge.encumber(1, 31, "ancestral", "these laws");

        // A deed too narrow for the vocabulary forces collisions by
        // pigeonhole: 12 kinds cannot have 12 distinct tags in 4 slots.
        let narrow = deed_on(&mut edge, "cramped", 4);
        let found = narrow.collisions(&KINDS);
        assert!(
            !found.is_empty(),
            "12 kinds in a 4-wide deed reported no collision — pigeonhole \
             says otherwise, so the report is broken",
        );
        for (one, other, tag) in &found {
            assert_eq!(narrow.tag_for(one), Some(*tag));
            assert_eq!(narrow.tag_for(other), Some(*tag));
            assert_ne!(one, other, "a kind collided with itself");
        }

        // Order-invariant: the same vocabulary shuffled reports the same
        // pairs.
        let mut shuffled: Vec<&str> = KINDS.to_vec();
        shuffled.reverse();
        assert_eq!(
            narrow.collisions(&shuffled),
            found,
            "the collision report depends on declaration order",
        );

        // And a deed with room reports none for this vocabulary — so the
        // report is not simply always non-empty.
        let roomy = deed_on(&mut edge, "roomy", 200);
        assert!(
            roomy.collisions(&KINDS).is_empty(),
            "a 200-wide deed collided on 12 names: {:?}",
            roomy.collisions(&KINDS),
        );
    }

    // ===================================================================
    // D4 — the offset is what crosses
    // ===================================================================

    /// **Two edges give a holder different numbers and the same offsets.**
    ///
    /// The absolute tag is a fact about one edge; the offset is the record
    /// kind's identity. This is `potential_at`'s rule — the box origin is
    /// the frame, the offset is gauge-invariant — applied to vocabulary.
    #[test]
    fn d4_the_offset_survives_a_change_of_edge() {
        let mut edge_a = Ledger::new(Layout::founding());
        edge_a.encumber(1, 31, "ancestral", "these laws");
        let here = deed_on(&mut edge_a, "peer", 32);

        let mut edge_b = Ledger::new(Layout::founding());
        edge_b.encumber(1, 120, "a busier neighbour", "their advert");
        let there = deed_on(&mut edge_b, "peer", 32);

        assert_ne!(here.low(), there.low(), "the edges agree — nothing to test");

        for kind in KINDS {
            assert_eq!(
                here.offset_for(kind),
                there.offset_for(kind),
                "{kind} changed identity between edges",
            );
            assert_ne!(
                here.tag_for(kind),
                there.tag_for(kind),
                "{kind} got the same absolute tag on two different edges",
            );
            // And the absolute tag is exactly origin + offset on each.
            assert_eq!(
                here.tag_for(kind),
                Some(here.low() + here.offset_for(kind).expect("derives")),
            );
            assert_eq!(
                there.tag_for(kind),
                Some(there.low() + there.offset_for(kind).expect("derives")),
            );
        }
    }

    /// **A deed with no width derives nothing**, rather than deriving zero.
    ///
    /// Tag 0 is the void — a zero-filled buffer decodes to it — so a
    /// derivation that fell back to zero would put every record from an
    /// empty deed on the one tag that must name nothing.
    #[test]
    fn d4_an_empty_deed_derives_nothing() {
        let empty = Deed {
            holder: "nobody".to_owned(),
            region: Vec::new(),
            live: true,
            within: None,
        };
        for kind in KINDS {
            assert_eq!(empty.tag_for(kind), None, "{kind} derived from no region");
            assert_eq!(empty.offset_for(kind), None);
        }
        assert!(empty.collisions(&KINDS).is_empty());
    }
}

mod replay_laws {


    use isthmus::deed::{chain, Act, Flaw, Ledger};
    use isthmus::layout::Layout;

    /// An edge with an observation on it, so the acts have context.
    fn base() -> Ledger {
        let mut ledger = Ledger::new(Layout::founding()).under("probe");
        ledger.encumber(1, 31, "ancestral", "for these laws");
        ledger
    }

    /// One act of every kind, with the chain tag it encodes as.
    ///
    /// The tag travels with the act so `r2` can check this list against
    /// the codec — an act shape nobody listed here is an act shape nobody
    /// tested.
    fn every_act() -> Vec<(u64, Act)> {
        vec![
            (
                chain::ENCUMBER,
                Act::Encumber {
                    low: 40,
                    high: 47,
                    by: "north".to_owned(),
                    witnessed: "these laws".to_owned(),
                },
            ),
            (
                chain::ISSUE,
                Act::Issue {
                    holder: "alpha".to_owned(),
                    low: 40,
                    high: 47,
                },
            ),
            (
                chain::RETIRE,
                Act::Retire {
                    holder: "alpha".to_owned(),
                },
            ),
            (
                chain::OPEN,
                Act::Open {
                    axis: "revision".to_owned(),
                    max: 7,
                },
            ),
            (
                chain::ENCUMBER_BOX,
                Act::EncumberBox {
                    region: vec![(40, 47)],
                    by: "north".to_owned(),
                    witnessed: "these laws".to_owned(),
                },
            ),
            (
                chain::ISSUE_BOX,
                Act::IssueBox {
                    holder: "beta".to_owned(),
                    region: vec![(40, 47)],
                },
            ),
            (
                chain::CEDE,
                // A slab of the prelude's estate (48-55), flush against
                // its top edge. An interior cut leaves two pieces and a
                // cut outside it is not the owner's to give.
                Act::Cede {
                    from: "owner".to_owned(),
                    to: "buyer".to_owned(),
                    region: vec![(52, 55)],
                },
            ),
            (
                chain::SUBLET,
                Act::Sublet {
                    from: "owner".to_owned(),
                    to: "moon".to_owned(),
                    region: vec![(50, 53)],
                },
            ),
            (
                chain::ANCHOR,
                Act::Anchor {
                    chain: "south".to_owned(),
                    height: 9,
                    digest: vec![1, 2, 3],
                    witnessed: "these laws".to_owned(),
                },
            ),
            (
                chain::BIND,
                Act::Bind {
                    holder: "beta".to_owned(),
                    scheme: 0x01,
                    key: vec![7; 32],
                    from_epoch: 0,
                    until_epoch: 9,
                },
            ),
            (
                chain::DECLARE,
                Act::Declare {
                    holder: "beta".to_owned(),
                    tag: 42,
                    definition: vec![3, 0, 0],
                },
            ),
            (
                chain::CERTIFY,
                Act::Certify {
                    holder: "beta".to_owned(),
                    fingerprint: [9u8; 32],
                },
            ),
            (
                chain::ESCROW,
                Act::Escrow {
                    holder: "beta".to_owned(),
                    amount: 1_000_000_000_000_000_000_000u128,
                },
            ),
            (
                chain::RELEASE,
                Act::Release {
                    holder: "beta".to_owned(),
                },
            ),
            (
                chain::SLASH,
                Act::Slash {
                    holder: "beta".to_owned(),
                    amount: 7,
                },
            ),
        ]
    }

    /// Which tag an act encodes as — **an exhaustive match, no wildcard**.
    ///
    /// Adding a tenth act stops this file compiling until somebody decides
    /// what its tag is, which a runtime assertion cannot do.
    fn tag_of(act: &Act) -> u64 {
        match act {
            Act::Encumber { .. } => chain::ENCUMBER,
            Act::Issue { .. } => chain::ISSUE,
            Act::Retire { .. } => chain::RETIRE,
            Act::Open { .. } => chain::OPEN,
            Act::EncumberBox { .. } => chain::ENCUMBER_BOX,
            Act::IssueBox { .. } => chain::ISSUE_BOX,
            Act::Cede { .. } => chain::CEDE,
            Act::Sublet { .. } => chain::SUBLET,
            Act::Anchor { .. } => chain::ANCHOR,
            Act::Bind { .. } => chain::BIND,
            Act::Declare { .. } => chain::DECLARE,
            Act::Certify { .. } => chain::CERTIFY,
            Act::Escrow { .. } => chain::ESCROW,
            Act::Release { .. } => chain::RELEASE,
            Act::Slash { .. } => chain::SLASH,
        }
    }

    /// Everything a fold says about an edge: live deeds, open space, the
    /// gaps, **the axes with their extents**, and the standing of every
    /// tag.
    ///
    /// The axes were `volume()` and a count. `volume()` is struck — a
    /// product across axes calls `[2, 8]` and `[4, 4]` the same space — and
    /// the axes themselves say strictly more than either.
    type Reading = (
        usize,
        u128,
        Vec<(u64, u64)>,
        Vec<isthmus::deed::Axis>,
        Vec<String>,
    );

    /// The whole reading, so "changed nothing" is a claim about all of it
    /// and not about whichever accessor was convenient.
    fn reading(ledger: &Ledger) -> Reading {
        (
            ledger.deeds().iter().filter(|d| d.live).count(),
            ledger.open(),
            ledger.gaps(),
            ledger.axes(),
            (0u64..=255)
                .map(|tag| format!("{:?}", ledger.standing_of(tag)))
                .collect(),
        )
    }

    // ===================================================================
    // R1 — every effect is idempotent, or refused
    // ===================================================================

    /// **The whole of `IS-2` §6.1 for the chain.** For every act: applying
    /// it twice either reads identically to applying it once, or the
    /// second application makes the chain ill-formed so the effect never
    /// lands.
    ///
    /// Both arms are lawful and they are *different* remedies. What is not
    /// lawful is the third case — an effect that lands twice and is
    /// accepted — which is what `Open` did until this law was written:
    /// replaying it doubled the axis count and multiplied the volume, and
    /// `well_formed` said nothing.
    #[test]
    fn r1_every_act_is_idempotent_under_replay_or_refused() {
        let mut idempotent = 0usize;
        let mut refused = 0usize;

        for (tag, act) in every_act() {
            // `Cede` needs an owner to convey from; giving every act the
            // same prelude keeps the law one law.
            let mut ground = base();
            ground.record(Act::Issue {
                holder: "owner".to_owned(),
                low: 48,
                high: 55,
            });

            let mut once = ground.clone();
            once.record(act.clone());
            let mut twice = ground.clone();
            twice.record(act.clone());
            twice.record(act.clone());

            assert!(
                once.well_formed().is_ok(),
                "tag {tag}: applying {act:?} ONCE is already ill-formed, so \
                 this row tests nothing about replay",
            );

            if twice.well_formed().is_err() {
                refused += 1;
                continue;
            }
            assert_eq!(
                reading(&once),
                reading(&twice),
                "tag {tag}: {act:?} replayed changed the fold AND was \
                 accepted — an effect that lands twice",
            );
            idempotent += 1;
        }

        // Both remedies must be exercised, or this law is passing because
        // one of its two arms is unreachable.
        assert!(idempotent > 0, "no act was idempotent — the law is vacuous");
        assert!(refused > 0, "no act was refused — the law is vacuous");
    }

    /// **The coverage gate.** Every chain tag has a row above.
    ///
    /// An act added to the codec without a row here leaves `r1` silently
    /// narrower than the thing it claims to be about — the "tests grow
    /// unbounded and unwitnessed" failure, in the one place where growth is
    /// the whole design.
    ///
    /// ## The first version of this gate did not fire, and it was measured
    ///
    /// It read the codec's tag set as `1..=chain::ANCHOR` — keying "the
    /// highest tag" off one named constant. When `Act::Sublet` landed as
    /// tag 9, `ANCHOR` was still 8, so the gate compared `1..=8` against
    /// its own eight rows and **agreed with itself** while a ninth act sat
    /// untested. A gate that derives its expectation from the thing it is
    /// checking is a gate that cannot fail.
    ///
    /// Its second half missed it too, for a different reason. It asserted
    /// that tag `ANCHOR + 1` does not decode — but it built the probe from
    /// a `Retire` payload, so tag 9 refused because a sublet cannot be
    /// parsed out of one string, not because tag 9 is unknown. **The right
    /// answer for the wrong reason still passes.** The refusal's *reason*
    /// is now checked, which is what makes the difference visible.
    #[test]
    fn r2_every_chain_tag_has_a_row() {
        let rows = every_act();

        // Each row's act really does encode as the tag it is filed under.
        for (tag, act) in &rows {
            assert_eq!(
                tag_of(act),
                *tag,
                "{act:?} is filed under tag {tag} and encodes as {}",
                tag_of(act),
            );
            let bytes = chain::encode(std::slice::from_ref(act));
            assert_eq!(
                bytes.first().map(|b| u64::from(*b)),
                Some(*tag),
                "{act:?} did not frame under tag {tag}",
            );
        }

        // The covered tags are contiguous from 1.
        let covered: std::collections::BTreeSet<u64> = rows.iter().map(|(tag, _)| *tag).collect();
        let highest = covered.iter().copied().max().unwrap_or(0);
        assert_eq!(
            covered,
            (1..=highest).collect::<std::collections::BTreeSet<u64>>(),
            "the covered tags have a gap",
        );

        // And the codec knows NOTHING above them — checked by the reason
        // it refuses, not merely that it does. An empty value isolates the
        // tag: every real act needs at least one field, so a known tag
        // refuses for truncation and an unknown one refuses for its tag.
        let mut probe = Vec::new();
        let layout = Layout::founding();
        isthmus::frame::put_frame(
            &layout,
            highest + 1,
            &[],
            &mut probe,
        )
        .expect("a one-byte tag frames");

        match chain::decode(&probe) {
            Err(isthmus::frame::Malformed::UnexpectedTag { found, .. }) => {
                assert_eq!(found, highest + 1, "the codec named a different tag");
            }
            Err(other) => panic!(
                "tag {} refused as {other:?} rather than as an unknown act — \
                 the codec HAS this act and no row here covers it",
                highest + 1,
            ),
            Ok(acts) => panic!(
                "tag {} decoded to {acts:?} — an act nobody tested for replay",
                highest + 1,
            ),
        }
    }

    // ===================================================================
    // R3 — the identity `Open` dedups on, and its limit
    // ===================================================================

    /// **A replayed `Open` opens nothing; a contradictory one is named.**
    ///
    /// `IS-2` §6.1 offers two ways to be safe and `Open` takes the second:
    /// it carries `axis`, and that name is the identity the fold dedups
    /// on. But dedup-by-name has an edge the ruling does not cover — the
    /// same name with a *different* extent is not a replay, and folding it
    /// to the first silently discards a declaration.
    #[test]
    fn r3_an_axis_opens_once_and_a_contradiction_is_refused() {
        // Replay: three times, one axis, and every reading identical.
        let mut once = base();
        once.open_axis("revision", 7);
        let mut thrice = base();
        for _ in 0..3 {
            thrice.open_axis("revision", 7);
        }
        assert_eq!(once.axes().len(), 2, "the axis did not open");
        assert_eq!(thrice.axes().len(), 2, "a replayed open opened an axis");
        assert_eq!(reading(&once), reading(&thrice), "a replayed open moved the fold");
        assert!(thrice.well_formed().is_ok(), "a replay is not a flaw");

        // Distinct names still open distinct axes — the dedup is on the
        // identity, not on the act kind.
        let mut two = base();
        two.open_axis("revision", 7);
        two.open_axis("epoch", 3);
        assert_eq!(two.axes().len(), 3, "a second name did not open an axis");
        // Per axis: the second NAME added a direction, and the extents are
        // the ones declared. A product would have called this the same as
        // widening one axis fourfold.
        assert_eq!(two.axes().len(), once.axes().len() + 1, "no direction was added");
        assert_eq!(two.axes()[2].name, "epoch");
        assert_eq!(two.axes()[2].max, 3, "the new axis's extent");

        // Contradiction: same name, different extent. Not a replay.
        let mut clash = base();
        clash.open_axis("revision", 7);
        clash.open_axis("revision", 15);
        assert_eq!(
            clash.well_formed(),
            Err(Flaw::AxisRedeclared {
                axis: "revision".to_owned(),
                at: 2,
            }),
            "one axis with two extents was accepted",
        );
        // And the fold kept the first, which is exactly why it is a flaw
        // rather than a merge: the second declaration is gone.
        assert_eq!(clash.axes().get(1).map(|a| a.max), Some(7));
    }

    /// **The acts an ancestor actually recorded are unaffected.**
    ///
    /// The founding chain carries no `Open`, so this fix could not have
    /// changed the authority's fold — checked here rather than asserted in
    /// a commit message, because "it cannot have broken anything" is the
    /// claim most worth measuring.
    #[test]
    fn r3_a_chain_without_axes_reads_the_same_either_way() {
        let mut ledger = base();
        ledger.record(Act::Issue {
            holder: "isthmus".to_owned(),
            low: 64,
            high: 79,
        });
        ledger.record(Act::Issue {
            holder: "assay".to_owned(),
            low: 80,
            high: 127,
        });
        assert_eq!(ledger.axes().len(), 1, "a one-line chain grew an axis");
        assert!(ledger.well_formed().is_ok());
        assert_eq!(ledger.deeds().iter().filter(|d| d.live).count(), 2);
    }
}

mod sphere_laws {


    use std::cmp::Ordering;

    use isthmus::deed::{chain, Act, Ledger};
    use isthmus::hello::{Hello, Uplink};
    use isthmus::sphere::{confirms, standoffs, Frontier, Precedence};
    use isthmus::layout::Layout;

    /// A digest, for tests only: FNV-1a, eight bytes.
    ///
    /// The library names no digest function on purpose — [`confirms`] takes
    /// one in. This one is deterministic and that is all a law needs; it is
    /// **not** a recommendation, and nothing outside this file uses it.
    fn fnv(bytes: &[u8]) -> Vec<u8> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash.to_le_bytes().to_vec()
    }

    /// A named chain with one holder on `width` tags from the bottom.
    fn chain_with(name: &str, holder: &str, width: u128) -> Ledger {
        let mut ledger = Ledger::new(Layout::founding()).under(name);
        ledger.issue(holder, width).expect("a bare edge deeds");
        ledger
    }

    /// Deterministic frontiers. Enumerated, not sampled — a pass means
    /// *none of these*, which is a claim, rather than *these draws*, which
    /// is a report.
    fn frontiers() -> Vec<Frontier> {
        let mut out = vec![Frontier::new()];
        for spec in [
            vec![("north", 1u64)],
            vec![("north", 7)],
            vec![("south", 1)],
            vec![("north", 3), ("south", 3)],
            vec![("north", 9), ("south", 1)],
            vec![("north", 1), ("south", 9)],
            vec![("north", 2), ("south", 2), ("east", 2)],
            vec![("east", 40)],
            vec![("north", 0), ("south", 0)],
        ] {
            let mut frontier = Frontier::new();
            for (name, height) in spec {
                frontier.observe(name, height);
            }
            out.push(frontier);
        }
        out
    }

    // ===================================================================
    // S1 — the join is a hypersphere envelope
    // ===================================================================

    /// **Idempotent, commutative, associative.** These three are what let
    /// two parties merge what they know without first agreeing on an order
    /// to merge it in — which is the whole reason the merge is a max and
    /// not a "latest".
    /// **One representation per frontier**, so the derived equality and
    /// [`Frontier::compare`] answering `Equal` are the same relation. Two
    /// notions of sameness in one type is one too many, and this law is the
    /// one that found the second.
    #[test]
    fn s1_equality_and_the_order_agree_about_sameness() {
        for a in frontiers() {
            for b in frontiers() {
                assert_eq!(
                    a == b,
                    a.compare(&b) == Some(Ordering::Equal),
                    "the derived equality and the order disagree: {a:?} vs {b:?}",
                );
            }
        }

        // Observing nothing is observing nothing, however it is spelled.
        let mut zeroed = Frontier::new();
        zeroed.observe("north", 0);
        zeroed.observe("south", 0);
        assert_eq!(zeroed, Frontier::new(), "a stored zero is not an absence");
        assert!(zeroed.chains().is_empty(), "a chain seen zero acts of was named");
    }

    #[test]
    fn s1_the_join_is_a_hypersphere_envelope() {
        for a in frontiers() {
            assert_eq!(a.join(&a), a, "join is not idempotent");
            assert_eq!(a.join(&Frontier::new()), a, "the empty frontier is not the identity");
            for b in frontiers() {
                assert_eq!(a.join(&b), b.join(&a), "join is not commutative");
                for c in frontiers() {
                    assert_eq!(
                        a.join(&b).join(&c),
                        a.join(&b.join(&c)),
                        "join is not associative",
                    );
                }
            }
        }
    }

    /// **The join is the least upper bound**, which is what makes it the
    /// merge: it is above both, and it is above them by exactly the
    /// observations they had.
    #[test]
    fn s1_the_join_is_an_upper_bound_and_agrees_with_the_order() {
        for a in frontiers() {
            for b in frontiers() {
                let joined = a.join(&b);
                assert!(
                    matches!(joined.compare(&a), Some(Ordering::Greater | Ordering::Equal)),
                    "the join is not above its left argument",
                );
                assert!(
                    matches!(joined.compare(&b), Some(Ordering::Greater | Ordering::Equal)),
                    "the join is not above its right argument",
                );
                // a <= b exactly when joining a into b changes nothing.
                let below = matches!(a.compare(&b), Some(Ordering::Less | Ordering::Equal));
                assert_eq!(
                    below,
                    joined == b,
                    "the order and the join disagree about whether a precedes b",
                );
            }
        }
    }

    // ===================================================================
    // S2 — the order is PARTIAL, and the gap is the point
    // ===================================================================

    /// **Reflexive and antisymmetric**, and `None` is never symmetric-broken:
    /// if `a` is concurrent with `b` then `b` is concurrent with `a`.
    #[test]
    fn s2_the_order_is_a_partial_order() {
        for a in frontiers() {
            assert_eq!(a.compare(&a), Some(Ordering::Equal), "not reflexive");
            for b in frontiers() {
                match (a.compare(&b), b.compare(&a)) {
                    (Some(Ordering::Less), Some(Ordering::Greater))
                    | (Some(Ordering::Greater), Some(Ordering::Less))
                    | (Some(Ordering::Equal), Some(Ordering::Equal))
                    | (None, None) => {}
                    (here, there) => panic!("compare is not antisymmetric: {here:?} vs {there:?}"),
                }
                assert_eq!(
                    a.concurrent_with(&b),
                    a.compare(&b).is_none(),
                    "concurrent_with and compare disagree",
                );
            }
        }
    }

    /// **The gate fires both ways.** A pair of frontiers that each saw
    /// something the other did not is concurrent; comparable pairs exist in
    /// the same enumeration, so this is not a suite that can only produce
    /// one verdict.
    #[test]
    fn s2_concurrency_is_reachable_and_so_is_comparability() {
        let mut ahead = Frontier::new();
        ahead.observe("north", 9);
        ahead.observe("south", 1);
        let mut behind = Frontier::new();
        behind.observe("north", 1);
        behind.observe("south", 9);

        assert_eq!(ahead.compare(&behind), None, "these must be concurrent");

        // And the same pair becomes ordered the moment one catches up —
        // the refusal is about the observations, not about the type.
        let caught_up = ahead.join(&behind);
        assert_eq!(
            caught_up.compare(&behind),
            Some(Ordering::Greater),
            "after joining, one must precede",
        );

        let mut any_ordered = false;
        let mut any_concurrent = false;
        for a in frontiers() {
            for b in frontiers() {
                match a.compare(&b) {
                    Some(_) => any_ordered = true,
                    None => any_concurrent = true,
                }
            }
        }
        assert!(
            any_ordered && any_concurrent,
            "the enumeration produces only one verdict, so it proves nothing",
        );
    }

    /// An **unnamed chain omits itself** from its own frontier — it can
    /// anchor others, and nobody can anchor it. Downstream, not upstream.
    #[test]
    fn s2_an_unnamed_chain_has_no_position_in_the_order() {
        let mut anonymous = Ledger::new(Layout::founding());
        anonymous.issue("alpha", 8).expect("a bare edge deeds");
        anonymous.anchor("north", 3, &fnv(b""), "read from disk");

        let frontier = anonymous.frontier();
        assert_eq!(frontier.chains(), vec!["north"], "an unnamed chain named itself");
        assert_eq!(frontier.height_of("north"), 3, "the vertical was not recorded");

        // Named, the same acts report a position.
        let named = anonymous.under("here");
        assert_eq!(named.frontier().height_of("here"), 2, "a named chain's own height");
    }

    // ===================================================================
    // S3 — a vertical grants nothing
    // ===================================================================

    /// **Anchoring changes no fold on this edge.** Observing a stranger
    /// must not enlarge, shrink, or move an estate — otherwise every party
    /// could grow by looking at things.
    #[test]
    fn s3_an_anchor_moves_no_ground() {
        for width in [1u128, 8, 64, 200] {
            let before = chain_with("north", "alpha", width);
            let mut after = before.clone();
            after.anchor("south", 41, &fnv(b"whatever"), "read at /opt/mirrors");
            after.anchor("east", 0, &[], "an empty chain, observed");

            assert_eq!(before.deeds().len(), after.deeds().len(), "deeds changed");
            assert_eq!(before.open(), after.open(), "open space changed");
            assert_eq!(before.gaps(), after.gaps(), "the gaps moved");
            assert_eq!(before.axes(), after.axes(), "an axis extent changed");
            assert_eq!(before.axes().len(), after.axes().len(), "an axis appeared");
            for tag in 0u64..=255 {
                assert_eq!(
                    before.standing_of(tag),
                    after.standing_of(tag),
                    "the standing of tag {tag} changed under an anchor",
                );
            }
            // And a history that was well-formed stays well-formed: my
            // chain must not become invalid because of what a stranger
            // appended to theirs.
            assert_eq!(
                before.well_formed().is_ok(),
                after.well_formed().is_ok(),
                "an anchor changed well-formedness",
            );
        }
    }

    // ===================================================================
    // S4 — the vertical survives the wire
    // ===================================================================

    /// Round trip, and **an unknown act still refuses**. Tag 8 is additive:
    /// a reader that does not have it must refuse the chain rather than
    /// fold a different history and report it as this one.
    #[test]
    fn s4_the_anchor_round_trips_and_an_unknown_act_still_refuses() {
        for digest in [vec![], vec![0u8], fnv(b"a prefix"), vec![0xff; 64]] {
            for height in [0u64, 1, 13, u64::MAX] {
                let acts = vec![
                    Act::Encumber {
                        low: 1,
                        high: 31,
                        by: "ancestral".to_owned(),
                        witnessed: "both registries".to_owned(),
                    },
                    Act::Anchor {
                        chain: "south".to_owned(),
                        height,
                        digest: digest.clone(),
                        witnessed: "/opt/mirrors/south/ledger/founding.tlv".to_owned(),
                    },
                    Act::Issue {
                        holder: "alpha".to_owned(),
                        low: 32,
                        high: 39,
                    },
                ];
                let bytes = chain::encode(&acts);
                assert_eq!(chain::decode(&bytes), Ok(acts), "the anchor did not survive");
            }
        }

        // The refusal twin: an act nobody knows tears the whole chain.
        let unknown = chain::encode(&[Act::Retire {
            holder: "alpha".to_owned(),
        }]);
        let mut mangled = unknown.clone();
        mangled[0] = 99;
        assert!(
            chain::decode(&mangled).is_err(),
            "an unknown act decoded — a reader that skips one folds a \
             different history and reports it as this one",
        );
    }

    // ===================================================================
    // S5 — an anchor is checkable, and checking it can fail
    // ===================================================================

    /// **A true anchor confirms; a tampered one does not.** Both, or the
    /// check is decoration.
    #[test]
    fn s5_an_anchor_is_confirmed_against_the_chain_it_names() {
        let south = chain_with("south", "beta", 8);

        for height in 0..=south.acts().len() {
            let truth = fnv(&chain::encode(south.at(height).acts()));
            let honest = Act::Anchor {
                chain: "south".to_owned(),
                height: height as u64,
                digest: truth.clone(),
                witnessed: "/opt/mirrors/south".to_owned(),
            };
            assert_eq!(
                confirms(&honest, &south, fnv),
                Some(true),
                "an honest anchor at height {height} did not confirm",
            );

            // Every single-byte perturbation of the digest must refuse.
            for at in 0..truth.len() {
                let mut lie = truth.clone();
                lie[at] ^= 0xff;
                let Act::Anchor { chain, .. } = &honest else {
                    unreachable!()
                };
                let forged = Act::Anchor {
                    chain: chain.clone(),
                    height: height as u64,
                    digest: lie,
                    witnessed: "forged".to_owned(),
                };
                assert_eq!(
                    confirms(&forged, &south, fnv),
                    Some(false),
                    "a digest wrong in byte {at} confirmed at height {height}",
                );
            }
        }
    }

    /// **Unanswerable is not false.** An anchor naming a different chain,
    /// or citing a height we do not have, is `None` — saying `false` would
    /// accuse a peer of lying about a prefix we simply do not hold yet.
    #[test]
    fn s5_an_unanswerable_anchor_is_not_a_refusal() {
        let south = chain_with("south", "beta", 8);
        let digest = fnv(&chain::encode(south.acts()));

        let elsewhere = Act::Anchor {
            chain: "east".to_owned(),
            height: 1,
            digest: digest.clone(),
            witnessed: "somewhere".to_owned(),
        };
        assert_eq!(confirms(&elsewhere, &south, fnv), None, "wrong chain answered");

        let future = Act::Anchor {
            chain: "south".to_owned(),
            height: south.height() + 1,
            digest,
            witnessed: "somewhere".to_owned(),
        };
        assert_eq!(confirms(&future, &south, fnv), None, "a future height answered");

        let horizontal = Act::Retire {
            holder: "beta".to_owned(),
        };
        assert_eq!(confirms(&horizontal, &south, fnv), None, "a horizontal answered");
    }

    // ===================================================================
    // S6 — the standoff, classified by what each party had SEEN
    // ===================================================================

    /// **Concurrent.** Two chains deed the same ground, neither having
    /// anchored the other. Nobody is at fault and the board arbitrates.
    #[test]
    fn s6_two_blind_chains_collide_concurrently() {
        let north = chain_with("north", "alpha", 8);
        let south = chain_with("south", "beta", 8);

        let found = standoffs(&north, &south);
        assert_eq!(found.len(), 1, "one overlap, one standoff: {found:?}");
        let standoff = &found[0];
        assert_eq!(standoff.order, Precedence::Concurrent);
        assert_eq!(standoff.here.chain, "north");
        assert_eq!(standoff.here.holder, "alpha");
        assert_eq!(standoff.there.holder, "beta");
        assert!(
            north.standing_of(standoff.point[0]) != isthmus::deed::Standing::Open,
            "the disputed point is not deeded on the chain that claims it",
        );
    }

    /// **The other arm.** The same collision, but this time the party
    /// anchored the other chain *above* the conflicting act and deeded over
    /// it anyway. Not a collision — a party at fault.
    ///
    /// This is the gate's firing twin: without it, a classifier hard-coded
    /// to `Concurrent` passes the test above.
    #[test]
    fn s6_a_party_that_had_already_seen_the_claim_is_at_fault() {
        let south = chain_with("south", "beta", 8);
        let seen = fnv(&chain::encode(south.acts()));

        // north observes south FIRST, then deeds over it.
        let mut north = Ledger::new(Layout::founding()).under("north");
        north.anchor("south", south.height(), &seen, "/opt/mirrors/south");
        north.issue("alpha", 8).expect("a bare edge deeds");

        let found = standoffs(&north, &south);
        assert_eq!(found.len(), 1, "one overlap, one standoff: {found:?}");
        assert_eq!(
            found[0].order,
            Precedence::HereSawThere,
            "north anchored south above the claim and deeded over it anyway",
        );

        // And the mirror is the mirror: asking the other way round must
        // name the same party at fault, not whichever chain came first.
        let mirrored = standoffs(&south, &north);
        assert_eq!(mirrored.len(), 1);
        assert_eq!(
            mirrored[0].order,
            Precedence::ThereSawHere,
            "the classification depends on which chain was asked first",
        );

        // The anchor north recorded is true, so this is fault and not a
        // misreading: it really did see that state.
        assert_eq!(
            confirms(&north.acts()[0], &south, fnv),
            Some(true),
            "the anchor the fault rests on does not confirm",
        );
    }

    /// **Anchoring after the fact convicts nobody.** Learning about a claim
    /// today says nothing about what was known when the ground was deeded,
    /// and a classifier reading the present frontier would get this wrong.
    #[test]
    fn s6_an_anchor_recorded_afterwards_does_not_make_a_party_at_fault() {
        let south = chain_with("south", "beta", 8);

        let mut north = chain_with("north", "alpha", 8);
        // The order is the whole test: deed first, observe second.
        north.anchor(
            "south",
            south.height(),
            &fnv(&chain::encode(south.acts())),
            "/opt/mirrors/south",
        );

        let found = standoffs(&north, &south);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].order,
            Precedence::Concurrent,
            "a party was convicted for what it learned afterwards",
        );
    }

    /// **Two chains that do not overlap produce no standoff** — the check
    /// is not reporting a conflict for every pair it is handed.
    #[test]
    fn s6_disjoint_chains_stand_off_over_nothing() {
        let north = chain_with("north", "alpha", 8);

        let mut south = Ledger::new(Layout::founding()).under("south");
        south.encumber(1, 200, "reserved", "constructed for this law");
        south.issue("beta", 8).expect("above the encumbrance");

        assert!(
            standoffs(&north, &south).is_empty(),
            "disjoint ground reported a standoff",
        );
        // And south really did deed something, so the emptiness is
        // disjointness rather than an empty chain.
        assert_eq!(south.deeds().iter().filter(|d| d.live).count(), 1);
    }

    /// **A retired claim is not a dispute.** Ground spent on one side
    /// settles nothing, and dragging it to the board would price a claim
    /// nobody holds.
    #[test]
    fn s6_retired_ground_is_not_disputed() {
        let north = chain_with("north", "alpha", 8);
        let mut south = chain_with("south", "beta", 8);

        assert_eq!(standoffs(&north, &south).len(), 1, "the live case first");
        south.retire("beta");
        assert!(
            standoffs(&north, &south).is_empty(),
            "a retired claim was still disputed",
        );
    }

    /// **Mutual anchors above each other's acts report no agreed order.**
    ///
    /// Only reachable through an anchor that cites a height its target does
    /// not have, which [`confirms`] answers `None` for. The classifier must
    /// not pick a side on it: choosing arbitrarily here would let a party
    /// manufacture fault by anchoring the future.
    #[test]
    fn s6_a_manufactured_anchor_does_not_manufacture_fault() {
        let mut north = Ledger::new(Layout::founding()).under("north");
        let mut south = Ledger::new(Layout::founding()).under("south");

        north.anchor("south", 99, &fnv(b"invented"), "invented");
        north.issue("alpha", 8).expect("a bare edge deeds");
        south.anchor("north", 99, &fnv(b"invented"), "invented");
        south.issue("beta", 8).expect("a bare edge deeds");

        let found = standoffs(&north, &south);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].order,
            Precedence::Concurrent,
            "an invented anchor manufactured fault",
        );
        // And the invention is detectable, which is why the board is not
        // stuck with the tie.
        assert_eq!(confirms(&north.acts()[0], &south, fnv), None);
    }

    // ===================================================================
    // S7 — the wire half: a peer says who it is
    // ===================================================================

    /// **The opt-out is byte-identical to `IS-5/1`.**
    ///
    /// This is the whole of what makes the field safe to add to a live
    /// wire: a peer that has not chosen to be addressable emits exactly
    /// the bytes it emitted before, so nothing that already works stops.
    #[test]
    fn s7_a_peer_that_declares_no_uplink_is_byte_identical() {
        let named = chain_with("north", "alpha", 8);
        let mut anonymous = Ledger::new(Layout::founding());
        anonymous.issue("alpha", 8).expect("a bare edge deeds");

        let plain = Hello::of(&named, "alpha", 1 << 20).encode();
        assert_eq!(
            Hello::of(&anonymous, "alpha", 1 << 20).encode(),
            plain,
            "the name leaked into a declaration that did not opt in",
        );
        // And an unnamed chain has nothing to declare, so opting in is a
        // no-op rather than an error — the downstream peer says nothing,
        // not that it is nobody.
        assert_eq!(Uplink::of(&anonymous, fnv), None);
        assert_eq!(
            Hello::of(&anonymous, "alpha", 1 << 20)
                .declaring(Uplink::of(&anonymous, fnv))
                .encode(),
            plain,
            "declaring nothing changed the bytes",
        );
        // Opting in does change them, or the field is not being sent.
        assert_ne!(
            Hello::of(&named, "alpha", 1 << 20)
                .declaring(Uplink::of(&named, fnv))
                .encode(),
            plain,
            "the uplink block was not emitted",
        );
    }

    /// Round trip, and **absent is not empty**: a named chain with an empty
    /// frontier is a real declaration, and a chain with no name is not.
    #[test]
    fn s7_the_declaration_round_trips_and_absent_is_not_empty() {
        let mut cases = vec![
            Hello::of(&chain_with("north", "alpha", 8), "alpha", 1 << 20),
            Hello::default(),
        ];
        for width in [1u128, 8, 200] {
            let ledger = chain_with("north", "alpha", width);
            cases.push(
                Hello::of(&ledger, "alpha", 1 << 20).declaring(Uplink::of(&ledger, fnv)),
            );
        }
        // A named chain that has seen nothing at all, and one with a wide
        // frontier and an empty digest.
        cases.push(Hello::default().declaring(Some(Uplink {
            chain: "north".to_owned(),
            digest: Vec::new(),
            frontier: Frontier::new(),
        })));
        let mut wide = Frontier::new();
        for (name, height) in [("north", 1u64), ("south", 9), ("east", u64::MAX)] {
            wide.observe(name, height);
        }
        cases.push(Hello::default().declaring(Some(Uplink {
            chain: "north".to_owned(),
            digest: vec![0xff; 64],
            frontier: wide,
        })));

        for hello in &cases {
            let bytes = hello.encode();
            assert_eq!(Hello::decode(&bytes).as_ref(), Ok(hello), "no round trip");

            // A truncated declaration is not a partial one — with exactly
            // one cut admitted, and it is worth stating sharply because it
            // is the format's one downgrade.
            //
            // The uplink block is optional and last, so a cut at the byte
            // where it begins yields a *complete, valid* declaration that
            // simply says nothing about who the sender is. No arrangement
            // of an optional trailing field avoids that. What can be
            // pinned down is that this is the ONLY surviving cut and that
            // what survives is strictly less: never a different uplink,
            // only an absent one.
            let anonymous = Hello {
                uplink: None,
                ..hello.clone()
            }
            .encode();
            for cut in 1..bytes.len() {
                let Ok(decoded) = Hello::decode(&bytes[..cut]) else {
                    continue;
                };
                assert_eq!(
                    cut,
                    anonymous.len(),
                    "a declaration cut at {cut} decoded somewhere other than \
                     the uplink boundary",
                );
                assert_eq!(
                    decoded.uplink, None,
                    "a cut produced a DIFFERENT uplink rather than no uplink",
                );
                assert_eq!(
                    decoded,
                    Hello {
                        uplink: None,
                        ..hello.clone()
                    },
                    "a cut changed something other than the uplink",
                );
            }
            // And trailing bytes are not this declaration.
            let mut extra = bytes.clone();
            extra.push(0);
            assert!(Hello::decode(&extra).is_err(), "trailing bytes accepted");
        }

        // Absent and empty must not encode the same.
        let absent = Hello::default();
        let empty = Hello::default().declaring(Some(Uplink::default()));
        assert_ne!(absent.encode(), empty.encode(), "absent encoded as empty");
        assert_eq!(Hello::decode(&empty.encode()), Ok(empty));
    }

    /// **End to end: a peer declares, and the anchor it justifies is true.**
    ///
    /// This is the uplink, and it closes the direction that did not exist —
    /// south says who it is, north records having seen it, and the record
    /// confirms against south's actual chain.
    #[test]
    fn s7_a_declaration_becomes_a_vertical_that_confirms() {
        let south = chain_with("south", "beta", 8);
        let declared = Hello::of(&south, "beta", 1 << 20).declaring(Uplink::of(&south, fnv));

        // Across the wire.
        let heard = Hello::decode(&declared.encode()).expect("the declaration did not survive");
        let uplink = heard.uplink.as_ref().expect("no uplink declared");
        assert_eq!(uplink.chain, "south");
        assert_eq!(uplink.height(), south.height());

        let mut north = Ledger::new(Layout::founding()).under("north");
        north.record(uplink.anchor("declared over the session"));

        assert_eq!(
            confirms(&north.acts()[0], &south, fnv),
            Some(true),
            "the anchor a live declaration produced does not confirm",
        );
        assert_eq!(north.frontier().height_of("south"), south.height());

        // The gate fires: a declaration about a chain that then moves on
        // no longer confirms at the height it stated... and still confirms
        // at that height, because a prefix is a prefix. Both, or the check
        // is either useless or wrong.
        let mut moved = south.clone();
        moved.encumber(200, 210, "somebody", "later");
        assert_eq!(
            confirms(&north.acts()[0], &moved, fnv),
            Some(true),
            "a chain growing invalidated an anchor to its prefix",
        );
        let stale = Uplink {
            chain: "south".to_owned(),
            digest: uplink.digest.clone(),
            frontier: moved.frontier(),
        };
        assert_eq!(
            confirms(&stale.anchor("stale"), &moved, fnv),
            Some(false),
            "a digest of the old prefix confirmed at the new height",
        );
    }

    /// **One anchor, over the sender's own chain, and nothing else.**
    ///
    /// A declaration's frontier names other chains, and those are the
    /// sender's observations, not ours. Recording them as our own acts
    /// would launder provenance — "I observed X" on the strength of
    /// somebody saying they did.
    #[test]
    fn s7_a_declaration_launders_no_provenance() {
        let mut south = Ledger::new(Layout::founding()).under("south");
        south.anchor("east", 7, &fnv(b"east at 7"), "south's own reading");
        south.issue("beta", 8).expect("a bare edge deeds");

        let uplink = Uplink::of(&south, fnv).expect("south is named");
        assert_eq!(
            uplink.frontier.chains(),
            vec!["east", "south"],
            "the declaration should carry what south has seen",
        );

        let act = uplink.anchor("declared over the session");
        let Act::Anchor { chain, .. } = &act else {
            panic!("not a vertical")
        };
        assert_eq!(chain, "south", "an anchor was minted over hearsay");

        // The ordering power survives even though the observation does not
        // — that is the distinction the type is making.
        let mut north = Ledger::new(Layout::founding()).under("north");
        north.record(act);
        assert_eq!(north.frontier().height_of("east"), 0, "hearsay was recorded");
        assert_eq!(uplink.frontier.height_of("east"), 7, "hearsay was discarded");
    }

    /// **Anonymous is not concurrent.** `Frontier::compare` answering
    /// `None` means simultaneous; `Hello::against` answering `None` means
    /// one of you is unaddressable, and reporting the second as the first
    /// would file an unreachable peer as a rival.
    #[test]
    fn s7_an_anonymous_peer_is_not_a_simultaneous_one() {
        let north = chain_with("north", "alpha", 8);
        let south = chain_with("south", "beta", 8);

        let a = Hello::of(&north, "alpha", 1 << 20).declaring(Uplink::of(&north, fnv));
        let b = Hello::of(&south, "beta", 1 << 20).declaring(Uplink::of(&south, fnv));
        let quiet = Hello::of(&south, "beta", 1 << 20);

        // Two named peers that have not seen each other: comparable as a
        // pair, concurrent as a verdict.
        assert_eq!(a.against(&b), Some(None), "two named peers are concurrent");
        // One anonymous: no verdict at all.
        assert_eq!(a.against(&quiet), None, "an anonymous peer was classified");
        assert_eq!(quiet.against(&a), None, "and symmetrically");

        // And an ordered pair is reachable, so `Some(None)` above is a
        // finding rather than the only thing this can say.
        let mut caught_up = north.clone();
        caught_up.record(
            Uplink::of(&south, fnv)
                .expect("south is named")
                .anchor("read over the session"),
        );
        let ahead = Hello::of(&caught_up, "alpha", 1 << 20)
            .declaring(Uplink::of(&caught_up, fnv));
        assert_eq!(
            ahead.against(&b),
            Some(Some(Ordering::Greater)),
            "a peer that anchored the other is not ahead of it",
        );
    }

    /// **An unnamed chain yields no classification at all.** A verdict of
    /// `Concurrent` for every conflict would be right by accident, and a
    /// classification that is right by accident is not one.
    #[test]
    fn s6_an_unaddressable_chain_is_not_classified() {
        let north = chain_with("north", "alpha", 8);
        let mut anonymous = Ledger::new(Layout::founding());
        anonymous.issue("beta", 8).expect("a bare edge deeds");

        assert!(
            standoffs(&north, &anonymous).is_empty(),
            "an unnamed chain was classified",
        );
        // Naming it is the whole difference — the ground never moved.
        assert_eq!(standoffs(&north, &anonymous.under("south")).len(), 1);
    }
}

mod invariant_theorems {


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
        observed.encumber(1, 31, "external-registry", "external registries");
        observed.encumber(1, 31, "mesh-a", "external header");
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
}

mod theorem {


    use isthmus::deed::{cocycle, Flaw, Ledger, Refused};
    use isthmus::layout::{Layout, Tag};

    const SHAPE: [u128; 2] = [4, 3];

    /// A node with holder "H" deeded a 4×3 box, origin shifted by history.
    fn node(spacer_width: u128) -> Ledger {
        let mut edge = Ledger::new(Layout::with_tag_width(1));
        edge.open_axis("revision", 5);
        if spacer_width > 0 {
            edge.issue_box("spacer", &[spacer_width, 6])
                .expect("the spacer must land");
        }
        edge.issue_box("H", &SHAPE).expect("room for H");
        edge
    }

    fn h_box(edge: &Ledger) -> Vec<(Tag, Tag)> {
        edge.deeds()
            .into_iter()
            .find(|d| d.live && d.holder == "H")
            .expect("H holds")
            .region
    }

    fn points(region: &[(Tag, Tag)]) -> Vec<Vec<Tag>> {
        let mut out = Vec::new();
        for x in region[0].0..=region[0].1 {
            for y in region[1].0..=region[1].1 {
                out.push(vec![x, y]);
            }
        }
        out
    }

    /// The translation T1 characterizes.
    fn tau(p: &[Tag], from: &[(Tag, Tag)], onto: &[(Tag, Tag)]) -> Vec<Tag> {
        p.iter()
            .zip(from.iter())
            .zip(onto.iter())
            .map(|((at, (flow, _)), (tlow, _))| tlow + (at - flow))
            .collect()
    }

    // ===================================================================
    // T0 + T1 — bijection, and admit ⟺ translation, exhaustively
    // ===================================================================

    #[test]
    fn t1_the_admitted_relation_is_exactly_the_translation() {
        let a = node(0);
        let b = node(9);
        let box_a = h_box(&a);
        let box_b = h_box(&b);
        assert_ne!(box_a, box_b, "the frames must differ or τ is trivial");

        // φ_A is a bijection onto the offset grid: 12 points, 12
        // distinct offsets, each inside the shape.
        let mut seen = std::collections::BTreeSet::new();
        for p in points(&box_a) {
            let (holder, offsets) = a.potential_at(&p).expect("in the box");
            assert_eq!(holder, "H");
            assert!(u128::from(offsets[0]) < SHAPE[0] && u128::from(offsets[1]) < SHAPE[1]);
            assert!(seen.insert(offsets.clone()), "φ collided at {p:?}");

            // T0: the product verdict is the conjunction of axis verdicts —
            // checked by comparing vector equality against per-axis
            // equality for every candidate q.
            for q in points(&box_b) {
                let admitted = cocycle(&a, &p, &b, &q);
                let (_, wq) = b.potential_at(&q).expect("in the box");
                let per_axis = offsets[0] == wq[0] && offsets[1] == wq[1];
                assert_eq!(admitted, per_axis, "the verdict is not componentwise");

                // T1: admitted exactly at the translation.
                assert_eq!(
                    admitted,
                    q == tau(&p, &box_a, &box_b),
                    "admit and τ disagree at {p:?} -> {q:?}"
                );
            }
        }
        assert_eq!(seen.len(), 12, "the bijection covers the grid");
    }

    // ===================================================================
    // T2 — invariant under EVERY gauge in a generated family, and NOT
    // under a lone-node gauge, which is precisely a disagreement
    // ===================================================================

    /// An injective re-labeling of the offset grid.
    type Gauge = Box<dyn Fn(&[Tag]) -> Vec<Tag>>;

    /// Injective re-labelings of the offset grid: per-axis mirrors,
    /// per-axis rotations, and their compositions.
    fn gauges() -> Vec<Gauge> {
        let mut out: Vec<Gauge> = Vec::new();
        for mirror0 in [false, true] {
            for rot0 in 0..2u64 {
                for mirror1 in [false, true] {
                    let f = move |w: &[Tag]| -> Vec<Tag> {
                        let s0 = SHAPE[0] as u64;
                        let s1 = SHAPE[1] as u64;
                        let mut x = if mirror0 { s0 - 1 - w[0] } else { w[0] };
                        x = (x + rot0) % s0;
                        let y = if mirror1 { s1 - 1 - w[1] } else { w[1] };
                        vec![x, y]
                    };
                    out.push(Box::new(f));
                }
            }
        }
        out
    }

    #[test]
    fn t2_the_verdict_is_invariant_under_every_global_regauge() {
        let a = node(0);
        let b = node(9);
        let box_a = h_box(&a);
        let box_b = h_box(&b);

        let mut verdicts = 0usize;
        for g in gauges() {
            for p in points(&box_a) {
                let (_, wp) = a.potential_at(&p).expect("in");
                for q in points(&box_b) {
                    let (_, wq) = b.potential_at(&q).expect("in");
                    // The gauge applied to BOTH derivations — a global
                    // re-labeling, which the wire cannot even express.
                    assert_eq!(
                        g(&wp) == g(&wq),
                        wp == wq,
                        "a global re-gauge moved a verdict at {p:?} -> {q:?}"
                    );
                    verdicts += 1;
                }
            }
        }
        assert_eq!(verdicts, 8 * 12 * 12, "the family was swept whole");

        // And the twin: the SAME gauge applied to one node only is a
        // disagreement, and disagreement is what the gate exists to catch.
        // The identity gauge is exempt — it is the one lone application
        // that changes nothing.
        let mirror = |w: &[Tag]| vec![SHAPE[0] as u64 - 1 - w[0], w[1]];
        let mut refused = 0usize;
        for p in points(&box_a) {
            let (_, wp) = a.potential_at(&p).expect("in");
            let q = tau(&p, &box_a, &box_b);
            let (_, wq) = b.potential_at(&q).expect("in");
            if mirror(&wp) != wq {
                refused += 1;
            }
        }
        assert!(
            refused > 0,
            "a lone-node gauge produced no disagreement — the gate has \
             nothing to fire on and the theorem's twin is vacuous"
        );
    }

    // ===================================================================
    // T3 — every non-translation is refused per edge, no loop needed
    // ===================================================================

    #[test]
    fn t3_every_generated_non_translation_is_refused_on_its_own_edge() {
        let a = node(0);
        let b = node(9);
        let box_a = h_box(&a);
        let box_b = h_box(&b);

        // The transducer family: τ composed with every non-identity gauge
        // of the offset grid — mirrors, rotations, compositions. Each is
        // a plausible, in-range, holder-preserving map. Each must fail
        // SOMEWHERE, on its own edge.
        let mut non_translations = 0usize;
        for g in gauges() {
            // Detect the identity by probing the grid.
            let is_identity = points(&box_a).iter().all(|p| {
                let (_, w) = a.potential_at(p).expect("in");
                g(&w) == w
            });
            if is_identity {
                continue;
            }
            non_translations += 1;

            let mut caught_at = None;
            for p in points(&box_a) {
                let (_, wp) = a.potential_at(&p).expect("in");
                let mangled = g(&wp);
                let q: Vec<Tag> = mangled
                    .iter()
                    .zip(box_b.iter())
                    .map(|(w, (low, _))| low + w)
                    .collect();
                if !cocycle(&a, &p, &b, &q) {
                    caught_at = Some(p.clone());
                    break;
                }
            }
            assert!(
                caught_at.is_some(),
                "a non-translation was admitted at every point — T3 is refuted"
            );
        }
        assert_eq!(
            non_translations, 7,
            "the family holds 8 gauges and one identity"
        );
    }

    // ===================================================================
    // T4 — flatness: every cycle of admitted hops closes, k = 2..5
    // ===================================================================

    #[test]
    fn t4_every_admitted_cycle_of_every_generated_length_closes() {
        let nodes: Vec<Ledger> = vec![node(0), node(9), node(4), node(13), node(2)];
        let boxes: Vec<_> = nodes.iter().map(h_box).collect();

        for k in 2..=nodes.len() {
            let ring = &nodes[..k];
            let ring_boxes = &boxes[..k];

            for p in points(&ring_boxes[0]) {
                let mut at = p.clone();
                for hop in 0..k {
                    let next = (hop + 1) % k;
                    let q = ring[hop]
                        .translate_at(&at, &ring[next])
                        .unwrap_or_else(|| panic!("hop {hop} failed at {at:?}"));
                    assert!(
                        cocycle(&ring[hop], &at, &ring[next], &q),
                        "an unverified hop in the ring"
                    );
                    at = q;
                }
                assert_eq!(at, p, "a {k}-cycle of admitted hops did not close");
            }
        }
    }

    // ===================================================================
    // THE BOUNDARY — the hypothesis is necessary, constructed
    // ===================================================================

    /// Two same-shape live boxes for one holder on one node: one claim,
    /// two admitted points. The relation stops being a function, exactly
    /// as the theorem's hypothesis says it must — and three facts about
    /// that state are shown at once:
    ///
    /// 1. **The issuer cannot reach it.** `issue_box` refuses the second
    ///    deed with `AlreadyHeld` — the hypothesis is an invariant of the
    ///    issuance discipline, by induction, not an assumption.
    /// 2. **Transcription can reach it**, because `record()` judges
    ///    nothing. History is where the state can exist.
    /// 3. **The checker names it.** `well_formed` refuses the transcribed
    ///    chain as `DoubleHold`, at the exact act — so for chains the
    ///    issuer did not build, the hypothesis is discharged by a
    ///    decidable check rather than assumed.
    #[test]
    fn the_single_deed_hypothesis_is_necessary_not_decorative() {
        let a = node(0);
        let box_a = h_box(&a);

        let mut b = Ledger::new(Layout::with_tag_width(1));
        b.open_axis("revision", 5);
        let first = b.issue_box("H", &SHAPE).expect("room");

        // 1. The issuer holds the invariant.
        assert!(
            matches!(
                b.issue_box("H", &SHAPE),
                Err(Refused::AlreadyHeld { .. })
            ),
            "the issuer built the state the theorem forbids"
        );
        assert!(b.well_formed().is_ok(), "the refused issue left no trace");

        // 2. Transcription reaches it anyway — a disjoint same-shape box,
        // injected as history.
        let second: Vec<(Tag, Tag)> = vec![
            (first.region[0].1 + 1, first.region[0].1 + 4),
            (first.region[1].0, first.region[1].1),
        ];
        b.record(isthmus::deed::Act::IssueBox {
            holder: "H".into(),
            region: second.clone(),
        });

        let p = vec![box_a[0].0, box_a[1].0]; // offset (0,0) on A
        let q1 = vec![first.region[0].0, first.region[1].0];
        let q2 = vec![second[0].0, second[1].0];
        assert_ne!(q1, q2);

        assert!(cocycle(&a, &p, &b, &q1), "the first box admits");
        assert!(
            cocycle(&a, &p, &b, &q2),
            "the second box admits the SAME claim"
        );

        // 3. The checker refuses the chain, naming the act — index 2:
        // Open, the first issue, the transcription. The REFUSED issue left
        // no act, which is itself part of the invariant: a refusal that
        // appended would be a history of things that did not happen.
        assert!(
            matches!(b.well_formed(), Err(Flaw::DoubleHold { at: 2, .. })),
            "the checker did not name the transcribed double-hold: {:?}",
            b.well_formed()
        );
    }
}
