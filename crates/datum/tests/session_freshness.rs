//! IS-2/2 (N2): the session challenge. A live key answers this
//! session's token; a replayed session's answer covers a token the
//! court never issued again, and the session never goes live.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::admission;
use datum::plumbd::{self, SessionRules};
use datum::reward::RewardBook;
use isthmus::deed::{Act, Ledger};
use isthmus::hello::Hello;
use isthmus::layout::Layout;

const BOUND: usize = 1 << 16;

fn edge_with(holder: &str) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room");
    ledger
}

fn court(key: &sig::Keypair) -> (std::net::SocketAddr, Arc<Mutex<RewardBook>>) {
    let mut ledger = edge_with("court");
    ledger.record(Act::Bind {
        holder: "solver-a".into(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public().to_vec(),
        from_epoch: 0,
        until_epoch: u64::MAX,
    });
    let book = Arc::new(Mutex::new(RewardBook::new()));
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (layout, book2) = (Layout::founding(), Arc::clone(&book));
    std::thread::spawn(move || {
        let rules = SessionRules {
            holder: "court".into(),
            bound: BOUND,
            enforce: true,
        };
        let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book2, &Arc::new(Mutex::new(Vec::new())), |_| {});
    });
    (addr, book)
}

fn envelope(n: u32) -> Vec<u8> {
    let body = datum::domains::demo_cycle_claim(n, 0).encode();
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
    wire
}

#[test]
fn a_live_key_answers_the_challenge_and_credits() {
    let key = sig::Keypair::from_seed([1u8; 32]);
    let (addr, book) = court(&key);
    plumbd::produce_signed(
        addr,
        &Layout::founding(),
        &edge_with("solver-a"),
        "solver-a",
        BOUND,
        &[envelope(11)],
        &key,
    )
    .expect("fresh session");
    let deadline = Instant::now() + Duration::from_secs(5);
    while book.lock().expect("book").act_len() != 1 {
        assert!(Instant::now() < deadline, "fresh session never credited");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_replayed_session_answer_never_goes_live() {
    let key = sig::Keypair::from_seed([2u8; 32]);
    let (addr, book) = court(&key);
    let layout = Layout::founding();
    let ledger = edge_with("solver-a");
    let ours = Hello::of(&ledger, "solver-a", BOUND as u32);

    // Session 1 — legitimate: capture the challenge and answer it.
    let mut s1 = TcpStream::connect(addr).expect("connect");
    plumbd::send_hello(&mut s1, &layout, plumbd::hello_tag(&ledger, "solver-a"), &ours)
        .expect("hello");
    let mut buf = Vec::new();
    let _court = plumbd::read_hello(&mut s1, &mut buf, &layout, &ours, BOUND).expect("court");
    let (_t, challenge1) = plumbd::read_record(&mut s1, &mut buf, &layout, BOUND)
        .expect("read")
        .expect("challenge");
    // The RECORDED answer an attacker captures off the wire:
    let recorded_answer = key.attest(&challenge1).encode();
    drop(s1); // session 1 abandoned

    // Session 2 — the replay: same recorded answer, NEW challenge.
    let mut s2 = TcpStream::connect(addr).expect("connect");
    plumbd::send_hello(&mut s2, &layout, plumbd::hello_tag(&ledger, "solver-a"), &ours)
        .expect("hello");
    let mut buf = Vec::new();
    let _court = plumbd::read_hello(&mut s2, &mut buf, &layout, &ours, BOUND).expect("court");
    let (_t, challenge2) = plumbd::read_record(&mut s2, &mut buf, &layout, BOUND)
        .expect("read")
        .expect("challenge");
    assert_ne!(challenge1, challenge2, "the token never repeats");

    let mut wire = Vec::new();
    isthmus::frame::put_frame(&layout, admission::ATTESTATION_TAG, &recorded_answer, &mut wire)
        .expect("frame");
    s2.write_all(&wire).expect("replayed answer sent");

    // Signed work follows — but the session never went live, so it
    // refuses before any envelope is even held.
    let env = envelope(13);
    s2.write_all(&env).expect("envelope");
    let att = key.attest(&env).encode();
    let mut wire = Vec::new();
    isthmus::frame::put_frame(&layout, admission::ATTESTATION_TAG, &att, &mut wire)
        .expect("frame");
    s2.write_all(&wire).expect("attestation");
    drop(s2);

    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        book.lock().expect("book").act_len(),
        0,
        "a replayed session earns nothing: its answer covers a dead token"
    );
}

#[test]
fn an_unsigned_producer_still_talks_to_a_lenient_court() {
    // Back-compat: a non-enforcing court sends the challenge too, and
    // an unsigned producer simply reads past it.
    let ledger = edge_with("court");
    let book = Arc::new(Mutex::new(RewardBook::new()));
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    {
        let (layout, ledger, book) = (Layout::founding(), ledger.clone(), Arc::clone(&book));
        std::thread::spawn(move || {
            let rules = SessionRules {
                holder: "court".into(),
                bound: BOUND,
                enforce: false,
            };
            let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
        });
    }
    plumbd::produce(
        addr,
        &Layout::founding(),
        &edge_with("solver-b"),
        "solver-b",
        BOUND,
        &[envelope(17)],
    )
    .expect("unsigned session");
    let deadline = Instant::now() + Duration::from_secs(5);
    while book.lock().expect("book").act_len() != 1 {
        assert!(Instant::now() < deadline, "lenient court never credited");
        std::thread::sleep(Duration::from_millis(20));
    }
}
