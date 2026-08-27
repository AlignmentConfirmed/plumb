//! N3's measurements: a killed court resumes from its snapshot, and
//! federation refuses replay across real sockets, continuously.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use datum::court_service::{self, ServiceConfig};
use datum::reward::RewardBook;

fn scratch(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("plumb-court-service-{name}-{}", std::process::id()));
    path
}

fn credit_triangle(book: &mut RewardBook, seed: i64) {
    let shape = datum::onramp::shape_from_edges(
        3,
        [
            (0, 1, assay::whole(seed)),
            (1, 2, assay::whole(seed)),
            (0, 2, assay::whole(seed)),
        ],
    )
    .expect("triangle builds");
    let body = datum::onramp::shape_body(0, shape).expect("body encodes");
    book.credit_claim(&body).expect("credits");
}

#[test]
fn a_killed_court_resumes_and_still_refuses_replay() {
    let path = scratch("resume");
    let _ = std::fs::remove_file(&path);

    // A court credits work and snapshots.
    let mut book = RewardBook::new();
    credit_triangle(&mut book, 1);
    court_service::snapshot_atomic(&path, &book).expect("snapshot writes");

    // The process "dies" — the book drops. The service restarts from
    // the snapshot: the acts are back, and the same work is replay.
    drop(book);
    let restored = Arc::new(Mutex::new(RewardBook::new()));
    let config = ServiceConfig {
        snapshot: Some(path.clone()),
        snapshot_secs: 3600, // this test exercises restore, not the loop
        ..ServiceConfig::default()
    };
    let (handle, restored_acts) =
        court_service::start(&config, &restored).expect("service starts");
    assert_eq!(restored_acts, 1, "the snapshot's act came back");
    {
        let mut guard = restored.lock().expect("book");
        assert_eq!(guard.act_len(), 1);
        let shape = datum::onramp::shape_from_edges(
            3,
            [
                (0, 1, assay::whole(1)),
                (1, 2, assay::whole(1)),
                (0, 2, assay::whole(1)),
            ],
        )
        .expect("triangle");
        let body = datum::onramp::shape_body(0, shape).expect("body");
        assert!(
            guard.credit_claim(&body).is_err(),
            "the restored court remembers: same structure is replay"
        );
    }
    handle.stop();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_corrupt_snapshot_refuses_rather_than_forgetting() {
    let path = scratch("corrupt");
    std::fs::write(&path, b"XDCT?not really").expect("writes");
    let book = Arc::new(Mutex::new(RewardBook::new()));
    let config = ServiceConfig {
        snapshot: Some(path.clone()),
        ..ServiceConfig::default()
    };
    assert!(
        court_service::start(&config, &book).is_err(),
        "an unreadable record is no authority at all — never an empty one"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn federation_carries_work_once_across_real_sockets() {
    // Court B accepts federation on an ephemeral port.
    let probe = TcpListener::bind("127.0.0.1:0").expect("probe");
    let fed_addr = probe.local_addr().expect("addr").to_string();
    drop(probe); // the service rebinds it

    let book_b = Arc::new(Mutex::new(RewardBook::new()));
    let config_b = ServiceConfig {
        fed_listen: Some(fed_addr.clone()),
        ..ServiceConfig::default()
    };
    let (handle_b, _) = court_service::start(&config_b, &book_b).expect("B starts");

    // Court A credits work and pushes to B on a fast loop.
    let book_a = Arc::new(Mutex::new(RewardBook::new()));
    credit_triangle(&mut book_a.lock().expect("book"), 2);
    let config_a = ServiceConfig {
        fed_peers: vec![fed_addr],
        fed_secs: 1,
        ..ServiceConfig::default()
    };
    let (handle_a, _) = court_service::start(&config_a, &book_a).expect("A starts");

    // B learns A's act…
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if book_b.lock().expect("book").act_len() == 1 {
            break;
        }
        assert!(Instant::now() < deadline, "B never merged A's act");
        std::thread::sleep(Duration::from_millis(50));
    }

    // …and repeated pushes add nothing: replay refuses by work_id,
    // continuously, across the wire.
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(
        book_b.lock().expect("book").act_len(),
        1,
        "the same snapshot pushed again and again merges zero"
    );

    handle_a.stop();
    handle_b.stop();
}
