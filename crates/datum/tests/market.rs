//! THE MARKET — every economic-layer measurement in one binary:
//! declared-domain credit, chain-registered universes, the lemma
//! market, receipts and their conformance pins, the yield rebate,
//! and the refinement market. One binary instead of seven.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod common;

mod declared_domain {


    use assay::complex::DeclaredClaim;
    
    use datum::reward::{RewardBook, RewardRefused};

    fn hexagon(transport: u64) -> DeclaredClaim {
        datum::domains::demo_cycle_claim(6, transport)
    }

    #[test]
    fn a_declared_closure_credits_once_across_transports() {
        let mut book = RewardBook::new();
        let credit = book
            .credit_claim(&hexagon(1).encode())
            .expect("the hexagon closes and credits");
        assert_eq!(
            credit.axes.components(),
            &[6, 6],
            "multi-axial: one component per declared dimension"
        );

        // The same structure under a different transport is the same work.
        match book.credit_claim(&hexagon(2).encode()) {
            Err(RewardRefused::Replay { .. }) => {}
            other => panic!("expected replay, got {other:?}"),
        }
    }

    #[test]
    fn an_open_declared_chain_earns_nothing() {
        let mut open = hexagon(1);
        open.witness.pop();
        let mut book = RewardBook::new();
        assert!(
            book.credit_claim(&open.encode()).is_err(),
            "claims that do not close earn nothing — same law, new domain"
        );
        assert_eq!(book.act_len(), 0);
    }

}

mod domains {


    use assay::complex::{ComplexBroken, DeclaredClaim, DeclaredComplex, Entry, DEFAULT_FUEL};
    use assay::whole;
    use datum::domains::{self, DomainRefused};
    use datum::extent::Extent;
    use isthmus::deed::{Act, Ledger};
    use isthmus::layout::Layout;

    /// The hexagon universe — the library's own fixture.
    fn hexagon_universe() -> DeclaredComplex {
        datum::domains::demo_hexagon_universe()
    }

    fn cycle_claim(universe: &DeclaredComplex, transport: u64) -> DeclaredClaim {
        DeclaredClaim {
            transport,
            complex: universe.clone(),
            dim: 1,
            witness: (0..6).map(|i| (i, whole(1))).collect(),
        }
    }

    /// An edge where "geometer" holds a range and registered the hexagon
    /// universe on its low tag.
    fn registered_edge() -> (Ledger, u64) {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger.issue("geometer", 16).expect("room");
        let tag = ledger
            .deeds()
            .into_iter()
            .find(|d| d.live && d.holder == "geometer")
            .expect("issued")
            .low();
        ledger.record(Act::Declare {
            holder: "geometer".into(),
            tag,
            definition: hexagon_universe().encode(),
        });
        (ledger, tag)
    }

    #[test]
    fn a_court_learns_a_discipline_from_the_chain_alone() {
        let (ledger, tag) = registered_edge();
        let body = cycle_claim(&hexagon_universe(), 1).encode();
        let spent = domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL)
            .expect("the registered universe judges the claim — no rebuild");
        assert!(spent > 0, "and the judging had a measured price");
    }

    #[test]
    fn an_unregistered_tag_cannot_be_judged() {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger.issue("geometer", 16).expect("room");
        let body = cycle_claim(&hexagon_universe(), 1).encode();
        assert_eq!(
            domains::verify_registered(&ledger, 200, &body, DEFAULT_FUEL),
            Err(DomainRefused::Unregistered)
        );
    }

    #[test]
    fn the_wrong_universe_refuses() {
        let (ledger, tag) = registered_edge();
        // A triangle universe — closes fine, but it is not what the chain
        // registered for this tag.
        let n = 3u32;
        let mut op = Vec::new();
        for i in 0..n {
            let (source, target) = (i, (i + 1) % n);
            let mut pair = vec![
                Entry { row: target, col: i, coeff: whole(1) },
                Entry { row: source, col: i, coeff: whole(-1) },
            ];
            pair.sort_by_key(|e| (e.col, e.row));
            op.extend(pair);
        }
        let triangle = DeclaredComplex { cells: vec![n, n], ops: vec![op] };
        let body = DeclaredClaim {
            transport: 1,
            complex: triangle,
            dim: 1,
            witness: (0..3).map(|i| (i, whole(1))).collect(),
        }
        .encode();
        assert_eq!(
            domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL),
            Err(DomainRefused::WrongUniverse),
            "closing in your own private geometry proves nothing here"
        );
    }

    #[test]
    fn a_definition_lapses_with_its_grant() {
        let (mut ledger, tag) = registered_edge();
        ledger.record(Act::Retire {
            holder: "geometer".into(),
        });
        let body = cycle_claim(&hexagon_universe(), 1).encode();
        assert_eq!(
            domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL),
            Err(DomainRefused::Unregistered),
            "a vocabulary does not outlive its grant"
        );
    }

    #[test]
    fn a_later_declaration_supersedes_and_registration_is_not_trust() {
        let (mut ledger, tag) = registered_edge();
        // The holder republishes garbage over its own tag. Registration
        // succeeds — the chain records speech — and judgment refuses.
        ledger.record(Act::Declare {
            holder: "geometer".into(),
            tag,
            definition: vec![9, 9, 9],
        });
        let body = cycle_claim(&hexagon_universe(), 1).encode();
        assert!(matches!(
            domains::verify_registered(&ledger, tag, &body, DEFAULT_FUEL),
            Err(DomainRefused::BadDefinition(_)),

        ));
    }

    // ── UC6: fuel is a priced budget ────────────────────────────────────

    #[test]
    fn an_over_budget_evaluation_refuses_with_the_price_named() {
        let (ledger, tag) = registered_edge();
        let body = cycle_claim(&hexagon_universe(), 1).encode();

        // The space priced 10 units of fuel on its fuel axis.
        let price = Extent::new(vec![6, 10]);
        let budget = domains::fuel_budget(&price, 1);
        assert_eq!(budget, 10);

        match domains::verify_registered(&ledger, tag, &body, budget) {
            Err(DomainRefused::Broken(ComplexBroken::FuelExhausted { budget })) => {
                assert_eq!(budget, 10, "the refusal names the priced budget");
            }
            other => panic!("expected a priced fuel refusal, got {other:?}"),
        }

        // An unpriced space grants nothing at all.
        assert_eq!(domains::fuel_budget(&Extent::new(vec![6]), 1), 0);
    }
}

mod proof_market {


    use assay::complex::{DeclaredComplex, Entry, ProofClaim};
    use assay::whole;
    use datum::reward::{RewardBook, RewardRefused};

    fn path(n: u32) -> DeclaredComplex {
        let mut op = Vec::new();
        for i in 0..n {
            op.push(Entry { row: i, col: i, coeff: whole(-1) });
            op.push(Entry { row: i + 1, col: i, coeff: whole(1) });
        }
        DeclaredComplex { cells: vec![n + 1, n], ops: vec![op] }
    }

    /// A derivation from v0 to v_n over the path universe.
    fn proof(n: u32, transport: u64, deps: Vec<Vec<u8>>) -> ProofClaim {
        ProofClaim {
            transport,
            complex: path(n),
            dim: 1,
            target: vec![(0, whole(-1)), (n, whole(1))],
            witness: (0..n).map(|i| (i, whole(1))).collect(),
            deps,
        }
    }

    #[test]
    fn the_lemma_market_credits_in_citation_order() {
        let mut book = RewardBook::new();

        // The lemma: a 2-step derivation, settled first.
        let lemma = proof(2, 1, Vec::new());
        let lemma_id = lemma.work_id();
        book.credit_claim(&lemma.encode()).expect("the lemma settles");

        // The theorem: a 5-step derivation standing on the lemma.
        let theorem = proof(5, 1, vec![lemma_id.as_bytes().to_vec()]);
        let credit = book
            .credit_claim(&theorem.encode())
            .expect("a theorem on settled ground credits");
        assert_eq!(credit.axes.components(), &[6, 5]);
    }

    #[test]
    fn citing_the_unsettled_refuses_by_address() {
        let mut book = RewardBook::new();
        let phantom = proof(2, 1, Vec::new()).work_id();
        let theorem = proof(5, 1, vec![phantom.as_bytes().to_vec()]);
        match book.credit_claim(&theorem.encode()) {
            Err(RewardRefused::UnsettledDependency { work_id }) => {
                assert_eq!(work_id, phantom, "the refusal names the missing lemma");
            }
            other => panic!("expected UnsettledDependency, got {other:?}"),
        }

        // Settle the lemma; the same theorem now credits: citation order
        // is settlement order, not submission order.
        book.credit_claim(&proof(2, 9, Vec::new()).encode())
            .expect("the lemma settles under any transport");
        book.credit_claim(&theorem.encode())
            .expect("and the theorem follows it");
    }


    #[test]
    fn a_broken_derivation_earns_nothing_even_on_settled_ground() {
        let mut book = RewardBook::new();
        let lemma = proof(2, 1, Vec::new());
        let lemma_id = lemma.work_id();
        book.credit_claim(&lemma.encode()).expect("settles");
        let mut gappy = proof(5, 1, vec![lemma_id.as_bytes().to_vec()]);
        gappy.witness.remove(2);
        assert!(matches!(
            book.credit_claim(&gappy.encode()),
            Err(RewardRefused::OpenWork)
        ));
        assert_eq!(book.act_len(), 1, "citations license nothing by themselves");
    }
}

mod receipts {


    use datum::query::{Guarantee, Query, QueryBroken};
    use datum::receipt::{self, ReceiptRefused};
    use datum::reward::RewardBook;
    use isthmus::deed::Ledger;
    use isthmus::layout::Layout;

    fn court_chain(court: &str, key: &sig::Keypair, from: u64, until: u64) -> Ledger {
        let mut ledger = super::common::edge_with(court);
        super::common::bind(&mut ledger, court, key, from, until);
        ledger
    }

    fn sample_query() -> Query {
        Query {
            poser: "agent-7".into(),
            shape: vec![6, 6],
            domain_tag: 82,
            guarantee: Guarantee::Rederivation,
            statement: b"close the hexagon".to_vec(),
        }
    }

    // ── X1: the question's name ─────────────────────────────────────────

    #[test]
    fn a_question_keeps_its_name_while_the_pot_fills() {
        let query = sample_query();
        let id = query.query_id();

        // Funding, transport, and time are not in the encoding at all —
        // the id is stable by construction. What DOES change the question
        // changes the name:
        let mut different = sample_query();
        different.statement = b"close the heptagon".to_vec();
        assert_ne!(different.query_id(), id);

        let mut cheaper = sample_query();
        cheaper.guarantee = Guarantee::Convergence;
        assert_ne!(
            cheaper.query_id(),
            id,
            "proof and agreement are different products; different names"
        );

        let back = Query::decode(&query.encode()).expect("its own bytes");
        assert_eq!(back, query);
        assert_eq!(back.query_id(), id);
    }

    #[test]
    fn an_unpriced_guarantee_refuses() {
        let mut bytes = sample_query().encode();
        if let Some(first) = bytes.first_mut() {
            *first = 9;
        }
        assert_eq!(
            Query::decode(&bytes),
            Err(QueryBroken::UnpricedGuarantee(9)),
            "the market does not sell what it has not priced (X5)"
        );
    }

    // ── X2: the receipt ─────────────────────────────────────────────────

    #[test]
    fn a_receipt_verifies_against_chain_state_alone() {
        let key = sig::Keypair::from_seed([4u8; 32]);
        let chain = court_chain("court-a", &key, 0, 100);

        // A real settlement produces the credit…
        let mut book = RewardBook::new();
        let body = datum::domains::demo_hexagon_claim(1).encode();
        let credit = book.credit_claim(&body).expect("settles");

        // …the court signs the statement…
        let signed = receipt::issue("court-a", 5, sample_query().query_id(), &credit, &key);

        // …and a facilitator holding ONLY the receipt and the public
        // chain verifies it. No court. No book. (The book is not even in
        // scope here — it was dropped.)
        drop(book);
        receipt::verify(&signed, &chain).expect("chain + bytes suffice");
        assert_eq!(signed.receipt.axes, vec![6, 6]);
    }

    #[test]
    fn every_forgery_class_refuses_by_name() {
        let key = sig::Keypair::from_seed([4u8; 32]);
        let stranger = sig::Keypair::from_seed([5u8; 32]);
        let chain = court_chain("court-a", &key, 3, 9);
        let mut book = RewardBook::new();
        let credit = book
            .credit_claim(&datum::domains::demo_hexagon_claim(1).encode())
            .expect("settles");
        let qid = sample_query().query_id();

        // Tampered amount: the signature binds exact bytes.
        let mut tampered = receipt::issue("court-a", 5, qid, &credit, &key);
        tampered.receipt.axes = vec![600, 600];
        assert_eq!(
            receipt::verify(&tampered, &chain),
            Err(ReceiptRefused::Forged)
        );

        // A stranger's valid signature: unbound on this chain.
        let unbound = receipt::issue("court-a", 5, qid, &credit, &stranger);
        assert_eq!(
            receipt::verify(&unbound, &chain),
            Err(ReceiptRefused::Unbound)
        );

        // The right key claiming the wrong court's name.
        let misnamed = receipt::issue("court-b", 5, qid, &credit, &key);
        assert_eq!(
            receipt::verify(&misnamed, &chain),
            Err(ReceiptRefused::NotThatCourt)
        );

        // Outside the bind window: stale.
        let stale = receipt::issue("court-a", 10, qid, &credit, &key);
        assert_eq!(receipt::verify(&stale, &chain), Err(ReceiptRefused::Stale));

        // The genuine article still stands among the wreckage.
        let good = receipt::issue("court-a", 5, qid, &credit, &key);
        assert!(receipt::verify(&good, &chain).is_ok());
    }

    // ── X4: the facilitator vector ──────────────────────────────────────

    /// The regeneration path — never hand-typed. Run explicitly:
    /// `cargo test -p plumb-datum --test market -- --ignored`
    #[test]
    #[ignore = "writes the committed vector; run on codec change, then commit"]
    fn regenerate_the_facilitator_vector() {
        let key = sig::Keypair::from_seed([4u8; 32]);
        let mut book = RewardBook::new();
        let credit = book
            .credit_claim(&datum::domains::demo_hexagon_claim(1).encode())
            .expect("settles");
        let signed = receipt::issue("court-a", 5, sample_query().query_id(), &credit, &key);
        let mut vector = signed.receipt.encode();
        vector.extend_from_slice(&signed.attestation.encode());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::write(root.join("../../conformance/20-receipt.bin"), &vector).expect("writes");
        // The chain the receipt verifies against — the check is
        // self-contained: two files, no live system.
        let chain = court_chain("court-a", &key, 0, 100);
        std::fs::write(
            root.join("../../conformance/21-receipt-chain.bin"),
            isthmus::deed::chain::encode(chain.acts()),
        )
        .expect("writes");
    }

    #[test]
    fn the_facilitator_vector_is_pinned() {
        // Deterministic end to end: fixed seed, fixed claim, fixed query.
        // The committed vector is (receipt bytes ‖ attestation bytes), and
        // this test regenerates it from the codec — the file and the code
        // cannot drift without this assertion refusing.
        let key = sig::Keypair::from_seed([4u8; 32]);
        let mut book = RewardBook::new();
        let credit = book
            .credit_claim(&datum::domains::demo_hexagon_claim(1).encode())
            .expect("settles");
        let signed = receipt::issue("court-a", 5, sample_query().query_id(), &credit, &key);

        let mut vector = signed.receipt.encode();
        vector.extend_from_slice(&signed.attestation.encode());

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let committed = std::fs::read(root.join("../../conformance/20-receipt.bin"))
            .expect("vector on disk");
        assert_eq!(committed, vector, "the codec produces these bytes");

        // And the committed pair is a COMPLETE facilitator check: decode
        // the receipt from the vector, replay the committed chain, verify
        // — two files, no live system, expected verdict Ok.
        let chain_bytes = std::fs::read(root.join("../../conformance/21-receipt-chain.bin"))
            .expect("chain on disk");
        let acts = isthmus::deed::chain::decode(&chain_bytes).expect("decodes");
        let chain = Ledger::replay(Layout::founding(), acts);
        let split = committed.len() - sig::ATTESTATION_LEN;
        let parsed = receipt::Receipt::decode(committed.get(..split).expect("receipt part"))
            .expect("receipt decodes");
        let attestation =
            sig::Attestation::decode(committed.get(split..).expect("attestation part"))
                .expect("attestation decodes");
        receipt::verify(
            &receipt::SignedReceipt {
                receipt: parsed,
                attestation,
            },
            &chain,
        )
        .expect("the committed pair verifies: the facilitator's whole check");
    }
}

mod yield_rebate {


    use assay::complex::{ComplexBroken, DeclaredClaim, DeclaredComplex};
    use assay::whole;
    use datum::bounty::{settle_answer, AnswerRefused, Bounty};
    use datum::query::{Guarantee, Query};
    use datum::reward::RewardBook;

    /// The theta universe: two vertices, THREE parallel edges, each with
    /// boundary v1 − v0. Its cycle space is two-dimensional, so it holds
    /// genuinely leaner and fatter closures — which is what a rebate
    /// needs to select between.
    fn theta() -> DeclaredComplex {
        datum::domains::demo_theta_universe()
    }

    fn posted() -> (Query, Bounty) {
        let query = Query {
            poser: "agent-7".into(),
            shape: vec![2, 3],
            domain_tag: 82,
            guarantee: Guarantee::Rederivation,
            statement: theta().encode(), // the POSER fixes the universe
        };
        let bounty = Bounty {
            query_id: query.query_id(),
            max_fuel: 200,
            max_bytes: 400,
            base: 1_000,
            per_saved_fuel: 10,
            per_saved_byte: 3,
        };
        (query, bounty)
    }

    fn answer(witness: Vec<(u32, assay::Exact)>, transport: u64) -> Vec<u8> {
        DeclaredClaim {
            transport,
            complex: theta(),
            dim: 1,
            witness,
        }
        .encode()
    }

    #[test]
    fn the_leaner_witness_captures_more_of_the_same_bounty() {
        let (query, bounty) = posted();
        let mut book = RewardBook::new();

        // Lean: two edges cancel (e0 − e1). Fat: three edges with a
        // doubled coefficient (e0 + e1 − 2·e2) — also a perfect cycle,
        // also credited, measurably more expensive to check.
        let lean = settle_answer(&bounty, &query, &answer(vec![(0, whole(1)), (1, whole(-1))], 1), &mut book)
            .expect("the lean cycle closes");
        let fat = settle_answer(
            &bounty,
            &query,
            &answer(vec![(0, whole(1)), (1, whole(1)), (2, whole(-2))], 1),
            &mut book,
        )
        .expect("the fat cycle closes too");

        assert!(lean.spent_fuel < fat.spent_fuel, "the meter tells them apart");
        assert!(lean.spent_bytes < fat.spent_bytes);
        assert!(
            lean.payout > fat.payout,
            "same universe, same bounty: elegance is the difference — \
             lean {} vs fat {}",
            lean.payout,
            fat.payout
        );

        // Both are bounded by the escrow, which is what makes the market
        // underwritable.
        assert!(lean.payout <= bounty.escrow_bound());
        assert!(fat.payout <= bounty.escrow_bound());
    }

    #[test]
    fn a_self_posed_universe_earns_no_rebate_however_well_it_closes() {
        let (query, bounty) = posted();
        let mut book = RewardBook::new();

        // A beautiful hexagon — in the solver's OWN universe.
        let own = datum::domains::demo_hexagon_claim(1).encode();
        assert_eq!(
            settle_answer(&bounty, &query, &own, &mut book),
            Err(AnswerRefused::NotThePosersUniverse),
            "a node authoring its own task solves it for free — so it is \
             not paid here"
        );
        assert_eq!(book.act_len(), 0, "and nothing touched the book");
    }

    #[test]
    fn over_budget_refuses_with_the_price_named() {
        let (query, mut bounty) = posted();
        let mut book = RewardBook::new();
        let body = answer(vec![(0, whole(1)), (1, whole(-1))], 1);

        // Fuel: the evaluator's own named refusal.
        bounty.max_fuel = 3;
        match settle_answer(&bounty, &query, &body, &mut book) {
            Err(AnswerRefused::Broken(ComplexBroken::FuelExhausted { budget })) => {
                assert_eq!(budget, 3);
            }
            other => panic!("expected a priced fuel refusal, got {other:?}"),
        }

        // Bytes: the byte budget names itself the same way.
        bounty.max_fuel = 200;
        bounty.max_bytes = 10;
        match settle_answer(&bounty, &query, &body, &mut book) {
            Err(AnswerRefused::Oversized { max_bytes, got }) => {
                assert_eq!(max_bytes, 10);
                assert!(got > 10);
            }
            other => panic!("expected Oversized, got {other:?}"),
        }
    }

    #[test]
    fn the_replay_law_reaches_the_bounty_market() {
        let (query, bounty) = posted();
        let mut book = RewardBook::new();
        let body = answer(vec![(0, whole(1)), (1, whole(-1))], 1);
        settle_answer(&bounty, &query, &body, &mut book).expect("first settles");

        // The same structure under a new transport: same work, no pay.
        let copy = answer(vec![(0, whole(1)), (1, whole(-1))], 99);
        assert!(
            matches!(
                settle_answer(&bounty, &query, &copy, &mut book),
                Err(AnswerRefused::Book(datum::reward::RewardRefused::Replay { .. }))
            ),
            "T2 is untouched: the rebate never pays twice for one object"
        );
    }
}

mod refinement {


    use assay::complex::{DeclaredClaim, DeclaredComplex, ProofClaim};
    use assay::whole;
    use datum::bounty::{settle_refinement, RefineRefused, RefinementBounty};
    use datum::reward::RewardBook;

    const CEILING: u64 = 10_000;

    /// Theta with a filling: two vertices, three parallel edges, and ONE
    /// 2-cell f with ∂f = 2·e1 − 2·e2 — so the fat and lean cycles below
    /// are not merely same-boundary but genuinely homologous, and the
    /// certificate has something true to prove.
    fn theta_filled() -> DeclaredComplex {
        datum::domains::demo_theta_filled_universe()
    }

    /// The fat cycle: e0 + e1 − 2·e2.
    fn fat(transport: u64) -> DeclaredClaim {
        DeclaredClaim {
            transport,
            complex: theta_filled(),
            dim: 1,
            witness: vec![(0, whole(1)), (1, whole(1)), (2, whole(-2))],
        }
    }

    /// The lean cycle: e0 − e1. Same boundary (none), fewer everything.
    fn lean(transport: u64) -> DeclaredClaim {
        DeclaredClaim {
            transport,
            complex: theta_filled(),
            dim: 1,
            witness: vec![(0, whole(1)), (1, whole(-1))],
        }
    }

    /// O4's certificate: the 2-cell fills the difference —
    /// fat − lean = 2·e1 − 2·e2 = ∂f, exhibited as a proof claim.
    fn certificate() -> Vec<u8> {
        ProofClaim {
            transport: 0,
            complex: theta_filled(),
            dim: 2,
            target: vec![(1, whole(2)), (2, whole(-2))],
            witness: vec![(0, whole(1))],
            deps: Vec::new(),
        }
        .encode()
    }

    fn settled_original() -> (RewardBook, RefinementBounty) {
        let mut book = RewardBook::new();
        let original = fat(1);
        let target = original.work_id();
        book.credit_claim(&original.encode()).expect("original settles");
        let bounty = RefinementBounty {
            target,
            min_improvement_percent: 10,
            reward: 5_000,
        };
        (book, bounty)
    }

    #[test]
    fn a_strictly_leaner_chain_refines_settled_work() {
        let (mut book, bounty) = settled_original();
        let refined = settle_refinement(&bounty, &lean(1).encode(), None, &mut book, CEILING)
            .expect("the lean cycle refines");

        assert!(refined.saved_fuel > 0, "the meter measured real savings");
        assert!(refined.saved_bytes > 0);
        assert_eq!(refined.payout, 5_000);
        assert!(!refined.homologous, "no certificate was offered");

        // O3 — the record advertises the cheap articulation; nothing was
        // rewritten, and both ids remain settled work.
        let advertised = book.refinements_of(&bounty.target);
        assert_eq!(advertised.len(), 1);
        assert_eq!(advertised.first().map(|(id, _, _)| id.clone()), Some(refined.credit.work_id.clone()));
        assert!(book.seen().contains(&bounty.target), "old citations unbroken");
    }

    #[test]
    fn the_threshold_refuses_with_every_number_named() {
        let (mut book, mut bounty) = settled_original();
        bounty.min_improvement_percent = 20; // the same lean chain falls short here
        match settle_refinement(&bounty, &lean(1).encode(), None, &mut book, CEILING) {
            Err(RefineRefused::NotAnImprovement {
                needed_percent,
                original_fuel,
                refined_fuel,
            }) => {
                assert_eq!(needed_percent, 20);
                assert!(
                    u128::from(refined_fuel) * 100 > u128::from(original_fuel) * 80,
                    "the numbers say exactly how far it fell short: {refined_fuel} vs {original_fuel}"
                );
            }
            other => panic!("expected NotAnImprovement, got {other:?}"),
        }
        assert_eq!(book.act_len(), 1, "an almost-improvement earns nothing");
    }

    #[test]
    fn identical_resubmission_and_unsettled_target_refuse() {
        let (mut book, bounty) = settled_original();

        // The original "refining" itself dies at the THRESHOLD, before
        // the book is even consulted: zero improvement is not an
        // improvement, and the anti-dust gate fires first.
        assert!(matches!(
            settle_refinement(&bounty, &fat(9).encode(), None, &mut book, CEILING),
            Err(RefineRefused::NotAnImprovement { .. })
        ));
        assert_eq!(book.act_len(), 1, "and the book never moved");

        // A bounty on work nobody settled.
        let phantom = RefinementBounty {
            target: lean(0).work_id(),
            min_improvement_percent: 10,
            reward: 1,
        };
        let mut fresh = RewardBook::new();
        assert_eq!(
            settle_refinement(&phantom, &lean(1).encode(), None, &mut fresh, CEILING),
            Err(RefineRefused::UnsettledTarget)
        );
    }

    #[test]
    fn the_homology_certificate_is_verified_not_believed() {
        // With the true certificate: homologous, provably.
        let (mut book, bounty) = settled_original();
        let refined = settle_refinement(
            &bounty,
            &lean(1).encode(),
            Some(&certificate()),
            &mut book,
            CEILING,
        )
        .expect("refines with proof of class");
        assert!(refined.homologous, "∂h = fat − lean, exhibited and checked");

        // A certificate claiming the wrong difference refuses by name.
        let (mut book, bounty) = settled_original();
        let mut wrong = ProofClaim::decode(&certificate()).expect("decodes");
        wrong.target = vec![(1, whole(1)), (2, whole(-1))];
        assert_eq!(
            settle_refinement(&bounty, &lean(1).encode(), Some(&wrong.encode()), &mut book, CEILING),
            Err(RefineRefused::CertificateWrongDifference)
        );

        // A certificate whose filling does not actually fill: the SQ1
        // evaluator refuses it, and the refusal carries through.
        let (mut book, bounty) = settled_original();
        let mut unfilled = ProofClaim::decode(&certificate()).expect("decodes");
        unfilled.witness = vec![(0, whole(2))]; // ∂(2f) = 4e1 − 4e2 ≠ target
        assert!(matches!(
            settle_refinement(&bounty, &lean(1).encode(), Some(&unfilled.encode()), &mut book, CEILING),
            Err(RefineRefused::CertificateBroken(_))
        ));
    }

    #[test]
    fn the_equivalence_federates_once_like_everything_else() {
        let (mut book_a, bounty) = settled_original();
        settle_refinement(&bounty, &lean(1).encode(), None, &mut book_a, CEILING)
            .expect("refines");

        let mut book_b = RewardBook::new();
        let first = book_b.merge_acts_from(&book_a);
        assert!(first >= 3, "two credits and the equivalence crossed");
        assert_eq!(book_b.refinements_of(&bounty.target).len(), 1);

        let again = book_b.merge_acts_from(&book_a);
        assert_eq!(again, 0, "gossip creates no value, equivalences included");
    }
}

mod shaped_closure {


    use assay::complex::{DeclaredComplex, DEFAULT_FUEL};

    fn vector(name: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::read(root.join("../../conformance").join(name)).expect("vector on disk")
    }

    /// Parse one IS-1/5 shaped-closure frame: tag 51, one shaped grain.
    /// Returns (orbs, definition bytes).
    fn parse_shaped(frame: &[u8]) -> (Vec<u32>, Vec<u8>) {
        assert_eq!(frame.first(), Some(&51), "tag 51");
        let len = u32::from_le_bytes(frame[1..5].try_into().expect("len")) as usize;
        let value = &frame[5..5 + len];
        assert_eq!(frame.len(), 5 + len, "no trailing bytes");
        let count = u32::from_le_bytes(value[0..4].try_into().expect("count"));
        assert_eq!(count, 1);
        let mut at = 4usize;
        let arity = u32::from_le_bytes(value[at..at + 4].try_into().expect("arity")) as usize;
        at += 4;
        let mut orbs = Vec::new();
        for _ in 0..arity {
            orbs.push(u32::from_le_bytes(value[at..at + 4].try_into().expect("orb")));
            at += 4;
        }
        let dl = u32::from_le_bytes(value[at..at + 4].try_into().expect("def len")) as usize;
        at += 4;
        let definition = value[at..at + dl].to_vec();
        assert_eq!(at + dl, value.len(), "grain is whole");
        (orbs, definition)
    }

    #[test]
    fn the_hexagon_and_the_five_simplex_are_distinct_on_the_wire() {
        let hexagon = vector("17-shaped-closure-hexagon.bin");
        let simplex = vector("18-shaped-closure-simplex.bin");
        assert_ne!(hexagon, simplex, "the defect class, closed at the byte level");

        let (orbs_h, def_h) = parse_shaped(&hexagon);
        let (orbs_s, def_s) = parse_shaped(&simplex);
        assert_eq!(orbs_h, orbs_s, "SAME six orbs — the old frame collapsed here");

        // The engine's own codec reads the independent implementation's
        // bytes, and the declarations are real complexes.
        let hex = DeclaredComplex::decode(&def_h).expect("python bytes, rust reader");
        let simp = DeclaredComplex::decode(&def_s).expect("python bytes, rust reader");
        hex.admit(DEFAULT_FUEL).expect("the hexagon is a complex");
        simp.admit(DEFAULT_FUEL).expect("the simplex skeleton is a complex");
        assert_ne!(hex, simp);
        assert_eq!(hex.cells, vec![6, 6], "six orbs, six edges: the ring");
        assert_eq!(simp.cells, vec![6, 15], "six orbs, fifteen edges: K6");

        // The vertex map law: dimension-0 cell count equals the arity.
        assert_eq!(hex.cells.first().copied(), Some(orbs_h.len() as u32));
        assert_eq!(simp.cells.first().copied(), Some(orbs_s.len() as u32));
    }

    #[test]
    fn the_legacy_grain_is_shape_unknown_not_a_simplex() {
        let legacy = vector("19-shaped-closure-legacy.bin");
        let (orbs, definition) = parse_shaped(&legacy);
        assert_eq!(orbs, (0..6).collect::<Vec<u32>>());
        assert!(
            definition.is_empty(),
            "IS-1/5's legacy form: the shape is EXPLICITLY unknown — a \
             reader must not infer the simplex, or any shape at all"
        );
    }
}

mod conjecture {
    //! R2/SQ4: a query can pose a THEOREM — the un-flattening. The
    //! statement pins universe AND target; a proof answering a
    //! different theorem refuses by name, and a cycle cannot answer
    //! a conjecture at all.

    use datum::bounty::{settle_answer, AnswerRefused, Bounty};
    use datum::query::{Conjecture, Guarantee, Query};
    use datum::reward::RewardBook;

    fn posed() -> (Query, Bounty, assay::rewrite::Compiled) {
        let compiled = assay::rewrite::Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![(vec![b'b', b'a'], vec![b'a', b'b'])],
        }
        .compile(3)
        .expect("compiles");
        let bba = compiled.word(b"bba").expect("axiom");
        let abb = compiled.word(b"abb").expect("theorem");
        let conjecture = Conjecture {
            universe: compiled.complex.clone(),
            target: compiled.target(bba, abb).expect("target"),
        };
        let query = Query {
            poser: "hilbert".into(),
            shape: vec![15, 5],
            domain_tag: 82,
            guarantee: Guarantee::Rederivation,
            statement: conjecture.encode(),
        };
        let bounty = Bounty {
            query_id: query.query_id(),
            max_fuel: 500,
            max_bytes: 2000,
            base: 10_000,
            per_saved_fuel: 10,
            per_saved_byte: 1,
        };
        (query, bounty, compiled)
    }

    fn derivation(compiled: &assay::rewrite::Compiled, words: &[&[u8]]) -> Vec<u8> {
        let path: Vec<usize> = words
            .iter()
            .map(|w| compiled.word(w).expect("in universe"))
            .collect();
        let first = *path.first().expect("axiom");
        let last = *path.last().expect("theorem");
        assay::complex::ProofClaim {
            transport: 1,
            complex: compiled.complex.clone(),
            dim: 1,
            target: compiled.target(first, last).expect("target"),
            witness: compiled.derive(&path).expect("licensed"),
            deps: Vec::new(),
        }
        .encode()
    }

    #[test]
    fn the_posed_theorem_settles_and_the_wrong_one_refuses_by_name() {
        let (query, bounty, compiled) = posed();
        let mut book = RewardBook::new();

        // A correct derivation of a DIFFERENT theorem: real math,
        // wrong question — the poser paid for bba → abb.
        let other = derivation(&compiled, &[b"ba", b"ab"]);
        assert_eq!(
            settle_answer(&bounty, &query, &other, &mut book),
            Err(AnswerRefused::NotThePosedTheorem)
        );

        // A cycle cannot answer a conjecture at all.
        let cycle = datum::domains::demo_cycle_claim(6, 1).encode();
        assert_eq!(
            settle_answer(&bounty, &query, &cycle, &mut book),
            Err(AnswerRefused::NotAProof)
        );
        assert_eq!(book.act_len(), 0);

        // The posed theorem, derived: settles, with the rebate.
        let answer = derivation(&compiled, &[b"bba", b"bab", b"abb"]);
        let settled = settle_answer(&bounty, &query, &answer, &mut book)
            .expect("the conjecture closes");
        assert!(settled.payout > bounty.base, "yield on the unspent budget");
    }
}
