//! LAWS for replay. `IS-2` §6.1, applied to the chain's own acts.
//!
//! The ruling:
//!
//! > A frame that has an effect must be idempotent under replay,
//! > **either naturally or by carrying an identity the receiver dedups
//! > on.**
//!
//! `IS-2` §6 measured that rule against `strand`'s aperture, where a
//! replayed pair doubles both sides. It was never measured against the
//! acts — and **the acts are effects**: every one of them moves the
//! fold. A chain is exactly the place where an effect applied twice
//! moves capacity twice.
//!
//! One law over every act, not one test per act. A test per act is how
//! a ninth act arrives uncovered.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

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
    assert_eq!(two.axes().len(), 3, "a second NAME did not open an axis");
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
