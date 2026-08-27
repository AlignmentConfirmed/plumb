//! IS-4 at the court: witnesses go on the record over real TCP, and
//! the watcher obeys all four prohibitions of §6.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::plumbd::{self, SessionRules, WitnessLog};
use datum::reward::RewardBook;
use datum::witnessing::{self, WatcherRefused};
use isthmus::deed::Ledger;
use isthmus::layout::Layout;
use isthmus::witness::{Arm, Observer, Witness};

const BOUND: usize = 1 << 16;

fn edge_with(holder: &str) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room");
    ledger
}

fn envelope(n: u32) -> Vec<u8> {
    let body = datum::domains::demo_cycle_claim(n, 0).encode();
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
    wire
}

fn witness_about(envelope: &[u8], arm: Arm) -> Witness {
    Witness {
        arm,
        observer: Observer {
            kind: 1,
            identity: [3u8; 32],
            revision: "IS-6/5".into(),
            depth: 0,
        },
        subject: witnessing::subject_of(envelope),
        derivation: Vec::new(),
    }
}

#[test]
fn a_witness_goes_on_the_record_over_tcp() {
    let ledger = edge_with("court");
    let book = Arc::new(Mutex::new(RewardBook::new()));
    let log: WitnessLog = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    {
        let (layout, ledger, book, log) =
            (Layout::founding(), ledger.clone(), Arc::clone(&book), Arc::clone(&log));
        std::thread::spawn(move || {
            let rules = SessionRules {
                holder: "court".into(),
                bound: BOUND,
                enforce: false,
            };
            let _ = plumbd::serve(&listener, &layout, &ledger, &rules, &book, &log, |_| {});
        });
    }

    let subject = envelope(21);
    let statement = witness_about(&subject, Arm::Succinct);
    let sent = plumbd::witness_to(
        addr,
        &Layout::founding(),
        &edge_with("watcher-1"),
        "watcher-1",
        BOUND,
        std::slice::from_ref(&statement),
        None,
    )
    .expect("witness session");
    assert_eq!(sent, 1);

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let guard = log.lock().expect("log");
            if guard.len() == 1 {
                assert_eq!(guard.first(), Some(&statement), "kept verbatim, unjudged");
                break;
            }
        }
        assert!(Instant::now() < deadline, "witness never reached the record");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_watcher_is_handed_its_subject_or_refuses() {
    let subject = envelope(23);
    let statement = witness_about(&subject, Arm::Replay);

    // §6.1: handed the WRONG subject, it refuses — it may not observe
    // (fetch the right one) and may not repair (pretend this matches).
    let wrong = envelope(25);
    assert_eq!(
        witnessing::watch(&statement, &wrong),
        Err(WatcherRefused::NotTheSubject)
    );

    // Handed the right subject, the replay arm re-derives in full and
    // the report is never bare: it carries the observer (§6.4).
    let report = witnessing::watch(&statement, &subject).expect("answers");
    assert!(report.verified, "the 23-cycle closes on re-derivation");
    assert_eq!(report.observer, statement.observer);
}

#[test]
fn the_replay_arm_actually_re_derives() {
    // A witness about a BROKEN claim: the digest matches (the witness
    // honestly names the broken bytes), but replay re-derivation says
    // the claim does not close. Verdict false, never a refusal — the
    // watcher answered the question it was asked.
    let mut open_claim = datum::domains::demo_cycle_claim(9, 0);
    open_claim.witness.pop();
    let body = open_claim.encode();
    let mut subject = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut subject).expect("frames");

    let statement = witness_about(&subject, Arm::Replay);
    let report = witnessing::watch(&statement, &subject).expect("answers");
    assert!(
        !report.verified,
        "an intact chain over a non-solution is not a solution"
    );
    assert_eq!(report.observer, statement.observer, "still not bare");
}
