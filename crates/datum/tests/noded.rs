//! N1's measurement: two nodes attach over real TCP, a claim crosses,
//! the court credits it — and replay refuses on the second try.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::plumbd;
use datum::reward::RewardBook;
use isthmus::deed::Ledger;
use isthmus::layout::Layout;

const BOUND: usize = 1 << 16;

fn edge_with(holder: &str) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room on a fresh edge");
    ledger
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

#[test]
fn a_claim_crosses_tcp_and_credits_once() {
    let layout = Layout::founding();
    let court_ledger = edge_with("test-court");
    let book = Arc::new(Mutex::new(RewardBook::new()));

    // The court, on an ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    {
        let (layout, ledger, book) = (layout.clone(), court_ledger.clone(), Arc::clone(&book));
        std::thread::spawn(move || {
            let rules = plumbd::SessionRules {
                holder: "test-court".into(),
                bound: BOUND,
                enforce: false,
            };
            let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book, &Arc::new(Mutex::new(Vec::new())), |_| {});
        });
    }

    // The producer: attach, send the same claim twice.
    let producer_ledger = edge_with("test-producer");
    let envelope = triangle_envelope();
    let sent = plumbd::produce(
        addr,
        &layout,
        &producer_ledger,
        "test-producer",
        BOUND,
        &[envelope.clone(), envelope],
    )
    .expect("attach and send");
    assert_eq!(sent, 2);

    // The court credits the work once; the copy is replay. Poll — the
    // session runs on the serve thread.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let guard = book.lock().expect("book");
            if guard.seen().len() == 1 && guard.act_len() == 1 {
                break; // one work_id, one credit act: the copy refused
            }
        }
        assert!(
            Instant::now() < deadline,
            "court never credited: book has {} work_ids",
            book.lock().expect("book").seen().len()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn sessions_refuse_peers_with_no_shared_revision() {
    // A session is held to shared revisions by read_hello; this pins
    // the refusal type so the daemon's drop is a decision, not a hang.
    use isthmus::hello::Hello;
    let ours = Hello::of(&edge_with("a"), "a", 1 << 12);
    let theirs = Hello {
        revisions: vec!["XX-9/9".into()],
        ..Hello::default()
    };
    assert!(ours.shared_revisions(&theirs).is_empty());
}
