//! The carrier's measurement: a signed claim credits THROUGH a relay
//! that forwards frames unread — and the signature survives carriage,
//! because the attestation binds envelope bytes, not a route.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::plumbd;
use datum::reward::RewardBook;
use isthmus::deed::{Act, Ledger};
use isthmus::layout::Layout;

const BOUND: usize = 1 << 16;

fn edge_with(holder: &str) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room");
    ledger
}

#[test]
fn a_signed_claim_credits_through_an_unreading_carrier() {
    let layout = Layout::founding();
    let key = sig::Keypair::from_seed([5u8; 32]);

    // The enforcing court, with the solver's key bound.
    let mut court_ledger = edge_with("court");
    court_ledger.record(Act::Bind {
        holder: "solver-a".into(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public().to_vec(),
        from_epoch: 0,
        until_epoch: u64::MAX,
    });
    let book = Arc::new(Mutex::new(RewardBook::new()));
    let court_listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let court_addr = court_listener.local_addr().expect("addr");
    {
        let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
        std::thread::spawn(move || {
            let rules = plumbd::SessionRules {
                holder: "court".into(),
                bound: BOUND,
                enforce: true,
            };
            let _ = plumbd::serve(&court_listener, &layout, &ledger, &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
        });
    }

    // The carrier between them.
    let carrier_listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let carrier_addr = carrier_listener.local_addr().expect("addr");
    {
        let (layout, ledger) = (layout.clone(), edge_with("carrier"));
        std::thread::spawn(move || {
            let _ = plumbd::carry(
                &carrier_listener,
                &layout,
                &ledger,
                "carrier",
                BOUND,
                court_addr.to_string(),
                |_| {},
            );
        });
    }

    // The client sends THROUGH the carrier.
    let body = datum::domains::demo_cycle_claim(9, 0).encode();
    let mut envelope = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut envelope).expect("frames");
    plumbd::produce_signed(
        carrier_addr,
        &layout,
        &edge_with("solver-a"),
        "solver-a",
        BOUND,
        std::slice::from_ref(&envelope),
        &key,
    )
    .expect("client attaches to the carrier");

    let deadline = Instant::now() + Duration::from_secs(5);
    while book.lock().expect("book").act_len() != 1 {
        assert!(
            Instant::now() < deadline,
            "the claim never credited through the carrier"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
