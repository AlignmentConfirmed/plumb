//! THE BOARD, run end to end: a kernel joins the POWC network.
//!
//! Edge-free — the court is the stored chain, the survey is geometry,
//! and no kernel is linked.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use datum::extent::Extent;
use datum::board::{clears, enact, survey, validate, Application, Estate, Proposal, Turned};
use datum::negotiation::Position;
use isthmus::deed::{Act, Ledger, Standing};
use isthmus::layout::Layout;
use isthmus::ratio::Exact;
use num_bigint::BigInt;

fn court() -> Ledger {
    datum::ledger::authority().expect("no authority")
}

/// The old helper took `work: u128` — a scalar. It now builds a
/// position offering that amount on the base pole, so every call site
/// reads unchanged while the structure underneath is the negotiable
/// one.
/// An application, offering `work` **on every axis it could be asked
/// for**.
///
/// The ask is per-axis now, so a position is too. It was one offer on
/// `BASE_POLE` — one pole standing for every direction, matching a
/// price that had been folded into one number. Both collapses are
/// gone, so an applicant states what it will pay for space in each
/// direction, which is what it was always actually offering.
fn apply(applicant: &str, shape: Vec<u128>, work: u128) -> Application {
    let mut position = Position::new();
    let amount = Exact::from(BigInt::from(work));
    // The founding axis, plus every direction a galaxy could open for
    // a request of this arity. The names are the court's, so an
    // applicant reads them rather than being told.
    position.offer(isthmus::layout::TAG, amount.clone());
    // A galaxy names its new directions after the applicant that
    // asked for them, so those are the poles it will be charged on.
    for at in 1..=shape.len() {
        position.offer(&format!("{applicant}-d{at}"), amount.clone());
        position.offer(&format!("nova-d{at}"), amount.clone());
    }
    Application {
        applicant: applicant.into(),
        shape,
        position,
        witness: format!("{applicant}'s convergence witness, held for #43"),
    }
}

// ===================================================================
// "I want to join the POWC network"
// ===================================================================

/// The whole flow against the REAL court: apply, survey, validate,
/// enact — and afterwards the newcomer holds a deed the theorems apply
/// to, because the future chain was well-formed before it was history.
#[test]
fn a_new_kernel_joins_and_the_court_grows_by_one_estate() {
    let court = court();
    let before: Vec<_> = court.deeds().into_iter().filter(|d| d.live).collect();

    let application = apply("coral", vec![8], 64);
    let proposal = survey(&court, &application).expect("space exists on the founding edge");

    // The estate is a run on the line, priced PER AXIS. A one-axis
    // court has a one-component price, and `[8]` is the extent taken —
    // not a volume, which would call `[2, 8]` and `[4, 4]` the same
    // request.
    assert!(matches!(proposal.estate, Estate::Run { .. }));
    assert_eq!(proposal.price, Extent::new(vec![8]));
    clears(&proposal, &application.position).expect("the position clears the ask");

    // Authenticity before history.
    validate(&court, &proposal).expect("a surveyed proposal validates");
    let grown = enact(&court, &proposal, &application.position).expect("the board enacts");

    // The newcomer holds; nobody moved; the hypotheses still hold, so
    // the cocycle theorems apply to the grown court unchanged.
    assert!(grown.deeds().iter().any(|d| d.live && d.holder == "coral"));
    for old in &before {
        assert!(
            grown
                .deeds()
                .iter()
                .any(|d| d.live && d.holder == old.holder && d.region == old.region),
            "{} moved when coral joined",
            old.holder
        );
    }
    grown.well_formed().expect("the grown court is well-formed");
}

/// The docket: three applicants in sequence, each validated against
/// the chain **as grown so far** — a queue, not a snapshot.
#[test]
fn the_docket_processes_in_order_against_the_growing_chain() {
    let mut chain = court();
    let docket = [
        apply("coral", vec![8], 100),
        apply("basalt", vec![16], 100),
        apply("pumice", vec![4], 100),
    ];

    let mut granted: Vec<Proposal> = Vec::new();
    for application in &docket {
        let proposal = survey(&chain, application).expect("space remains");
        chain = enact(&chain, &proposal, &application.position).expect("validated and landed");
        granted.push(proposal);
    }

    chain.well_formed().expect("the grown chain is well-formed");
    for one in &granted {
        assert!(chain.deeds().iter().any(|d| d.live && d.holder == one.applicant));
    }
    // Sequential grants never collide: the second survey saw the
    // first's grant because the docket validates against the grown
    // chain, not the snapshot it started from.
    for (i, one) in granted.iter().enumerate() {
        for other in granted.iter().skip(i + 1) {
            if let (Some(Act::Issue { low: a, high: b, .. }), Some(Act::Issue { low: c, high: d, .. })) =
                (one.acts.first(), other.acts.first())
            {
                assert!(b < c || d < a, "the docket granted overlapping runs");
            }
        }
    }
}

// ===================================================================
// The calculation makes space: the galaxy
// ===================================================================

/// A full line is not a refusal — it is a galaxy. The survey opens an
/// axis, places the estate off the zero slice, and prices it at the
/// space CREATED, which exceeds the box requested.
#[test]
fn when_the_line_is_full_the_survey_makes_a_galaxy_and_prices_the_creation() {
    // A small court, filled COMPLETELY — and filled such that NOBODY
    // can sell the requested shape. Two premise errors were caught here
    // by the survey itself: first a 31-tag remnant got a Run (the line
    // was not full), then 32-wide owners got a Parcel (the space was
    // for sale — the purchase correctly outranks creation). A galaxy
    // is the move when the space neither exists nor is sellable:
    // 16-wide estates cannot cede a 16-slab and remain a box.
    let mut small = Ledger::new(Layout::with_tag_width(1));
    let mut n = 0usize;
    while small.issue(&format!("m{n}"), 16).is_ok() {
        n += 1;
    }
    while small.issue(&format!("m{n}"), 1).is_ok() {
        n += 1;
    }
    assert_eq!(small.largest_open(), 0, "the line must actually be full");

    let application = apply("nova", vec![16], 10_000);
    let proposal = survey(&small, &application).expect("the survey makes space");

    let Estate::Galaxy { ref axes, ref region } = proposal.estate else {
        panic!("a full line must yield a galaxy, got {:?}", proposal.estate);
    };
    assert_eq!(axes, &vec!["nova-d1".to_string()]);
    assert!(region[1].0 >= 1, "the estate landed on the full zero slice");

    // The price is the space created, PER AXIS — the new direction as
    // well as the box taken from it. `>` was asserted here against a
    // product, which is a total order a volume supplies falsely: two
    // estates of different shape and equal product compare equal. The
    // honest statement is per axis.
    let taken = Extent::of(region);
    assert!(
        !proposal.price.fits_in(&taken) || proposal.price == taken,
        "the price is smaller than the box on some axis",
    );
    let grown = enact(&small, &proposal, &application.position).expect("lands");
    let after = Extent::of_court(&grown);
    assert!(
        after.axes() >= 2,
        "opening a direction did not add an axis: {after}",
    );
    // The opener paid at least the extent it took, on every axis.
    for at in 0..proposal.price.axes() {
        assert!(
            proposal.price.component(at) >= taken.component(at),
            "axis {at}: paid {:?} for {:?}",
            proposal.price.component(at),
            taken.component(at),
        );
    }

    // And everyone who held the line still holds it, on the zero slice.
    for holder in 0..n {
        let name = format!("m{holder}");
        assert!(grown.deeds().iter().any(|d| d.live && d.holder == name));
    }
}

// ===================================================================
// The economics balance
// ===================================================================

/// Funding is a negotiation, not a gate. The survey answers geometry
/// regardless; a short position gets a **counter** naming the gap, the
/// proposal stands, and the same estate lands the moment a delta
/// clears the fold. `Turned::Underfunded` — the scalar demand — is
/// gone, and this test is where its replacement separates.
#[test]
fn a_short_position_counters_and_a_covering_one_lands() {
    let court = court();

    let poor = apply("driftwood", vec![32], 31);
    let proposal = survey(&court, &poor).expect("geometry answers regardless of funding");
    assert_eq!(proposal.price, Extent::new(vec![32]));

    let counter = clears(&proposal, &poor.position).expect_err("31 against 32");
    // The demand is named for the AXIS it is a demand on. It was
    // named `BASE_POLE` — one pole standing for every direction —
    // which was the second collapse after the price was folded.
    assert_eq!(
        counter.short,
        vec![("tag".to_string(), Exact::from(BigInt::from(1)))],
        "the counter names the gap, on the axis it is on"
    );

    // The proposal did not die: one more delta and the SAME proposal
    // clears and lands.
    let mut topped = poor.position.clone();
    topped.offer("tag", Exact::from(BigInt::from(32)));
    clears(&proposal, &topped).expect("the fixpoint");
    enact(&court, &proposal, &topped).expect("witnessed and landed");
}

/// **A wider request never costs less on the axis it widened** — and
/// that is all a price can be monotone in.
///
/// This asserted `price >= last` on a product, which is a TOTAL order
/// over shapes that are not totally ordered: `[2, 8]` and `[4, 4]`
/// compare equal under a volume and are incomparable in fact. On one
/// axis the order is real, so the law is stated where it holds.
#[test]
fn a_wider_request_never_costs_less_on_the_axis_it_widened() {
    let court = court();
    let mut last: Option<Extent> = None;
    for width in [1u128, 4, 8, 32, 48] {
        let proposal = survey(&court, &apply("gauge", vec![width], u128::MAX))
            .expect("space exists");
        if let Some(previous) = &last {
            assert!(
                proposal.price.component(0) >= previous.component(0),
                "width {width} priced below a narrower estate on axis 0",
            );
        }
        last = Some(proposal.price.clone());
    }
    assert!(last.is_some(), "no estate was priced");
}

/// **And shapes of equal product are not equal prices.** The defect the
/// per-axis price exists to remove, constructed: `[2, 8]` and `[4, 4]`
/// both fold to 16 and are incomparable per axis.
#[test]
fn two_shapes_a_volume_calls_equal_are_priced_apart() {
    let a = Extent::new(vec![2, 8]);
    let b = Extent::new(vec![4, 4]);

    assert_ne!(a, b, "the two shapes are the same reading");
    assert_eq!(
        a.compare(&b),
        None,
        "[2, 8] and [4, 4] compare — a product would have called them equal",
    );
    assert!(!a.fits_in(&b) && !b.fits_in(&a), "neither contains the other");

    // Their products agree, which is exactly why the product was wrong.
    let product = |e: &Extent| e.components().iter().product::<u128>();
    assert_eq!(product(&a), product(&b), "the premise needs equal products");
    assert_eq!(product(&a), 16);
}

// ===================================================================
// Authenticity: a tampered proposal dies at validation
// ===================================================================

/// An act edited after the survey — moved onto ground the survey never
/// granted — is refused by the future chain's own induction, naming
/// the flaw. The board cannot be handed a forged grant.
#[test]
fn a_tampered_proposal_is_refused_by_the_future_chains_induction() {
    let court = court();
    let mut proposal =
        survey(&court, &apply("magpie", vec![8], 100)).expect("space exists");

    // Tamper: move the granted run onto lith's estate.
    proposal.acts = vec![Act::Issue {
        holder: "magpie".into(),
        low: 130,
        high: 137,
    }];
    match validate(&court, &proposal) {
        Err(Turned::Invalid(flaw)) => {
            let text = format!("{flaw:?}");
            assert!(text.contains("Overlap"), "the wrong flaw: {text}");
        }
        other => panic!("a forged grant survived validation: {other:?}"),
    }

    // And a second tampering class: granting the applicant twice.
    let honest = survey(&court, &apply("magpie", vec![8], 100)).expect("space");
    let mut doubled = honest.clone();
    doubled.acts.extend(honest.acts.clone().into_iter().map(|act| {
        if let Act::Issue { holder, low, high } = act {
            Act::Issue {
                holder,
                low: low + 100,
                high: high + 100,
            }
        } else {
            act
        }
    }));
    assert!(
        matches!(validate(&court, &doubled), Err(Turned::Invalid(_))),
        "a double grant survived validation"
    );
}

// ===================================================================
// The honest boundaries
// ===================================================================

/// Retired ground never reissues — the survey routes around it, so
/// "re-using a system" can only ever mean subletting inside a LIVE
/// estate, which is the sublet act (#47), not a grant of dead ground.
#[test]
fn the_survey_never_grants_retired_ground() {
    let mut chain = Ledger::new(Layout::with_tag_width(1));
    let dead = chain.issue("gone", 64).expect("room");
    chain.retire("gone");

    let scavenger = apply("scavenger", vec![64], 10_000);
    let proposal = survey(&chain, &scavenger)
        .expect("space exists elsewhere or is made");
    if let Some(Act::Issue { low, high, .. }) = proposal.acts.last() {
        assert!(
            *high < dead.low() || *low > dead.high(),
            "the survey granted retired ground"
        );
    }
    let grown = enact(&chain, &proposal, &scavenger.position).expect("lands");
    for tag in dead.low()..=dead.high() {
        assert_eq!(grown.standing_of(tag), Standing::Retired);
    }
}

// ===================================================================
// The purchase: building on a relatively full planet
// ===================================================================

/// The line is full. The survey does not conjure — it finds an owner
/// whose estate can yield a slab of the requested shape, conveys it,
/// and **the owner is on the settlement for exactly the space they
/// gave up**. Ground moves continuously: never open in between, so
/// never-reissue is untouched.
#[test]
fn a_full_planet_sells_a_slab_and_the_owner_is_settled() {
    // One owner holds nearly everything; remnants filled so open space
    // cannot satisfy the request.
    let mut planet = Ledger::new(Layout::with_tag_width(1));
    planet.issue("terra", 200).expect("room");
    let mut n = 0usize;
    while planet.issue(&format!("f{n}"), 1).is_ok() {
        n += 1;
    }
    assert_eq!(planet.largest_open(), 0, "the planet must be full");

    let application = apply("settler", vec![16], 1_000);
    let proposal = survey(&planet, &application).expect("the space is for sale");

    let Estate::Parcel { ref from, ref region } = proposal.estate else {
        panic!("a full planet must sell, got {:?}", proposal.estate);
    };
    assert_eq!(from, "terra");
    assert_eq!(proposal.price, Extent::new(vec![16]));
    assert_eq!(
        proposal.settlement,
        vec![("terra".to_string(), Extent::new(vec![16]))],
        "the owner is not settled for the space they would have occupied"
    );

    let grown = enact(&planet, &proposal, &application.position).expect("the conveyance lands");
    grown.well_formed().expect("still well-formed after the sale");

    // The settler holds the slab; terra holds the remainder; the sum
    // is what terra held — conservation of ground.
    let settler = grown
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == "settler")
        .expect("the settler holds");
    let terra = grown
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == "terra")
        .expect("terra still holds");
    assert_eq!(settler.region, *region);
    // Per axis. On a one-axis court an estate's extent IS its width,
    // and `volume()` is struck — it was the product across axes, and a
    // product calls [2, 8] and [4, 4] the same estate.
    assert_eq!(
        Extent::of(&settler.region).component(0).unwrap_or(0)
            + Extent::of(&terra.region).component(0).unwrap_or(0),
        200,
    );

    // And the potentials still work for BOTH: the theorems apply to
    // the post-sale court unchanged.
    assert!(grown.potential_at(&[settler.low()]).is_some());
    assert!(grown.potential_at(&[terra.low()]).is_some());
}

/// An underfunded buyer is turned away with the price of the parcel —
/// the settlement is a payment, not a formality.
#[test]
fn an_underfunded_buyer_cannot_take_the_slab() {
    let mut planet = Ledger::new(Layout::with_tag_width(1));
    planet.issue("terra", 255).expect("room");

    let squatter = apply("squatter", vec![16], 15);
    let proposal = survey(&planet, &squatter).expect("the slab is surveyable");
    assert_eq!(proposal.price, Extent::new(vec![16]));

    // Force being looked over for now, as ruled: there is no branch
    // that takes without settling. A short position holds a COUNTER,
    // and enactment witnesses a fixpoint that does not exist yet — so
    // the slab does not move, and nothing died either.
    let counter = clears(&proposal, &squatter.position).expect_err("15 against 16");
    assert_eq!(
        counter.short,
        vec![("tag".to_string(), Exact::from(BigInt::from(1)))],
        "the gap is on the axis the space is on",
    );
    assert!(matches!(
        enact(&planet, &proposal, &squatter.position),
        Err(datum::board::EnactRefused::NotCleared(_))
    ));
    // terra still holds everything.
    let terra = planet
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == "terra")
        .expect("terra holds");
    assert_eq!(Extent::of(&terra.region).component(0), Some(255));
}

// ===================================================================
// The 11-D kernel: the cosmology is the analogy, this is the system
// ===================================================================

/// A kernel arrives requiring harmonic 11-D mathematics. The survey
/// accounts for the space on the mesh, opens the ten axes the
/// requirement needs — **it does not flatten the requirement to fit
/// the mesh** — and the estate lands as an 11-D manifold the theorems
/// apply to axis by axis.
#[test]
fn an_11d_kernel_gets_an_11d_manifold_not_a_flattened_one() {
    let court = court();
    assert_eq!(court.axes().len(), 1, "the founding court is a line");

    let shape: Vec<u128> = vec![4, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2];
    assert_eq!(shape.len(), 11);
    // The requirement IS the shape, per axis. It was `shape.iter()
    // .product()` — the flattening the board no longer performs, and
    // which this very test exists to say the survey must not do.
    let needed = Extent::new(shape.clone());

    let application = apply("harmonic", shape.clone(), u128::MAX);
    let proposal = survey(&court, &application).expect("the manifold is made");

    let Estate::Galaxy { ref axes, ref region } = proposal.estate else {
        panic!("an 11-D requirement must open axes, got {:?}", proposal.estate);
    };
    assert_eq!(axes.len(), 10, "ten new directions for eleven dimensions");
    assert_eq!(region.len(), 11, "the estate is genuinely 11-D");
    for (axis, (low, high)) in region.iter().enumerate() {
        let extent = u128::from(*high) - u128::from(*low) + 1;
        assert_eq!(
            extent, shape[axis],
            "axis {axis}: the requirement was flattened"
        );
    }

    // The accounting: what the mesh held, what the kernel needed, and
    // the price of what was created.
    assert_eq!(proposal.accounting.mesh, Extent::new(vec![256]));
    assert_eq!(proposal.accounting.needed, needed);
    // Creating ten directions costs at least the box taken from them,
    // ON EVERY AXIS. `>` was asserted on a product here, which is a
    // total order over shapes that do not have one.
    let asked = Extent::new(shape.clone());
    for at in 0..asked.axes() {
        assert!(
            proposal.price.component(at) >= asked.component(at),
            "axis {at}: paid {:?} for a requirement of {:?}",
            proposal.price.component(at),
            asked.component(at),
        );
    }
    assert!(
        !proposal.price.fits_in(&asked) || proposal.price == asked,
        "the price fits strictly inside the requirement on every axis",
    );

    // Land it and read a point deep in the manifold — the potential is
    // an 11-vector, and the cocycle machinery needs nothing new,
    // because every theorem was proven per axis and lifted.
    let grown = enact(&court, &proposal, &application.position).expect("lands");
    grown.well_formed().expect("well-formed at 11 dimensions");
    let deep: Vec<u64> = region.iter().map(|(low, _)| *low).collect();
    let (holder, offsets) = grown.potential_at(&deep).expect("the origin of the estate");
    assert_eq!(holder, "harmonic");
    assert_eq!(offsets, vec![0u64; 11]);
}

/// A shapeless application is turned away before any calculation.
#[test]
fn a_shapeless_application_is_turned_away() {
    let court = court();
    assert_eq!(
        survey(&court, &apply("void", vec![], 100)),
        Err(Turned::Shapeless)
    );
    assert_eq!(
        survey(&court, &apply("void", vec![8, 0], 100)),
        Err(Turned::Shapeless)
    );
}