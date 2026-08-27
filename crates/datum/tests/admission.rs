//! S4–S7's measurements: the court refuses forged / stale / unbound,
//! the check is payload-blind (a carrier's operation), anchors digest
//! with BLAKE3 at the court edge, and unknown schemes are named.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::admission::{self, AdmissionRefused};
use datum::plumbd;
use datum::reward::RewardBook;
use isthmus::deed::{Act, Ledger};
use isthmus::layout::Layout;

const BOUND: usize = 1 << 16;

fn edge_with(holder: &str) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room on a fresh edge");
    ledger
}

fn bind(ledger: &mut Ledger, holder: &str, key: &sig::Keypair, from: u64, until: u64) {
    ledger.record(Act::Bind {
        holder: holder.into(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public().to_vec(),
        from_epoch: from,
        until_epoch: until,
    });
}

fn triangle_envelope() -> Vec<u8> {
    let shape = datum::onramp::shape_from_edges(
        3,
        [
            (0, 1, assay::whole(1)),
            (1, 2, assay::whole(1)),
            (0, 2, assay::whole(1)),
        ],
    )
    .expect("triangle builds");
    let body = datum::onramp::shape_body(0, shape).expect("body encodes");
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
    wire
}

// ── S4: the three refusals, and the acceptance ──────────────────────

#[test]
fn a_bound_presenter_in_window_admits_and_names_its_holder() {
    let key = sig::Keypair::from_seed([1u8; 32]);
    let mut court = edge_with("court");
    bind(&mut court, "solver-a", &key, 0, 10);
    let envelope = triangle_envelope();
    let att = key.attest(&envelope).encode();
    assert_eq!(
        admission::admit(&court, 5, &envelope, &att),
        Ok("solver-a".to_string())
    );
}

#[test]
fn forged_stale_and_unbound_each_refuse_by_name() {
    let key = sig::Keypair::from_seed([1u8; 32]);
    let other = sig::Keypair::from_seed([2u8; 32]);
    let mut court = edge_with("court");
    bind(&mut court, "solver-a", &key, 3, 9);
    let envelope = triangle_envelope();

    // Forged: right key, wrong bytes.
    let att = key.attest(b"different bytes entirely").encode();
    assert_eq!(
        admission::admit(&court, 5, &envelope, &att),
        Err(AdmissionRefused::Forged)
    );

    // Unbound: a valid signature by a key no holder bound.
    let att = other.attest(&envelope).encode();
    assert_eq!(
        admission::admit(&court, 5, &envelope, &att),
        Err(AdmissionRefused::Unbound)
    );

    // Stale: bound key, epoch outside the window.
    let att = key.attest(&envelope).encode();
    assert_eq!(
        admission::admit(&court, 10, &envelope, &att),
        Err(AdmissionRefused::Stale {
            epoch: 10,
            window: (3, 9)
        })
    );
}

#[test]
fn a_rotated_away_key_no_longer_admits() {
    let old = sig::Keypair::from_seed([1u8; 32]);
    let new = sig::Keypair::from_seed([2u8; 32]);
    let mut court = edge_with("court");
    bind(&mut court, "solver-a", &old, 0, 100);
    bind(&mut court, "solver-a", &new, 0, 100); // rotation appends

    let envelope = triangle_envelope();
    let att = old.attest(&envelope).encode();
    assert_eq!(
        admission::admit(&court, 5, &envelope, &att),
        Err(AdmissionRefused::Unbound),
        "the superseded key is history, not authority"
    );
    let att = new.attest(&envelope).encode();
    assert!(admission::admit(&court, 5, &envelope, &att).is_ok());
}

// ── S5: payload-blind — the carrier's operation ─────────────────────

#[test]
fn admission_never_judges_the_payload() {
    // The envelope's body is garbage no court could credit. Admission
    // still passes: it binds a presenter to bytes, it does not judge
    // claims. That separation is what lets a CARRIER run this check.
    let key = sig::Keypair::from_seed([3u8; 32]);
    let mut court = edge_with("court");
    bind(&mut court, "solver-a", &key, 0, 10);

    let mut garbage = Vec::new();
    isthmus::work::put_claim(b"not a decodable claim body", &mut garbage).expect("frames");
    let att = key.attest(&garbage).encode();
    assert!(
        admission::admit(&court, 1, &garbage, &att).is_ok(),
        "admission is identity, not verification"
    );
}

// ── S6: the anchor digest, chosen at the court edge ─────────────────

#[test]
fn anchors_digest_with_blake3_and_recompute_matches() {
    let ledger = edge_with("north");
    let prefix = isthmus::deed::chain::encode(ledger.acts());
    let digest = admission::anchor_digest(&prefix);
    assert_eq!(digest.len(), 32);
    assert_eq!(
        digest,
        admission::anchor_digest(&prefix),
        "reproducible by any party holding the same prefix"
    );

    // A stranger anchors this chain and a reader re-derives the digest.
    let mut south = Ledger::new(Layout::founding());
    south.record(Act::Anchor {
        chain: "north".into(),
        height: ledger.acts().len() as u64,
        digest: digest.clone(),
        witnessed: "admission tests".into(),
    });
    let recorded = south.acts().iter().find_map(|a| match a {
        Act::Anchor { digest, .. } => Some(digest.clone()),
        _ => None,
    });
    assert_eq!(recorded, Some(digest));
}

// ── S7: unknown schemes are named, end to end ───────────────────────

#[test]
fn an_unknown_scheme_is_a_named_refusal_at_the_court_seam() {
    let key = sig::Keypair::from_seed([4u8; 32]);
    let mut court = edge_with("court");
    bind(&mut court, "solver-a", &key, 0, 10);
    let envelope = triangle_envelope();
    let mut att = key.attest(&envelope);
    att.scheme = 0x02; // the topological seam, not yet spoken
    assert_eq!(
        admission::admit(&court, 5, &envelope, &att.encode()),
        Err(AdmissionRefused::UnknownScheme(0x02))
    );
}

// ── the whole seam over real TCP ────────────────────────────────────

#[test]
fn an_enforcing_court_credits_signed_and_refuses_unsigned() {
    let layout = Layout::founding();
    let solver_key = sig::Keypair::from_seed([7u8; 32]);
    let mut court_ledger = edge_with("test-court");
    bind(&mut court_ledger, "solver-a", &solver_key, 0, u64::MAX);
    let book = Arc::new(Mutex::new(RewardBook::new()));

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    {
        let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
        std::thread::spawn(move || {
            let _ = plumbd::serve(
                &listener, &layout, &ledger, "test-court", &book, BOUND, true, |_| {},
            );
        });
    }

    let producer_ledger = edge_with("solver-a");
    let envelope = triangle_envelope();

    // Unsigned: the enforcing court credits nothing.
    plumbd::produce(
        addr,
        &layout,
        &producer_ledger,
        "solver-a",
        BOUND,
        std::slice::from_ref(&envelope),
    )
    .expect("session runs");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        book.lock().expect("book").act_len(),
        0,
        "an unsigned envelope earns nothing under enforcement"
    );

    // Signed by the bound key: credited.
    plumbd::produce_signed(
        addr,
        &layout,
        &producer_ledger,
        "solver-a",
        BOUND,
        std::slice::from_ref(&envelope),
        &solver_key,
    )
    .expect("session runs");
    let deadline = Instant::now() + Duration::from_secs(5);
    while book.lock().expect("book").act_len() != 1 {
        assert!(Instant::now() < deadline, "signed claim never credited");
        std::thread::sleep(Duration::from_millis(20));
    }

    // Signed by an unbound key: refused, book unchanged.
    let stranger = sig::Keypair::from_seed([9u8; 32]);
    plumbd::produce_signed(
        addr,
        &layout,
        &producer_ledger,
        "solver-a",
        BOUND,
        &[triangle_envelope()],
        &stranger,
    )
    .expect("session runs");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(book.lock().expect("book").act_len(), 1, "stranger earned nothing");
}
