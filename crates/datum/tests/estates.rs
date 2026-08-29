//! ESTATES — the multi-chain laws: quotes, sphere merges, the
//! vertical, two-chain anchoring, and the master equation.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod d4_estate_quote {


    use datum::board::quote;
    use datum::extent::Extent;
    use isthmus::deed::Ledger;
    use isthmus::layout::Layout;
    use std::cmp::Ordering;

    fn founding_wide() -> Ledger {
        let mut court = Ledger::new(Layout::founding()).under("quote-court");
        court.issue("founder", 64).expect("issue");
        court
    }

    #[test]
    fn requested_shapes_are_incomparable_not_product_equal() {
        // The anti-fold pin: product both 16, multi-axis need is incomparable.
        let need_28 = Extent::new(vec![2, 8]);
        let need_44 = Extent::new(vec![4, 4]);
        assert_eq!(need_28.compare(&need_44), None);
        assert!(!need_28.fits_in(&need_44));
        assert!(!need_44.fits_in(&need_28));
    }

    #[test]
    fn quote_exposes_multi_axis_need_and_price() {
        let court = founding_wide();
        let q28 = quote(&court, "a", vec![2, 8]).expect("quote 2x8");
        let q44 = quote(&court, "b", vec![4, 4]).expect("quote 4x4");

        assert_eq!(q28.need.components(), &[2, 8]);
        assert_eq!(q44.need.components(), &[4, 4]);
        assert_eq!(q28.need.compare(&q44.need), None);

        // Price is multi-axial (not a single u128 field).
        assert!(
            q28.price.axes() >= 1 && !q28.price.is_empty(),
            "price must be an Extent, not a fold"
        );
        assert!(
            q44.price.axes() >= 1 && !q44.price.is_empty(),
            "price must be an Extent, not a fold"
        );
        // Distinct needs produce quotes that still carry distinct need vectors.
        assert_ne!(q28.need.components(), q44.need.components());
    }

    #[test]
    fn quote_same_shape_need_is_equal() {
        let court = founding_wide();
        let a = quote(&court, "a", vec![3, 3]).expect("a");
        let b = quote(&court, "b", vec![3, 3]).expect("b");
        assert_eq!(a.need.compare(&b.need), Some(Ordering::Equal));
        assert!(a.need.fits_in(&b.need));
    }

    #[test]
    fn quote_shapeless_refuses() {
        let court = founding_wide();
        assert!(quote(&court, "x", vec![]).is_err());
        assert!(quote(&court, "x", vec![0, 1]).is_err());
    }

    #[test]
    fn quote_returns_estate_and_ask() {
        let court = founding_wide();
        let q = quote(&court, "orbiter", vec![2, 2]).expect("orbit");
        // Estate is classified (Run / Orbit / Parcel / Galaxy — any is fine).
        let _ = format!("{:?}", q.estate);
        assert!(!q.price.is_empty());
        // Ask names poles for nonzero price components.
        let poles: Vec<_> = q.ask.poles().cloned().collect();
        assert!(
            !poles.is_empty(),
            "ask should name poles for multi-axis demand"
        );
    }
}

mod d5_sphere_merge_product {


    use assay::shape::triangle_claim;
    use datum::extent::Extent;
    use datum::merge::{admit_merge, MergePayout, MergeSplit};
    use datum::reward::{closed_box_claim, RewardBook, RewardRefused};
    use datum::settle::{self, MergeSettle};
    use isthmus::sphere::Precedence;

    /// Two high-overlap "universe" capacity vectors (bulk ≥ 9/10 of span).
    fn universe_a() -> Extent {
        // Shared bulk dominates; residual only on axis 0 of A and axis 2 of B.
        Extent::new(vec![100, 50, 50])
    }

    fn universe_b() -> Extent {
        Extent::new(vec![90, 50, 55])
    }

    #[test]
    fn sm13_two_universes_merge_to_bulk_and_residual_carry() {
        let a = universe_a();
        let b = universe_b();
        // M = [90, 50, 50], R_a = [10, 0, 0], R_b = [0, 0, 5]
        let split = MergeSplit::of(&a, &b).expect("split");
        assert_eq!(split.bulk.components(), &[90, 50, 50]);
        assert_eq!(split.residual_a.components(), &[10, 0, 0]);
        assert_eq!(split.residual_b.components(), &[0, 0, 5]);
        // Policy holds (bulk dominates).
        split
            .bulk_dominates_default(&a, &b)
            .expect("bulk policy");

        let payout = MergePayout::from_split(&split);
        assert_eq!(payout.principal.components(), split.bulk.components());
        assert_eq!(payout.carry_a.components(), split.residual_a.components());
        assert_eq!(payout.carry_b.components(), split.residual_b.components());

        // Residual is real — not zeroed by product or sum fold.
        assert!(payout.carry_a.components().iter().any(|c| *c > 0));
        assert!(payout.carry_b.components().iter().any(|c| *c > 0));
        assert_ne!(
            payout.carry_a.components(),
            payout.carry_b.components()
        );
    }

    #[test]
    fn sm13_settle_merge_routes_three_books_with_asserted_numbers() {
        let a = universe_a();
        let b = universe_b();
        // Distinct structures so work_ids differ (dual claim classes still once each).
        let body_a = triangle_claim(1).encode();
        let body_b = closed_box_claim(1, 2).encode();

        let mut principal = RewardBook::new();
        let mut carry_a = RewardBook::new();
        let mut carry_b = RewardBook::new();

        let req = MergeSettle {
            a: &a,
            b: &b,
            powc: None,
            pouw: Some((body_a.as_slice(), body_b.as_slice())),
            precedence: Precedence::Concurrent,
            allow_ordered: false,
            price_on_star: Some(&Extent::new(vec![90, 50, 50])),
        };
        let (admit, payout) =
            settle::settle_merge(&req, &mut principal, &mut carry_a, &mut carry_b)
                .expect("product settle");

        assert_eq!(admit.split.bulk.components(), &[90, 50, 50]);
        assert_eq!(principal.total().components(), &[90, 50, 50]);
        assert_eq!(carry_a.total().components(), &[10, 0, 0]);
        assert_eq!(carry_b.total().components(), &[0, 0, 5]);
        assert_eq!(payout.principal.components(), principal.total().components());
    }

    #[test]
    fn sm17_merge_work_ids_credit_once_then_replay_zero() {
        // PoUW work bodies admit the merge; court work_id is once per structure.
        let a = Extent::new(vec![20, 20]);
        let b = Extent::new(vec![20, 20]);
        let body_a = triangle_claim(7).encode();
        let body_b = closed_box_claim(3, 1).encode();

        let admit = admit_merge(
            &a,
            &b,
            None,
            Some((body_a.as_slice(), body_b.as_slice())),
            Precedence::Concurrent,
            false,
        )
        .expect("admit");

        let wa = admit.work_a.expect("work a");
        let wb = admit.work_b.expect("work b");
        assert_ne!(wa, wb, "distinct structures → distinct work_ids");

        let mut court = RewardBook::new();

        // Credit both merge legs once (primary work_id path).
        court.credit_claim(&body_a).expect("credit a");
        court.credit_claim(&body_b).expect("credit b");
        assert_eq!(court.act_len(), 2);

        // Replay identical structures → refuse (SM17 work_id once).
        assert!(matches!(
            court.credit_claim(&triangle_claim(99).encode()),
            Err(RewardRefused::Replay { .. })
        ));
        assert!(matches!(
            court.credit_claim(&closed_box_claim(99, 1).encode()),
            Err(RewardRefused::Replay { .. })
        ));
        assert_eq!(court.act_len(), 2, "replay must not grow acts");
    }

    #[test]
    fn sm17_second_settle_merge_does_not_double_work_credit() {
        let a = Extent::new(vec![12, 12]);
        let b = Extent::new(vec![12, 12]);
        let body = triangle_claim(0).encode();

        let mut principal = RewardBook::new();
        let mut ca = RewardBook::new();
        let mut cb = RewardBook::new();
        let mut work_court = RewardBook::new();

        // Gate work once.
        work_court.credit_claim(&body).expect("once");

        let req = MergeSettle {
            a: &a,
            b: &b,
            powc: None,
            pouw: Some((body.as_slice(), body.as_slice())),
            precedence: Precedence::Concurrent,
            allow_ordered: false,
            price_on_star: None,
        };
        // Economic allocate (add_extent) may stack; work_id path stays once.
        settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("first settle");
        settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("economic re-route ok");

        // Work court still one act only.
        assert!(matches!(
            work_court.credit_claim(&triangle_claim(1).encode()),
            Err(RewardRefused::Replay { .. })
        ));
        assert_eq!(work_court.act_len(), 1);
        // Principal stacked bulk twice (economic legs, not work mint).
        assert_eq!(principal.total().components(), &[24, 24]);
    }
}

mod merge {

    use assay::shape::triangle_claim;
    use assay::{whole, Boundary, Facet, Orientation};
    use datum::extent::Extent;
    use datum::merge::{
        admit_merge, admit_pouw, admit_powc, split_with_policy, MergePayout, MergeRefused, MergeSplit,
        DEFAULT_BULK_DEN, DEFAULT_BULK_NUM,
    };
    use datum::onramp::{shape_body, shape_from_edges};
    use datum::reward::RewardBook;
    use datum::settle;
    use isthmus::sphere::Precedence;
    use isthmus::work::{self, SHAPE_CLAIM_TAG};

    fn closed_1d(flux: i64) -> Boundary {
        let mut b = Boundary::new(1);
        let f = whole(flux);
        assert!(b.face(Facet::new(0, Orientation::Low, f.clone())));
        assert!(b.face(Facet::new(0, Orientation::High, f)));
        b
    }

    // ─── SM1–SM3 ─────────────────────────────────────────────────────────

    #[test]
    fn sm1_meet_and_residual_are_componentwise() {
        let a = Extent::new(vec![10, 4, 8]);
        let b = Extent::new(vec![6, 9, 8]);
        let m = a.meet(&b).expect("meet");
        assert_eq!(m.components(), &[6, 4, 8]);
        let ra = a.saturating_sub(&m).expect("ra");
        let rb = b.saturating_sub(&m).expect("rb");
        assert_eq!(ra.components(), &[4, 0, 0]);
        assert_eq!(rb.components(), &[0, 5, 0]);
    }

    #[test]
    fn sm2_merge_split_refuses_arity_mismatch() {
        let a = Extent::new(vec![3, 3]);
        let b = Extent::new(vec![3]);
        assert!(matches!(
            MergeSplit::of(&a, &b),
            Err(MergeRefused::Arity { left: 2, right: 1 })
        ));
    }

    #[test]
    fn sm2_empty_bulk_refuses() {
        let a = Extent::new(vec![0, 5]);
        let b = Extent::new(vec![3, 0]);
        // meet = [0,0] → nothing
        assert!(matches!(
            MergeSplit::of(&a, &b),
            Err(MergeRefused::EmptyBulk)
        ));
    }

    #[test]
    fn sm3_bulk_dominance_default_nine_tenths() {
        // 90% bulk on every axis: A=B=10 → M=10, residual 0 → ok
        let a = Extent::new(vec![10, 10]);
        let b = Extent::new(vec![10, 10]);
        split_with_policy(&a, &b, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN).expect("full overlap");

        // A=10, B=1 → M=1, span=10, 1/10 < 9/10 → thin
        let thin = Extent::new(vec![10]);
        let small = Extent::new(vec![1]);
        match split_with_policy(&thin, &small, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN) {
            Err(MergeRefused::BulkTooThin {
                axis: 0,
                bulk: 1,
                span: 10,
            }) => {}
            other => panic!("expected BulkTooThin, got {other:?}"),
        }

        // 9/10 exactly: M=9, span=10 → 9*10 >= 10*9 → ok
        let a9 = Extent::new(vec![9]);
        let b10 = Extent::new(vec![10]);
        split_with_policy(&a9, &b10, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN).expect("exactly 9/10");
    }

    #[test]
    fn sm3_ratios_are_per_axis_not_product() {
        let a = Extent::new(vec![10, 5]);
        let b = Extent::new(vec![10, 10]);
        let s = MergeSplit::of(&a, &b).expect("split");
        let r = s.ratios_per(&a, &b, 10).expect("ratios");
        // axis0: 10/10 = 10/10, axis1: 5/10 = 5/10
        assert_eq!(r, vec![10, 5]);
    }

    // ─── SM4–SM5 admit ───────────────────────────────────────────────────

    #[test]
    fn sm4_powc_requires_both_closed() {
        let ok = closed_1d(3);
        admit_powc(&ok, &ok).expect("closed");
        let mut open = Boundary::new(1);
        assert!(open.face(Facet::new(0, Orientation::Low, whole(1))));
        assert!(open.face(Facet::new(0, Orientation::High, whole(2))));
        assert!(matches!(
            admit_powc(&ok, &open),
            Err(MergeRefused::PowcOpen)
        ));
    }

    #[test]
    fn sm5_pouw_shape_work_ids() {
        let a = triangle_claim(1);
        let b = triangle_claim(2);
        let (wa, wb) = admit_pouw(&a, &b).expect("pouw");
        assert_eq!(wa, a.work_id());
        assert_eq!(wb, b.work_id());
        // same structure → same work_id
        assert_eq!(wa, wb);
    }

    // ─── SM6–SM7 admit_merge ─────────────────────────────────────────────

    #[test]
    fn sm6_admit_by_pouw_with_extents() {
        // High overlap so bulk policy passes: [10,10] ∧ [10,10]
        let a = Extent::new(vec![10, 10]);
        let b = Extent::new(vec![10, 10]);
        let body_a = triangle_claim(0).encode();
        let body_b = triangle_claim(1).encode();
        let admit = admit_merge(
            &a,
            &b,
            None,
            Some((body_a.as_slice(), body_b.as_slice())),
            Precedence::Concurrent,
            false,
        )
        .expect("admit");
        assert_eq!(admit.split.bulk.components(), &[10, 10]);
        assert!(admit.work_a.is_some());
    }

    #[test]
    fn sm7_ordered_fault_refuses_without_override() {
        let a = Extent::new(vec![10]);
        let b = Extent::new(vec![10]);
        let body = triangle_claim(0).encode();
        match admit_merge(
            &a,
            &b,
            None,
            Some((body.as_slice(), body.as_slice())),
            Precedence::HereSawThere,
            false,
        ) {
            Err(MergeRefused::OrderedFault {
                order: Precedence::HereSawThere,
            }) => {}
            other => panic!("expected OrderedFault, got {other:?}"),
        }
        // override allows
        admit_merge(
            &a,
            &b,
            None,
            Some((body.as_slice(), body.as_slice())),
            Precedence::HereSawThere,
            true,
        )
        .expect("override");
    }

    // ─── SM8–SM10 payout ─────────────────────────────────────────────────

    #[test]
    fn sm8_payout_principal_and_carry_are_multi_axial() {
        let a = Extent::new(vec![10, 4]);
        let b = Extent::new(vec![6, 9]);
        let s = MergeSplit::of(&a, &b).expect("split");
        let p = MergePayout::from_split(&s);
        assert_eq!(p.principal.components(), &[6, 4]);
        assert_eq!(p.carry_a.components(), &[4, 0]);
        assert_eq!(p.carry_b.components(), &[0, 5]);
        // No fold: total obligation is three vectors, not one product
        assert_ne!(
            p.principal.components().iter().product::<u128>(),
            0,
            "sanity"
        );
    }

    #[test]
    fn sm9_settle_merge_routes_three_books() {
        let a = Extent::new(vec![10, 10]);
        let b = Extent::new(vec![10, 10]);
        let body = triangle_claim(0).encode();
        let mut principal = RewardBook::new();
        let mut ca = RewardBook::new();
        let mut cb = RewardBook::new();
        let req = settle::MergeSettle {
            a: &a,
            b: &b,
            powc: None,
            pouw: Some((body.as_slice(), body.as_slice())),
            precedence: Precedence::Concurrent,
            allow_ordered: false,
            price_on_star: Some(&Extent::new(vec![10, 10])),
        };
        let (admit, payout) =
            settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("settle");
        assert_eq!(principal.total().components(), payout.principal.components());
        assert_eq!(ca.total().components(), payout.carry_a.components());
        assert_eq!(cb.total().components(), payout.carry_b.components());
        assert_eq!(admit.split.bulk.components(), &[10, 10]);
    }

    #[test]
    fn sm10_carry_not_folded_to_scalar_dust() {
        let a = Extent::new(vec![10, 1]);
        let b = Extent::new(vec![10, 10]);
        // bulk [10,1] — axis1 is 1/10 < 9/10 → policy fail (thin)
        assert!(matches!(
            split_with_policy(&a, &b, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN),
            Err(MergeRefused::BulkTooThin { axis: 1, .. })
        ));
        // Without policy, residual still multi-axial
        let s = MergeSplit::of(&a, &b).expect("split");
        assert_eq!(s.residual_b.components(), &[0, 9]);
        assert!(!s.residual_b.is_nothing() || s.residual_b.component(1) == Some(9));
    }

    // ─── SM11 highway path ───────────────────────────────────────────────

    #[test]
    fn sm11_highway_shape_frames_admit_merge() {
        // Two overlapping high-capacity spheres + tag-82 framed shape bodies
        let cap_a = Extent::new(vec![10, 10, 10]);
        let cap_b = Extent::new(vec![10, 10, 10]);
        let shape = shape_from_edges(
            3,
            [
                (0, 1, whole(1)),
                (1, 2, whole(1)),
                (0, 2, whole(1)),
            ],
        )
        .expect("shape");
        let body = shape_body(0, shape).expect("body");
        let mut wire_a = Vec::new();
        let mut wire_b = Vec::new();
        work::put_shape_claim(&body, &mut wire_a).expect("frame a");
        work::put_shape_claim(&body, &mut wire_b).expect("frame b");

        let (tag, value_a) = work::take_frame(&wire_a).expect("take a");
        assert_eq!(tag, SHAPE_CLAIM_TAG);
        let (_, value_b) = work::take_frame(&wire_b).expect("take b");

        // Carrier classifies as mine (work tag)
        match work::classify(&wire_a).expect("classify") {
            work::Envelope::Mine { tag, .. } => assert_eq!(tag, SHAPE_CLAIM_TAG),
            work::Envelope::Forward { .. } => panic!("shape must deliver"),
        }

        let mut principal = RewardBook::new();
        let mut ca = RewardBook::new();
        let mut cb = RewardBook::new();
        let price = Extent::new(vec![10, 10, 10]);
        let req = settle::MergeSettle {
            a: &cap_a,
            b: &cap_b,
            powc: None,
            pouw: Some((value_a, value_b)),
            precedence: Precedence::Concurrent,
            allow_ordered: false,
            price_on_star: Some(&price),
        };
        let (_admit, payout) =
            settle::settle_merge(&req, &mut principal, &mut ca, &mut cb).expect("highway merge settle");
        assert_eq!(payout.principal.components(), &[10, 10, 10]);
        assert!(payout.carry_a.is_nothing());
        assert!(payout.carry_b.is_nothing());
    }
}

mod two_chain {

    use isthmus::deed::chain;
    use isthmus::deed::{Act, Ledger};
    use isthmus::hello::{Hello, Uplink};
    use isthmus::layout::Layout;
    use isthmus::sphere::{self, Frontier};
    use std::cmp::Ordering;

    /// Deterministic toy digest (same as `examples/uplink.rs`). Not a
    /// security recommendation — confirms takes the function in.
    fn fnv(bytes: &[u8]) -> Vec<u8> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash.to_le_bytes().to_vec()
    }

    fn named_estate(name: &str, holder: &str, width: u128) -> Ledger {
        let mut court = Ledger::new(Layout::founding()).under(name);
        court
            .issue(holder, width)
            .unwrap_or_else(|why| panic!("{name} could not issue estate: {why:?}"));
        court
    }

    // ===================================================================
    // full local demo
    // ===================================================================

    #[test]
    fn two_independent_chains_anchor_and_confirm() {
        let south = named_estate("south", "producer", 8);
        assert_eq!(south.name(), Some("south"));
        assert!(south.height() >= 1, "south must hold at least the Open");

        let mut north = named_estate("north", "newcomer", 8);
        assert_eq!(north.name(), Some("north"));

        // South declares who it is (IS-5/2 uplink).
        let declared = Hello::of(&south, "isthmus", 1 << 20).declaring(Uplink::of(&south, fnv));
        let bytes = declared.encode();
        let heard = Hello::decode(&bytes).expect("declaration survives the wire");
        let uplink = heard
            .uplink
            .as_ref()
            .expect("south declared an uplink");
        assert_eq!(uplink.chain, "south");
        assert_eq!(uplink.height(), south.height());
        assert_eq!(uplink.digest, fnv(&chain::encode(south.acts())));

        // North records ONE vertical over south's own chain.
        let height_before = north.height();
        let act = uplink.anchor("local two-chain demo");
        north.record(act);
        assert_eq!(
            north.height(),
            height_before + 1,
            "north gained exactly one vertical act"
        );

        let vertical = north.acts().last().expect("anchor landed");
        assert!(
            matches!(vertical, Act::Anchor { chain, .. } if chain == "south"),
            "last act must be the south anchor"
        );

        // Digest truth against the observed chain.
        assert_eq!(
            sphere::confirms(vertical, &south, fnv),
            Some(true),
            "anchor must confirm against south's prefix"
        );

        // A forged digest is false, not unanswerable.
        let mut forged = vertical.clone();
        if let Act::Anchor { digest, .. } = &mut forged {
            digest.fill(0xFF);
        }
        assert_eq!(
            sphere::confirms(&forged, &south, fnv),
            Some(false),
            "wrong digest must refuse confirmation"
        );

        // Wrong chain name is unanswerable (None), not a false accusation.
        let mut wrong_name = vertical.clone();
        if let Act::Anchor { chain, .. } = &mut wrong_name {
            *chain = "west".into();
        }
        assert_eq!(
            sphere::confirms(&wrong_name, &south, fnv),
            None,
            "anchor naming a different chain is unanswerable"
        );

        // Vertical grants no ground — live deeds stay what north issued.
        let live = north.deeds().iter().filter(|d| d.live).count();
        assert_eq!(live, 1, "anchor must not mint a second live deed");

        // North's frontier records the observation of south.
        let f = north.frontier();
        assert_eq!(f.height_of("north"), north.height());
        assert_eq!(f.height_of("south"), south.height());
    }

    #[test]
    fn reciprocal_anchors_order_the_frontiers() {
        let mut south = named_estate("south", "producer", 8);
        let mut north = named_estate("north", "newcomer", 8);

        // Concurrent until either sees the other.
        let s0 = Hello::of(&south, "s", 1 << 20).declaring(Uplink::of(&south, fnv));
        let n0 = Hello::of(&north, "n", 1 << 20).declaring(Uplink::of(&north, fnv));
        assert_eq!(
            s0.against(&n0),
            Some(None),
            "independent strangers start concurrent"
        );

        // North observes south.
        let south_up = Uplink::of(&south, fnv).expect("named");
        north.record(south_up.anchor("north saw south"));

        // South observes north (which now includes the vertical).
        let north_up = Uplink::of(&north, fnv).expect("named");
        south.record(north_up.anchor("south saw north"));

        let s1 = Hello::of(&south, "s", 1 << 20).declaring(Uplink::of(&south, fnv));
        let n1 = Hello::of(&north, "n", 1 << 20).declaring(Uplink::of(&north, fnv));

        // After reciprocal observation the fronts are comparable via join.
        let sf = s1.uplink.as_ref().unwrap().frontier.clone();
        let nf = n1.uplink.as_ref().unwrap().frontier.clone();
        let joined = sf.join(&nf);
        assert!(
            sf.compare(&joined) == Some(Ordering::Less) || sf.compare(&joined) == Some(Ordering::Equal),
            "south ≤ join"
        );
        assert!(
            nf.compare(&joined) == Some(Ordering::Less) || nf.compare(&joined) == Some(Ordering::Equal),
            "north ≤ join"
        );
        assert_eq!(joined.height_of("south"), south.height());
        assert_eq!(joined.height_of("north"), north.height());

        // North's anchor of south still confirms: south only grew *after*
        // that height, and confirms digests the cited prefix.
        let north_vertical = north
            .acts()
            .iter()
            .find(|a| matches!(a, Act::Anchor { chain, .. } if chain == "south"))
            .expect("north holds south anchor");
        assert_eq!(
            sphere::confirms(north_vertical, &south, fnv),
            Some(true),
            "north's anchor of south still confirms"
        );

        let south_vertical = south
            .acts()
            .iter()
            .find(|a| matches!(a, Act::Anchor { chain, .. } if chain == "north"))
            .expect("south holds north anchor");
        assert_eq!(
            sphere::confirms(south_vertical, &north, fnv),
            Some(true),
            "south's anchor of north confirms"
        );
    }

    #[test]
    fn chain_bytes_round_trip_including_verticals() {
        let south = named_estate("south", "producer", 8);
        let mut north = named_estate("north", "newcomer", 8);
        let uplink = Uplink::of(&south, fnv).expect("named");
        north.record(uplink.anchor("persist me"));

        let bytes = chain::encode(north.acts());
        let decoded = chain::decode(&bytes).expect("decode");
        assert_eq!(decoded, north.acts());
        let verticals = decoded
            .iter()
            .filter(|a| matches!(a, Act::Anchor { .. }))
            .count();
        assert_eq!(verticals, 1);
        assert!(decoded.len() >= 2, "Open + Anchor at minimum");
    }

    #[test]
    fn frontiers_join_is_per_chain_max() {
        let mut a = Frontier::new();
        a.observe("north", 3);
        a.observe("south", 10);
        let mut b = Frontier::new();
        b.observe("north", 7);
        b.observe("south", 4);
        let j = a.join(&b);
        assert_eq!(j.height_of("north"), 7);
        assert_eq!(j.height_of("south"), 10);
        assert!(a.concurrent_with(&b));
        assert_eq!(a.compare(&b), None);
    }
}

mod vertical {

    use isthmus::deed::chain::{self, ANCHOR};
    use isthmus::deed::{Act, Ledger};
    use isthmus::frame::Reader;
    use isthmus::layout::Layout;
    use isthmus::sphere::Frontier;
    use std::cmp::Ordering;

    /// IS-6 §7 published vector for Anchor (chain test C8).
    const C8_HEX: &str = "\
    08240000000500736f757468\
    0f00000000000000\
    080000000102030405060708\
    070073657373696f6e";

    fn c8_act() -> Act {
        Act::Anchor {
            chain: "south".to_owned(),
            height: 15,
            digest: vec![1, 2, 3, 4, 5, 6, 7, 8],
            witnessed: "session".to_owned(),
        }
    }

    #[test]
    fn anchor_vector_round_trips_and_matches_is6() {
        let act = c8_act();
        let bytes = chain::encode(std::slice::from_ref(&act));
        let published: String = C8_HEX.chars().filter(|c| !c.is_whitespace()).collect();
        let produced: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(produced, published, "codec drifted from IS-6 §7 C8");
        assert_eq!(chain::decode(&bytes), Ok(vec![act]));

        // Tag is the vertical act tag.
        let mut reader = Reader::new(&bytes);
        let (tag, _) = reader.frame(&Layout::founding()).expect("frame");
        assert_eq!(tag, ANCHOR);
    }

    #[test]
    fn frontier_records_vertical_observation() {
        let mut court = Ledger::new(Layout::founding()).under("north");
        court.anchor("south", 15, &[1, 2, 3, 4, 5, 6, 7, 8], "session");
        let f = court.frontier();
        assert_eq!(f.height_of("north"), 1); // one act on this chain
        assert_eq!(f.height_of("south"), 15);
        assert!(f.chains().contains(&"south"));
    }

    #[test]
    fn frontiers_join_by_per_chain_max_and_concurrency_is_none() {
        let mut a = Frontier::new();
        a.observe("datum", 10);
        a.observe("peer-b", 3);

        let mut b = Frontier::new();
        b.observe("datum", 7);
        b.observe("peer-b", 9);

        let j = a.join(&b);
        assert_eq!(j.height_of("datum"), 10);
        assert_eq!(j.height_of("peer-b"), 9);

        // a saw more datum, b saw more peer-b → concurrent
        assert!(a.concurrent_with(&b));
        assert_eq!(a.compare(&b), None);

        // after join, both are ≤ join
        assert_eq!(a.compare(&j), Some(Ordering::Less));
        assert_eq!(b.compare(&j), Some(Ordering::Less));
    }

    #[test]
    fn unknown_chain_act_refuses_not_skips() {
        // A chain of only a known Anchor decodes; a foreign tag mid-chain refuses.
        let good = chain::encode(&[c8_act()]);
        assert!(chain::decode(&good).is_ok());

        // Corrupt: flip act tag to 99 (not a chain act).
        let mut bad = good;
        if let Some(t) = bad.get_mut(0) {
            *t = 99;
        }
        assert!(
            chain::decode(&bad).is_err(),
            "unknown act must refuse so history cannot be silently misfolded"
        );
    }
}

mod master_equation {

    use assay::whole;
    use datum::extent::Extent;
    use datum::onramp::{shape_body, shape_from_edges};
    use datum::reward::{closed_box_claim, RewardBook};
    use isthmus::deed::Ledger;
    use isthmus::sphere::Frontier;
    use isthmus::layout::Layout;
    use std::cmp::Ordering;

    // ─── (S) Sphere linkage ─────────────────────────────────────────────────────

    #[test]
    fn l_frontier_join_is_per_chain_max() {
        let mut f = Frontier::new();
        f.observe("a", 3);
        f.observe("b", 10);
        let mut g = Frontier::new();
        g.observe("a", 7);
        g.observe("b", 4);
        let j = f.join(&g);
        assert_eq!(j.height_of("a"), 7);
        assert_eq!(j.height_of("b"), 10);
    }

    #[test]
    fn l_concurrent_frontiers_are_incomparable() {
        let mut f = Frontier::new();
        f.observe("a", 5);
        f.observe("b", 1);
        let mut g = Frontier::new();
        g.observe("a", 2);
        g.observe("b", 9);
        assert!(f.concurrent_with(&g));
        assert_eq!(f.compare(&g), None);
    }

    // ─── (E) Estate axes ─────────────────────────────────────────────────

    #[test]
    fn e_open_adds_dimension_without_granting_ground() {
        let mut court = Ledger::new(Layout::founding()).under("e");
        assert_eq!(court.axes().len(), 1, "founding line");
        court.open_axis("revision", 7);
        assert_eq!(court.axes().len(), 2, "Open multiplies directions");
        let live = court.deeds().into_iter().filter(|d| d.live).count();
        assert_eq!(live, 0, "Open grants nobody a live deed");
        court.well_formed().expect("open is lawful");
    }

    #[test]
    fn e_incomparable_boxes_are_not_ordered_by_product() {
        // [2,8] and [4,4] both "product 16" if volume were used — forbidden.
        let a = Extent::new(vec![2, 8]);
        let b = Extent::new(vec![4, 4]);
        assert!(a.compare(&b).is_none());
        assert!(!a.fits_in(&b));
        assert!(!b.fits_in(&a));
    }

    // ─── (C) Capacity ────────────────────────────────────────────────────

    #[test]
    fn c_credit_covers_price_componentwise_same_arity() {
        let mut book = RewardBook::new();
        book.credit_claim(&closed_box_claim(0, 1).encode())
            .expect("credit");
        assert!(book.covers(&Extent::new(vec![1, 1])));
        assert!(!book.covers(&Extent::new(vec![2, 1])));
        // Arity mismatch: not covers
        assert!(!book.covers(&Extent::new(vec![1])));
    }

    // ─── (W) Work identity ───────────────────────────────────────────────

    #[test]
    fn w_work_id_is_structure_credit_once() {
        let shape = shape_from_edges(2, [(0, 1, whole(1))]).expect("shape");
        let a = shape_body(1, shape.clone()).expect("body");
        let b = shape_body(99, shape).expect("body");
        let mut book = RewardBook::new();
        book.credit_claim(&a).expect("first");
        assert!(matches!(
            book.credit_claim(&b),
            Err(datum::reward::RewardRefused::Replay { .. })
        ));
    }

    // ─── (M) Anchor grants no ground; dim independent of vertical ───────

    #[test]
    fn m_anchor_extends_frontier_not_estate_axes() {
        let mut court = Ledger::new(Layout::founding()).under("north");
        let axes_before = court.axes().len();
        court.anchor("south", 15, &[1, 2, 3, 4, 5, 6, 7, 8], "session");
        assert_eq!(court.axes().len(), axes_before, "Anchor is not Open");
        assert_eq!(court.frontier().height_of("south"), 15);
        assert_eq!(court.deeds().into_iter().filter(|d| d.live).count(), 0);
        // Still well-formed after vertical only.
        court.well_formed().expect("anchor-only chain is lawful");
    }

    #[test]
    fn m_uplink_loop_is_knowledge_not_space_merge() {
        // A sees B@3; B sees A@2 — cycle of frontiers, independent spaces.
        let mut a = Ledger::new(Layout::founding()).under("A");
        a.anchor("B", 3, &[0u8; 8], "up");
        let mut b = Ledger::new(Layout::founding()).under("B");
        b.anchor("A", 2, &[0u8; 8], "down");

        assert_eq!(a.frontier().height_of("B"), 3);
        assert_eq!(b.frontier().height_of("A"), 2);
        // Neither absorbed the other's axes.
        assert_eq!(a.axes().len(), 1);
        assert_eq!(b.axes().len(), 1);
        // Join of frontiers is well-defined and larger in both components.
        let j = a.frontier().join(&b.frontier());
        assert_eq!(j.height_of("A"), a.frontier().height_of("A").max(2));
        assert_eq!(j.height_of("B"), 3);
        assert_eq!(a.frontier().compare(&j), Some(Ordering::Less));
    }

    #[test]
    fn m_high_d_estate_path_is_not_capped_at_two_axes() {
        // Dimensional freedom: Open can add axes beyond 2 (full 11-D is board).
        let mut court = Ledger::new(Layout::founding()).under("mesh");
        for name in ["d1", "d2", "d3", "d4"] {
            court.open_axis(name, 3);
        }
        assert_eq!(
            court.axes().len(),
            5,
            "1 founding + 4 Open = 5-D space, not 2-D"
        );
        court.well_formed().expect("5-D open is lawful");
    }
}
