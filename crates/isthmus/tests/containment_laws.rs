//! LAWS for containment: estates inside estates.
//!
//! [`Act::Sublet`] deliberately breaks the disjointness the cocycle
//! theorem was stated with. Two live deeds cover the same ground, at
//! different depths, on purpose — so the hypothesis is **restated**
//! rather than weakened:
//!
//! ```text
//! H2   live deeds are pairwise disjoint
//! H2'  live deeds AT THE SAME DEPTH are pairwise disjoint,
//!      and every deed is strictly inside its parent
//! ```
//!
//! `H2′` reduces to `H2` on a chain with no sublets, so nothing already
//! proven is surrendered. What it buys is the property `H2` was only
//! ever a way of getting: the containment chain over any point is
//! **totally ordered by depth**, so "deepest holder" is unique and
//! `holder_at` is still a function.
//!
//! These laws are what makes that a claim rather than an assertion.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

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
/// pricing is the board's, and `decide/arbitration.md` rules it must
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
