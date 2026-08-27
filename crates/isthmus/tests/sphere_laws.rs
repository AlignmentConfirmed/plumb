//! LAWS for the chain of chains.
//!
//! The chain was a line. These are the properties that must hold once
//! it is not: that the join is a hypersphere envelope, that the order is
//! genuinely partial, that a vertical grants nothing, and that a
//! standoff between two chains is classified by what each party had
//! actually seen rather than by which chain we happened to ask first.
//!
//! Every gate here is built twice — a state it must admit and a state
//! it must refuse. A classifier that answered `Concurrent` for
//! everything would pass a test that only ever constructs concurrent
//! chains, and "no one is at fault" is the answer that costs somebody
//! their claim.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

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
