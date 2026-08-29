//! COURT LAWS — pricing, credit, settlement, negotiation, block
//! production, wire hygiene, and the workspace's own hygiene.

#![allow(clippy::arithmetic_side_effects, clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod board {


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

        // Tamper: move the granted run onto a kernel's estate.
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

        // As ruled: there is no branch
        // that takes without settling. A short position holds a counter,
        // and enactment witnesses a fixpoint that does not exist yet — so
        // the slab does not move.
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
}

mod reward {

    use assay::{whole, Boundary, Facet, Orientation};
    use datum::extent::Extent;
    use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};

    #[test]
    fn closed_work_credits_per_axis() {
        let mut book = RewardBook::new();
        let body = closed_box_claim(11, 5).encode();
        let credit = book.credit_claim(&body).expect("credit");
        assert_eq!(credit.transport, 11);
        assert_eq!(credit.axes.components(), &[1, 1]);
        assert!(credit.witness.is_some());
        assert_eq!(book.total().components(), &[1, 1]);
    }

    #[test]
    fn same_structure_replays_even_with_different_transport() {
        let mut book = RewardBook::new();
        let a = closed_box_claim(1, 1).encode();
        let b = closed_box_claim(99, 1).encode();
        book.credit_claim(&a).expect("first");
        match book.credit_claim(&b) {
            Err(RewardRefused::Replay { work_id }) => {
                assert_eq!(work_id, closed_box_claim(0, 1).work_id());
            }
            other => panic!("expected structure replay, got {other:?}"),
        }
    }

    #[test]
    fn shape_triangle_credits_per_orb() {
        let mut book = RewardBook::new();
        let body = triangle_claim(3).encode();
        let credit = book.credit_claim(&body).expect("shape credit");
        assert_eq!(credit.axes.components(), &[1, 1, 1]);
        assert!(credit.witness.is_none());
        // same shape, different transport → replay
        assert!(matches!(
            book.credit_claim(&triangle_claim(9).encode()),
            Err(RewardRefused::Replay { .. })
        ));
    }

    #[test]
    fn open_work_earns_nothing() {
        let mut b = Boundary::new(1);
        assert!(b.face(Facet::new(0, Orientation::Low, whole(1))));
        assert!(b.face(Facet::new(0, Orientation::High, whole(9))));
        let body = assay::Claim::new(1, b).encode();
        let mut book = RewardBook::new();
        assert!(matches!(
            book.credit_claim(&body),
            Err(RewardRefused::OpenWork)
        ));
    }

    #[test]
    fn credit_must_cover_price_on_every_axis() {
        let mut book = RewardBook::new();
        book.credit_claim(&closed_box_claim(1, 2).encode())
            .expect("credit");
        assert!(book.covers(&Extent::new(vec![1, 1])));
        assert!(!book.covers(&Extent::new(vec![2, 1])));
        book.settle_against(&Extent::new(vec![1, 1])).expect("ok");
        match book.settle_against(&Extent::new(vec![2, 0])) {
            Err(RewardRefused::Underfunded { .. }) => {}
            other => panic!("expected underfunded, got {other:?}"),
        }
    }

    #[test]
    fn different_structures_stack_credit() {
        let mut book = RewardBook::new();
        book.credit_claim(&closed_box_claim(1, 1).encode())
            .expect("a");
        book.credit_claim(&closed_box_claim(1, 2).encode())
            .expect("b");
        assert_eq!(book.total().components(), &[2, 2]);
    }

    #[test]
    fn onramp_shape_body_in_highway_frame() {
        // Tollway produces shape body; superhighway frames it; court credits
        // the opaque body without knowing the tollway.
        let body = triangle_claim(0).encode();
        let mut wire = Vec::new();
        isthmus::work::put_shape_claim(&body, &mut wire).expect("frame");
        let (tag, value) = isthmus::work::take_frame(&wire).expect("take");
        assert_eq!(tag, isthmus::work::SHAPE_CLAIM_TAG);
        let mut book = RewardBook::new();
        book.credit_claim(value).expect("credit from frame body");
        assert_eq!(book.total().components(), &[1, 1, 1]);
    }
}

mod settle {
    #![allow(
        clippy::panic,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )]

    use datum::block::BlockRefused;
    use datum::extent::Extent;
    use datum::negotiation::Position;
    use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};
    use datum::settle::{self, SettleRefused};
    use datum::board::Application;
    use isthmus::ratio::Exact;
    use num_bigint::BigInt;

    fn apply(applicant: &str, shape: Vec<u128>, work: u128) -> Application {
        let mut position = Position::new();
        let amount = Exact::from(BigInt::from(work));
        position.offer(isthmus::layout::TAG, amount.clone());
        for at in 1..=shape.len() {
            position.offer(&format!("{applicant}-d{at}"), amount.clone());
            position.offer(&format!("nova-d{at}"), amount.clone());
        }
        Application {
            applicant: applicant.into(),
            shape,
            position,
            witness: format!("{applicant} work_id credits"),
        }
    }

    /// One-axis closed boundary bodies (flux 1..=n) → credit [n].
    fn boundary_stack(n: u128) -> Vec<Vec<u8>> {
        let mut bodies = Vec::new();
        for flux in 1..=n {
            let mut b = assay::Boundary::new(1);
            let f = assay::whole(flux as i64);
            assert!(b.face(assay::Facet::new(0, assay::Orientation::Low, f.clone())));
            assert!(b.face(assay::Facet::new(0, assay::Orientation::High, f)));
            bodies.push(assay::Claim::new(0, b).encode());
        }
        bodies
    }

    // ── H4 ─────────────────────────────────────────────────────────────

    #[test]
    fn deed_price_is_multi_axis_extent_not_a_product() {
        // H4 core: space price is Extent; product would call [2,8] == [4,4].
        let a = Extent::new(vec![2, 8]);
        let b = Extent::new(vec![4, 4]);
        assert_eq!(
            a.components().iter().product::<u128>(),
            b.components().iter().product::<u128>()
        );
        assert!(a.compare(&b).is_none());
        assert!(!a.fits_in(&b) && !b.fits_in(&a));

        // Survey of a multi-axis request returns a multi-component price
        // (galaxy or orbit), never a single folded scalar.
        let court = datum::ledger::authority().expect("authority");
        let app = apply("wide", vec![2, 8], 256);
        let proposal = datum::board::survey(&court, &app).expect("survey");
        assert!(
            proposal.price.axes() >= 2,
            "multi-axis grant must price per axis, got {:?}",
            proposal.price.components()
        );
    }

    #[test]
    fn multi_axis_credit_must_cover_every_axis_of_price() {
        let mut book = RewardBook::new();
        // 2-axis credit [1,1]
        book.credit_claim(&closed_box_claim(1, 3).encode())
            .expect("credit");
        assert!(book.covers(&Extent::new(vec![1, 1])));
        // Cannot cover a taller first axis
        assert!(!book.covers(&Extent::new(vec![2, 1])));
        // Arity mismatch is not cover
        assert!(!book.covers(&Extent::new(vec![1])));
    }

    #[test]
    fn underfunded_book_refuses_enact() {
        let court = datum::ledger::authority().expect("authority");
        let application = apply("thin", vec![8], 64);
        let proposal = datum::board::survey(&court, &application).expect("survey");
        let book = RewardBook::new();
        match settle::enact_if_funded(&book, &court, &proposal, &application.position) {
            Err(SettleRefused::Work(RewardRefused::Underfunded { .. })) => {}
            other => panic!("expected underfunded work, got {other:?}"),
        }
    }

    #[test]
    fn funded_boundary_work_enacts_join() {
        let court = datum::ledger::authority().expect("authority");
        let application = apply("coral", vec![8], 64);
        let mut book = RewardBook::new();
        for body in boundary_stack(8) {
            book.credit_claim(&body).expect("credit");
        }
        assert_eq!(book.total().components(), &[8]);
        let proposal = datum::board::survey(&court, &application).expect("survey");
        assert_eq!(proposal.price, Extent::new(vec![8]));
        let grown = settle::enact_if_funded(&book, &court, &proposal, &application.position)
            .expect("enact funded");
        assert!(grown
            .deeds()
            .iter()
            .any(|d| d.live && d.holder == "coral"));
    }

    #[test]
    fn shape_credit_is_standing_not_upsilon_token() {
        let mut book = RewardBook::new();
        let credit = book
            .credit_claim(&triangle_claim(0).encode())
            .expect("shape credit");
        assert!(credit.witness.is_none());
        assert_eq!(credit.axes.components(), &[1, 1, 1]);
        // standing stacks as multi-axis room
        assert!(book.covers(&Extent::new(vec![1, 1, 1])));
    }

    // ── H5 ─────────────────────────────────────────────────────────────

    #[test]
    fn land_appends_settlement_acts_as_a_block() {
        let court = datum::ledger::authority().expect("authority");
        let prior = court.acts().len();
        let application = apply("reef", vec![4], 64);
        let mut book = RewardBook::new();
        let bodies = boundary_stack(4);
        let refs: Vec<&[u8]> = bodies.iter().map(|b| b.as_slice()).collect();
        let (proposal, block, credits) =
            settle::land(&mut book, &court, &application, &refs).expect("land");
        assert_eq!(credits.len(), 4);
        assert!(!proposal.acts.is_empty());
        assert_eq!(block.batch_len(), proposal.acts.len());
        assert_eq!(block.prior_height, prior);
        assert_eq!(block.height, prior + proposal.acts.len());
        assert_eq!(block.landed_slice().len(), proposal.acts.len());
        // Court holds the estate
        assert!(block
            .court
            .deeds()
            .iter()
            .any(|d| d.live && d.holder == "reef"));
        // Second land of same applicant shape may still work if free space remains
        block.court.well_formed().expect("settled court well-formed");
    }

    #[test]
    fn empty_block_is_not_a_settlement() {
        let court = datum::ledger::authority().expect("authority");
        assert!(matches!(
            datum::block::produce(&court, vec![]),
            Err(BlockRefused::Empty)
        ));
    }

    #[test]
    fn join_with_work_stacks_and_enacts() {
        let court = datum::ledger::authority().expect("authority");
        let application = apply("lagoon", vec![4], 64);
        let mut book = RewardBook::new();
        let bodies = boundary_stack(4);
        let refs: Vec<&[u8]> = bodies.iter().map(|b| b.as_slice()).collect();
        let (_proposal, grown, credits) =
            settle::join_with_work(&mut book, &court, &application, &refs).expect("join");
        assert_eq!(credits.len(), 4);
        assert!(grown
            .deeds()
            .iter()
            .any(|d| d.live && d.holder == "lagoon"));
    }
}

mod negotiation {


    use datum::negotiation::{balance, comparable, Ask, Position};
    use isthmus::ratio::Exact;
    use num_bigint::BigInt;

    fn n(v: i64) -> Exact {
        Exact::from(BigInt::from(v))
    }

    fn q(numer: i64, denom: i64) -> Exact {
        Exact::new(BigInt::from(numer), BigInt::from(denom))
    }

    /// Deltas: (pole, amount) offers, the unit that crosses the wire.
    fn deltas() -> Vec<(&'static str, Exact)> {
        vec![
            ("convergence", n(3)),
            ("transition", q(7, 2)),
            ("convergence", n(5)), // raises the earlier offer
            ("colour", q(1, 3)),
            ("transition", n(2)), // lower than standing: absorbed, not a retreat
        ]
    }

    fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
        if items.is_empty() {
            return vec![Vec::new()];
        }
        let mut out = Vec::new();
        for at in 0..items.len() {
            let mut rest = items.to_vec();
            let head = rest.remove(at);
            for mut tail in permutations(&rest) {
                tail.insert(0, head.clone());
                out.push(tail);
            }
        }
        out
    }

    // ===================================================================
    // The merge laws: merge is commutative, associative, idempotent
    // ===================================================================

    #[test]
    fn merge_is_associative_commutative_idempotent() {
        let mut positions = Vec::new();
        for a in [0i64, 2, 5] {
            for b in [0i64, 3] {
                let mut p = Position::new();
                if a > 0 {
                    p.offer("convergence", n(a));
                }
                if b > 0 {
                    p.offer("transition", n(b));
                }
                positions.push(p);
            }
        }

        for a in &positions {
            for b in &positions {
                // Commutative.
                let mut ab = a.clone();
                ab.merge(b);
                let mut ba = b.clone();
                ba.merge(a);
                assert_eq!(ab, ba, "merge is not commutative");

                // Idempotent.
                let mut aa = a.clone();
                aa.merge(a);
                assert_eq!(&aa, a, "merge is not idempotent");

                for c in &positions {
                    // Associative.
                    let mut left = a.clone();
                    left.merge(b);
                    left.merge(c);
                    let mut bc = b.clone();
                    bc.merge(c);
                    let mut right = a.clone();
                    right.merge(&bc);
                    assert_eq!(left, right, "merge is not associative");
                }
            }
        }
    }

    // ===================================================================
    // The light-speed law: every arrival order, one position
    // ===================================================================

    /// All 120 orders of five deltas — with a duplication injected, since
    /// light speed also means retransmission — fold to the identical
    /// position and the identical balance.
    #[test]
    fn every_arrival_order_folds_to_the_same_position() {
        let mut ask = Ask::default();
        ask.demand("convergence", n(4));
        ask.demand("transition", n(3));

        let orders = permutations(&deltas());
        assert_eq!(orders.len(), 120);

        let mut folded = None;
        for order in orders {
            let mut position = Position::new();
            for (pole, amount) in &order {
                position.offer(pole, amount.clone());
            }
            // Retransmit the first delta of this order — a duplicate on
            // the wire must change nothing.
            if let Some((pole, amount)) = order.first() {
                position.offer(pole, amount.clone());
            }

            let b = balance(&position, &ask);
            match &folded {
                None => folded = Some((position, b)),
                Some((p0, b0)) => {
                    assert_eq!(&position, p0, "arrival order changed the position");
                    assert_eq!(&b, b0, "arrival order changed the balance");
                }
            }
        }
    }

    // ===================================================================
    // THE DESTRUCTION, measured: the scalar-boolean fold diverges
    // ===================================================================

    /// The old structure — a running scalar and a gate `sum >= price`
    /// firing the moment it holds — fed the SAME deltas in two orders.
    /// The verdict traces diverge: one order grants at delta two, the
    /// other at delta three, and the "amount at grant" differs. Two
    /// parties separated by delay each hold a defensible, different
    /// account of what was negotiated. That is the destruction, as a
    /// measurement rather than a warning.
    #[test]
    fn the_scalar_boolean_gate_diverges_under_reordering() {
        let price = 6i64;
        let stream_a = [n(3), n(4), n(2)]; // gate fires at index 1, sum 7
        let stream_b = [n(2), n(3), n(4)]; // gate fires at index 2, sum 9

        let trace = |stream: &[Exact]| -> (usize, Exact) {
            let mut sum = Exact::from(BigInt::from(0));
            for (at, delta) in stream.iter().enumerate() {
                sum += delta.clone();
                if sum >= n(price) {
                    return (at, sum); // the gate fires on ITS instant
                }
            }
            (usize::MAX, sum)
        };

        let (fired_a, granted_a) = trace(&stream_a);
        let (fired_b, granted_b) = trace(&stream_b);

        // Same multiset of deltas, different instants, different amounts
        // bound at the grant.
        assert_ne!(fired_a, fired_b, "the divergence must be real to matter");
        assert_ne!(granted_a, granted_b);

        // The position fold over the same two streams: identical, both
        // ways, because there is no instant to disagree about.
        let mut ask = Ask::default();
        ask.demand("convergence", n(price));
        let fold = |stream: &[Exact]| {
            let mut p = Position::new();
            for (at, delta) in stream.iter().enumerate() {
                // Deltas as standing offers: each is the party's total
                // offer so far from ITS own account, so reordering cannot
                // manufacture or lose value.
                let so_far: Exact = stream
                    .iter()
                    .take(at + 1)
                    .cloned()
                    .fold(Exact::from(BigInt::from(0)), |a, b| a + b);
                let _ = delta;
                p.offer("convergence", so_far);
            }
            balance(&p, &ask)
        };
        assert_eq!(fold(&stream_a).clears(), fold(&stream_b).clears());
    }

    // ===================================================================
    // Incomparability: short here, long there, and no order to force
    // ===================================================================

    /// A party short on one pole and long on another is **incomparable**
    /// with a cleared position — not below it. The board's answer is the
    /// counter naming both sides: the material of a trade, standing on the
    /// docket, refused by nothing.
    #[test]
    fn short_here_long_there_is_incomparable_and_counters_name_both_sides() {
        let mut ask = Ask::default();
        ask.demand("convergence", n(10));
        ask.demand("transition", n(2));

        let mut trader = Position::new();
        trader.offer("convergence", n(4)); // short 6
        trader.offer("transition", n(9)); // long 7

        let mut covered = Position::new();
        covered.offer("convergence", n(10));
        covered.offer("transition", n(2));

        let traded = balance(&trader, &ask);
        let cleared = balance(&covered, &ask);

        assert!(!traded.clears());
        assert!(cleared.clears());
        assert_eq!(
            comparable(&traded, &cleared),
            None,
            "short-here-long-there was totally ordered — a gate could then \
             hold it, and the ruling says it must not"
        );

        let counter = traded.counter().expect("a standing counter");
        assert_eq!(counter.short, vec![("convergence".to_string(), n(6))]);
        assert_eq!(counter.long, vec![("transition".to_string(), n(7))]);
    }

    // ===================================================================
    // The docket flow: counter, delta, fixpoint, land
    // ===================================================================

    /// End to end on the real court: the survey attaches the ask, the
    /// short position gets a counter (and the proposal DOES NOT DIE), a
    /// later delta merges, the fold clears, and enactment witnesses the
    /// fixpoint it never conducted.
    #[test]
    fn the_docket_holds_the_counter_until_the_position_clears() {
        let court = datum::ledger::authority().expect("no authority");

        let mut application = datum::board::Application {
            applicant: "patient".into(),
            shape: vec![8],
            position: Position::new(),
            witness: "held for #43".into(),
        };
        // The ask is per axis now, so the offer is too: the founding
        // court is a line, and its axis is named `tag`.
        application.position.offer(isthmus::layout::TAG, n(3));

        // Geometry answers regardless of funding: the proposal exists.
        let proposal = datum::board::survey(&court, &application).expect("space exists");
        assert_eq!(proposal.price, datum::extent::Extent::new(vec![8]));

        // The fold does not clear; the answer is a counter, not a death.
        let counter = datum::board::clears(&proposal, &application.position)
            .expect_err("3 against 8 cannot clear");
        assert_eq!(counter.short, vec![(isthmus::layout::TAG.to_string(), n(5))]);
        assert!(matches!(
            datum::board::enact(&court, &proposal, &application.position),
            Err(datum::board::EnactRefused::NotCleared(_))
        ));

        // A later delta arrives — in any order, from any path.
        application.position.offer(isthmus::layout::TAG, n(8));
        datum::board::clears(&proposal, &application.position).expect("the fixpoint");

        let grown = datum::board::enact(&court, &proposal, &application.position)
            .expect("witnessed, validated, landed");
        grown.well_formed().expect("lawful history");
        assert!(grown.deeds().iter().any(|d| d.live && d.holder == "patient"));
    }
}

mod block_production {

    use datum::block::{self, BlockRefused};
    use isthmus::deed::{Act, Ledger};
    use isthmus::layout::Layout;

    #[test]
    fn empty_block_refuses() {
        let court = Ledger::new(Layout::founding());
        assert!(matches!(
            block::produce(&court, vec![]),
            Err(BlockRefused::Empty)
        ));
    }

    #[test]
    fn observation_encumber_appends() {
        let court = Ledger::new(Layout::founding());
        let acts = vec![Act::Encumber {
            low: 10,
            high: 12,
            by: "foreign-mesh".into(),
            witnessed: "test".into(),
        }];
        let block = block::produce(&court, acts).expect("produce");
        assert_eq!(block.prior_height, 0);
        assert_eq!(block.height, 1);
        assert_eq!(block.batch_len(), 1);
        block.court.well_formed().expect("well formed");
        assert_eq!(block.landed_slice().len(), 1);
    }

    #[test]
    fn second_block_extends_height() {
        let court = Ledger::new(Layout::founding());
        let b1 = block::produce(
            &court,
            vec![Act::Encumber {
                low: 1,
                high: 1,
                by: "a".into(),
                witnessed: "t".into(),
            }],
        )
        .expect("b1");
        let b2 = block::produce(
            &b1.court,
            vec![Act::Encumber {
                low: 2,
                high: 2,
                by: "b".into(),
                witnessed: "t".into(),
            }],
        )
        .expect("b2");
        assert_eq!(b1.height, 1);
        assert_eq!(b2.prior_height, 1);
        assert_eq!(b2.height, 2);
    }

    #[test]
    fn settlement_acts_from_funded_join_are_a_block() {
        // H5 integration: settle::land produces a block whose acts match the proposal.
        use datum::negotiation::Position;
        use datum::reward::RewardBook;
        use datum::settle;
        use datum::board::Application;
        use isthmus::ratio::Exact;
        use num_bigint::BigInt;

        let court = datum::ledger::authority().expect("authority");
        let prior = court.acts().len();
        let mut position = Position::new();
        let amount = Exact::from(BigInt::from(64u32));
        position.offer(isthmus::layout::TAG, amount.clone());
        position.offer("block-d1", amount.clone());
        position.offer("nova-d1", amount);
        let application = Application {
            applicant: "block-holder".into(),
            shape: vec![3],
            position,
            witness: "work".into(),
        };
        let mut book = RewardBook::new();
        let mut bodies = Vec::new();
        for flux in 1..=3 {
            let mut b = assay::Boundary::new(1);
            let f = assay::whole(flux);
            assert!(b.face(assay::Facet::new(0, assay::Orientation::Low, f.clone())));
            assert!(b.face(assay::Facet::new(0, assay::Orientation::High, f)));
            bodies.push(assay::Claim::new(0, b).encode());
        }
        let refs: Vec<&[u8]> = bodies.iter().map(|x| x.as_slice()).collect();
        let (proposal, block, _) =
            settle::land(&mut book, &court, &application, &refs).expect("land");
        assert_eq!(block.acts, proposal.acts);
        assert!(block.height > prior);
        assert!(block
            .court
            .deeds()
            .iter()
            .any(|d| d.live && d.holder == "block-holder"));
    }

    #[test]
    fn onramp_foreign_tag_is_forwarded_by_carrier() {
        let mut wire = Vec::new();
        isthmus::frame::put_frame(
            &Layout::founding(),
            200,
            b"mesh-a-dialect-bytes",
            &mut wire,
        )
        .expect("put");
        match isthmus::node::carrier_step(&wire).expect("step") {
            isthmus::node::CarrierOut::Forward { whole } => {
                assert_eq!(whole, wire.as_slice());
            }
            isthmus::node::CarrierOut::Deliver { .. } => {
                panic!("tollway tag must forward on the superhighway carrier")
            }
        }
    }
}

mod hygiene {

    use datum::hygiene::{credit_effect, CreditEffectRefused, HygieneRefused, WireHygiene};
    use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};
    use isthmus::work;

    #[test]
    fn session_rule_is_not_a_seen_set() {
        // IS-2 §6.1: session step only separates wait / take / refuse by length.
        // No digest lives there.
        let body = closed_box_claim(0, 1).encode();
        let mut frame = Vec::new();
        work::put_shape_claim(&body, &mut frame).expect("frame");
        // Twice through the framing rule — both take.
        let bound = 1 << 20;
        for _ in 0..2 {
            match isthmus::session::step(&isthmus::layout::Layout::founding(), &frame, bound) {
                isthmus::session::Step::Take(n) => assert_eq!(n, frame.len()),
                other => panic!("session should take the frame, got {other:?}"),
            }
        }
    }

    #[test]
    fn authority_drops_identical_effect_wire_bytes() {
        let mut h = WireHygiene::new();
        let mut book = RewardBook::new();
        let body = triangle_claim(0).encode();
        credit_effect(&mut h, &mut book, &body).expect("first credit");
        match credit_effect(&mut h, &mut book, &body) {
            Err(CreditEffectRefused::Wire(HygieneRefused::WireReplay { .. })) => {}
            other => panic!("expected wire hygiene refuse, got {other:?}"),
        }
        // Book still has one credit only
        assert_eq!(book.total().components(), &[1, 1, 1]);
    }

    #[test]
    fn work_id_still_primary_when_wire_differs() {
        let mut h = WireHygiene::new();
        let mut book = RewardBook::new();
        credit_effect(&mut h, &mut book, &closed_box_claim(1, 2).encode()).expect("a");
        match credit_effect(&mut h, &mut book, &closed_box_claim(2, 2).encode()) {
            Err(CreditEffectRefused::Work(RewardRefused::Replay { .. })) => {}
            other => panic!("expected structure replay, got {other:?}"),
        }
    }

    #[test]
    fn framed_shape_body_peel_then_hygiene() {
        // Carrier delivers frame; court peels body; hygiene + book apply.
        let body = triangle_claim(5).encode();
        let mut wire = Vec::new();
        work::put_shape_claim(&body, &mut wire).expect("put");
        let (tag, value) = work::take_frame(&wire).expect("take");
        assert_eq!(tag, work::SHAPE_CLAIM_TAG);

        let mut h = WireHygiene::new();
        let mut book = RewardBook::new();
        credit_effect(&mut h, &mut book, value).expect("credit");
        assert!(h.would_replay(value));
        assert!(matches!(
            credit_effect(&mut h, &mut book, value),
            Err(CreditEffectRefused::Wire(_))
        ));
    }
}

mod workspace_hygiene {


    use std::path::{Path, PathBuf};

    /// The workspace root, from this crate's own manifest dir.
    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crates/datum has a workspace above it")
            .to_path_buf()
    }

    /// Every member manifest, found on disk rather than assumed.
    fn member_manifests() -> Vec<PathBuf> {
        let crates = workspace_root().join("crates");
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&crates).expect("crates/ exists") {
            let dir = entry.expect("readable entry").path();
            let manifest = dir.join("Cargo.toml");
            if manifest.is_file() {
                found.push(manifest);
            }
        }
        found.sort();
        found
    }

    /// `path = "…"` values inside dependency tables, with their manifest.
    fn path_values(manifest: &Path) -> Vec<String> {
        let text = std::fs::read_to_string(manifest).expect("manifest reads");
        let mut inside = false;
        let mut out = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with('[') {
                inside = line.contains("dependencies");
                continue;
            }
            if !inside || line.starts_with('#') {
                continue;
            }
            let mut rest = line;
            while let Some(at) = rest.find("path = \"") {
                let tail = &rest[at + "path = \"".len()..];
                if let Some(end) = tail.find('"') {
                    out.push(tail[..end].to_string());
                    rest = &tail[end..];
                } else {
                    break;
                }
            }
        }
        out
    }

    #[test]
    fn every_path_dependency_stays_inside_the_workspace() {
        let root = workspace_root()
            .canonicalize()
            .expect("workspace root resolves");
        for manifest in member_manifests() {
            let dir = manifest.parent().expect("manifest has a dir");
            for value in path_values(&manifest) {
                assert!(
                    !value.starts_with('/'),
                    "{} names an absolute path: {value}\n\
                     A path outside the workspace is a tree an outsider \
                     does not have.",
                    manifest.display()
                );
                let resolved = dir
                    .join(&value)
                    .canonicalize()
                    .unwrap_or_else(|_| panic!(
                        "{} names a path that does not exist: {value}",
                        manifest.display()
                    ));
                assert!(
                    resolved.starts_with(&root),
                    "{} escapes the workspace: {value} -> {}",
                    manifest.display(),
                    resolved.display()
                );
            }
        }
    }

    /// And the gate can fail: the walk finds manifests, and finds the
    /// sibling paths that are supposed to exist. Without this, a broken
    /// walker returns empty lists — which would read as *all clean*.
    #[test]
    fn the_walker_reads_the_workspace() {
        let manifests = member_manifests();
        assert!(
            manifests.len() >= 4,
            "expected at least four member crates, found {}",
            manifests.len()
        );
        let total: usize = manifests.iter().map(|m| path_values(m).len()).sum();
        assert!(
            total >= 3,
            "expected at least the datum->isthmus/assay and sdk->isthmus \
             path deps, found {total} — the parser is broken, and a broken \
             parser reports success"
        );
    }
}

mod http_quarantine {
    //! The quarantine, held by a test: HTTP exists ONLY in the
    //! gateway edge binary. No library a node links may contain one
    //! byte of it — a source scan in the tradition of assay's
    //! no-float read.

    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use std::path::{Path, PathBuf};

    const MARKERS: [&str; 3] = ["HTTP/1.1", "Content-Length", "Payment Required"];

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    }

    fn rust_sources(dir: &Path, exclude_bins: bool, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                if exclude_bins && path.file_name().is_some_and(|n| n == "bin") {
                    continue;
                }
                rust_sources(&path, exclude_bins, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn no_library_contains_one_byte_of_http() {
        let crates = workspace().join("crates");
        let mut sources = Vec::new();
        for entry in std::fs::read_dir(&crates).expect("crates/") {
            let src = entry.expect("entry").path().join("src");
            if src.is_dir() {
                rust_sources(&src, true, &mut sources);
            }
        }
        assert!(sources.len() > 20, "the scan actually walked the tree");
        for path in &sources {
            let text = std::fs::read_to_string(path).expect("reads");
            for marker in MARKERS {
                assert!(
                    !text.contains(marker),
                    "HTTP escaped the quarantine: {marker:?} in {}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn the_edge_holds_what_the_quarantine_confines() {
        // The scanner must be able to FIND the markers, or a silent
        // scan reads as a clean one — and the gateway must actually
        // be where HTTP lives.
        let gateway = workspace().join("crates/datum/src/bin/gateway.rs");
        let text = std::fs::read_to_string(gateway).expect("the edge exists");
        for marker in MARKERS {
            assert!(text.contains(marker), "the edge lost {marker:?}");
        }
    }
}

mod live_corpus {
    //! P5: the market a court actually prices and pays out for is
    //! real, citable mathematics (`datum::corpus`'s dihedral group of
    //! order 6) — never `demo_theta_universe`, the synthetic fixture
    //! that priced every market before this. A source scan in the
    //! same tradition as `http_quarantine`: the property is about
    //! what got WIRED, which only source, not a unit test of
    //! `corpus.rs` in isolation, can actually confirm.
    //!
    //! `demo_hexagon_*`/`demo_cycle_*` are NOT banned here — they
    //! remain legitimate synthetic TRAFFIC generators (a client's
    //! "fresh work every round," a witness's demo subject, a join's
    //! proof-of-life claim), honestly labeled as such. What must
    //! never be a fixture is the priced QUESTION itself.

    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use std::path::{Path, PathBuf};

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    }

    /// The part of a binary's source before its trailing `#[cfg(test)]`
    /// region — both `plumbd.rs` and `gateway.rs` put every test-only
    /// item at the end of the file, so this is exactly the live code.
    fn live_source(path: &Path) -> String {
        let text = std::fs::read_to_string(path).expect("the binary exists");
        match text.find("#[cfg(test)]") {
            Some(at) => text.get(..at).unwrap_or(&text).to_owned(),
            None => text,
        }
    }

    #[test]
    fn neither_binarys_live_path_prices_the_synthetic_fixture() {
        for relative in ["crates/datum/src/bin/plumbd.rs", "crates/datum/src/bin/gateway.rs"] {
            let path = workspace().join(relative);
            let live = live_source(&path);
            assert!(
                !live.contains("demo_theta_universe"),
                "{relative}'s live path still prices the demo fixture, not the real corpus"
            );
        }
    }

    #[test]
    fn the_live_market_is_the_dihedral_corpus_not_a_fixture() {
        // The scanner must be able to FIND the real corpus wired in,
        // or a scan that never matches anything reads as a clean one.
        let plumbd = workspace().join("crates/datum/src/bin/plumbd.rs");
        let live = live_source(&plumbd);
        assert!(
            live.contains("corpus::dihedral_conjecture"),
            "plumbd's live path should pose datum::corpus's real theorem"
        );
    }
}
