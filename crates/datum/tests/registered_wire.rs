//! The audit's fix, pinned: UC4 on the LIVE wire. A claim arriving
//! under a chain-registered tag is judged against the registered
//! definition by the court SESSION — not just resolvable in a
//! library, but consumed on the socket.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::plumbd::{self, SessionRules};
use datum::reward::RewardBook;
use isthmus::deed::{Act, Ledger};
use isthmus::hello::Hello;
use isthmus::layout::Layout;

const BOUND: usize = 1 << 16;

#[test]
fn a_registered_tag_is_judged_on_the_wire_by_the_chain_taught_definition() {
    // The chain: "geometer" holds a range and registered the hexagon
    // universe on its low tag.
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue("court", 16).expect("room");
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
        definition: datum::domains::demo_hexagon_universe().encode(),
    });

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
            let _ = plumbd::serve(
                &listener,
                &layout,
                &ledger,
                &rules,
                &book,
                &Arc::new(Mutex::new(Vec::new())),
                |_| {},
            );
        });
    }

    // A raw session: hello, read hello + challenge, then two records
    // under the REGISTERED tag — the right universe, and a wrong one.
    let layout = Layout::founding();
    let ours = Hello::of(&ledger, "geometer", BOUND as u32);
    let mut stream = TcpStream::connect(addr).expect("connect");
    plumbd::send_hello(&mut stream, &layout, tag, &ours).expect("hello");
    let mut buf = Vec::new();
    let _court = plumbd::read_hello(&mut stream, &mut buf, &layout, &ours, BOUND).expect("court");
    let _challenge = plumbd::read_record(&mut stream, &mut buf, &layout, BOUND)
        .expect("read")
        .expect("challenge");

    // The right universe: the hexagon claim, framed under the tag.
    let good = datum::domains::demo_hexagon_claim(1).encode();
    let mut wire = Vec::new();
    isthmus::frame::put_frame(&layout, tag, &good, &mut wire).expect("frames");
    stream.write_all(&wire).expect("sends");

    // The wrong universe: a triangle — closes beautifully, but it is
    // not what the chain registered for this tag.
    let bad = datum::domains::demo_cycle_claim(3, 1).encode();
    let mut wire = Vec::new();
    isthmus::frame::put_frame(&layout, tag, &bad, &mut wire).expect("frames");
    stream.write_all(&wire).expect("sends");
    drop(stream);

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let guard = book.lock().expect("book");
            if guard.act_len() == 1 {
                break; // the hexagon credited; the triangle refused
            }
        }
        assert!(
            Instant::now() < deadline,
            "the registered-tag path never judged the claim"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        book.lock().expect("book").act_len(),
        1,
        "the wrong universe earned nothing on the registered tag"
    );
}
