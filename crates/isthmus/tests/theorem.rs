//! THE COCYCLE VERIFICATION THEOREM, cross-referenced to the spatial
//! structure and proven.
//!
//! # Setting
//!
//! The deed space is a **product of lines**: axes `a = 0..n`, each an
//! interval of coordinates, a deed a box `D = Π [low_a, high_a]`, a
//! point `p` a coordinate tuple. On node `N`, the potential of a point
//! inside holder `H`'s box is the per-axis offset
//!
//! ```text
//! φ_N(p) = p − low(D_N)        componentwise
//! ```
//!
//! A crossing `p@A → q@B` is **admitted** when the holders agree and
//! `φ_A(p) = φ_B(q)` — the cocycle condition, checked per edge.
//!
//! # Theorem
//!
//! Let holder `H` hold exactly one live deed per node, all of shape
//! `s` (the **single-deed hypothesis**). Then:
//!
//! **T0 (spatial decomposition).** The verdict on the product space is
//! the conjunction of the per-axis verdicts. *Proof:* equality of
//! offset vectors is componentwise. So every claim below reduces to
//! one dimension and lifts through the product — the spatial and the
//! wire-side findings are the same finding, once per axis.
//!
//! **T1 (characterization).** `φ_N` is a bijection `D_N → Π [0, s_a)`,
//! and the admitted relation is exactly the translation
//! `τ(p) = low(D_B) + (p − low(D_A))` — total on `D_A`, one `q` per
//! `p`. *Proof:* subtraction by a constant is a bijection of intervals
//! onto `[0, s_a)` per axis; product of bijections is a bijection.
//! `admit ⟺ φ_B(q) = φ_A(p) ⟺ q = φ_B⁻¹(φ_A(p)) = τ(p)`. ∎
//!
//! **T2 (gauge invariance).** For ANY injective `g` on offsets applied
//! to every node's derivation, the admitted set is unchanged:
//! `g(φ_A(p)) = g(φ_B(q)) ⟺ φ_A(p) = φ_B(q)`. *Proof:* injectivity,
//! both directions. ∎  — The verdict is a gauge invariant. The mirror
//! mutation that stayed green in the exchange was one point of this
//! family; the theorem is the whole family, which is why a gate that
//! anchors a labeling is wrong: it fires inside an equivalence class
//! the wire cannot even express.
//!
//! **T3 (per-edge completeness).** Any transducer `T: D_A → D_B` that
//! is admitted at every point IS the translation. *Proof:* admitted
//! everywhere means `φ_B ∘ T = φ_A`; `φ_B` is a bijection (T1), so
//! `T = φ_B⁻¹ ∘ φ_A = τ`. ∎  Contrapositive: every `T ≠ τ` is refused
//! at some point **on its own edge** — mirrors, shifts, swaps, all of
//! them, with no cycle required. The involution that hid from the loop
//! law cannot hide from this, because cancellation needs a loop and
//! there is none.
//!
//! **T4 (flatness).** Around any cycle of admitted hops the composite
//! is the identity. *Proof:* each hop preserves φ, so the endpoint of
//! the loop has the start's potential on the starting node, and φ is
//! injective there (T1), so endpoint = start. ∎  — The connection is
//! flat; A8's three-node closure was an instance, this is every cycle
//! of every length.
//!
//! **Boundary.** Drop the single-deed hypothesis and T1 fails
//! constructively: with two same-shape live boxes for one holder on
//! one node, one claim admits two distinct points — the relation stops
//! being a function. The hypothesis is necessary, not decorative, and
//! the court is checked for it in datum (`tests/chain.rs`).
//!
//! # What "proven" means here
//!
//! The ∎-arguments above are the proof. Each test below verifies its
//! claim **exhaustively over a stated finite space** — every point,
//! every generated gauge, every generated transducer — so the prose
//! and the machine check the same statements from two sides, and a
//! mutation run shows the checks can fail.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

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
