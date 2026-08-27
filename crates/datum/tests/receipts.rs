//! X1 + X2 measurements: a question keeps its name while the pot
//! fills; a receipt verifies against chain state alone; and every
//! forgery class refuses by name.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use datum::query::{Guarantee, Query, QueryBroken};
use datum::receipt::{self, ReceiptRefused};
use datum::reward::RewardBook;
use isthmus::deed::{Act, Ledger};
use isthmus::layout::Layout;

fn court_chain(court: &str, key: &sig::Keypair, from: u64, until: u64) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(court, 16).expect("room");
    ledger.record(Act::Bind {
        holder: court.into(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public().to_vec(),
        from_epoch: from,
        until_epoch: until,
    });
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
/// `cargo test -p plumb-datum --test receipts -- --ignored`
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
