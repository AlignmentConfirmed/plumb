//! DURABILITY — the court that survives: snapshots, restarts,
//! multi-host federation, and epoch acts, in one binary.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod court_service {


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
}

mod d2_durable_court {


    use datum::court_store::{self, StoreBroken};
    use datum::reward::{closed_box_claim, triangle_claim, RewardBook, RewardRefused};
    use isthmus::deed::Ledger;
    use isthmus::layout::Layout;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("datum-d2-{name}-{}.xdct", std::process::id()));
        p
    }

    /// **D-L2.** Process restart simulation: credit once, durable load, replay.
    #[test]
    fn restart_preserves_work_id_credit_and_refuses_replay() {
        let path = tmp("restart");
        let body = triangle_claim(7).encode();

        let mut live = RewardBook::new();
        let credit = live.credit_claim(&body).expect("first credit");
        assert_eq!(live.act_len(), 1);
        court_store::write(&path, &live).expect("write");

        // "New process" loads durable court.
        let mut restored = court_store::load(&path).expect("load");
        assert_eq!(restored.act_len(), 1);
        assert_eq!(restored.total().components(), live.total().components());
        assert!(restored.seen().contains(&credit.work_id));

        // Same structure, different transport → still Replay.
        match restored.credit_claim(&triangle_claim(99).encode()) {
            Err(RewardRefused::Replay { work_id }) => {
                assert_eq!(work_id, credit.work_id);
            }
            other => panic!("expected Replay after restart, got {other:?}"),
        }
        assert_eq!(restored.act_len(), 1, "replay must not append");

        // Distinct structure still credits after restore.
        let other = closed_box_claim(1, 3).encode();
        restored.credit_claim(&other).expect("new structure after restore");
        assert_eq!(restored.act_len(), 2);

        let _ = std::fs::remove_file(&path);
    }

    /// Round-trip encode/decode without filesystem.
    #[test]
    fn encode_decode_round_trip_empty_and_stacked() {
        let empty = RewardBook::new();
        let back = court_store::decode(&court_store::encode(&empty)).expect("empty");
        assert_eq!(back.act_len(), 0);

        let mut book = RewardBook::new();
        book.credit_claim(&triangle_claim(1).encode()).expect("t");
        book.credit_claim(&closed_box_claim(2, 1).encode()).expect("b");
        let bytes = court_store::encode(&book);
        let loaded = court_store::decode(&bytes).expect("decode");
        assert_eq!(loaded.act_len(), 2);
        assert_eq!(loaded.total().components(), book.total().components());
    }

    /// Corrupt store refuses.
    #[test]
    fn store_refuses_bad_magic_and_trailing() {
        assert!(matches!(
            court_store::decode(b"NOPE\x01\x00\x00\x00\x00"),
            Err(StoreBroken::Magic)
        ));
        let mut book = RewardBook::new();
        book.credit_claim(&triangle_claim(0).encode()).expect("c");
        let mut bytes = court_store::encode(&book);
        bytes.push(0xFF);
        assert!(matches!(
            court_store::decode(&bytes),
            Err(StoreBroken::Trailing)
        ));
    }

    /// **D-L3.** Two independent court nodes + durable handoff + merge.
    #[test]
    fn multi_node_handoff_merge_and_local_anchor_survive() {
        let path_north = tmp("north");
        let path_south = tmp("south");

        // ── North court credits shape A ──────────────────────────────────
        let mut north = RewardBook::new();
        let body_a = triangle_claim(11).encode();
        let credit_a = north.credit_claim(&body_a).expect("north A");
        court_store::write(&path_north, &north).expect("north write");

        // ── South loads north snapshot (handoff as if from peer process) ─
        let mut south = court_store::load(&path_north).expect("south loads north");
        assert!(south.seen().contains(&credit_a.work_id));
        assert!(matches!(
            south.credit_claim(&triangle_claim(22).encode()),
            Err(RewardRefused::Replay { .. })
        ));

        // South credits distinct structure B and exports.
        let body_b = closed_box_claim(5, 2).encode();
        let credit_b = south.credit_claim(&body_b).expect("south B");
        assert_ne!(credit_a.work_id, credit_b.work_id);
        court_store::write(&path_south, &south).expect("south write");

        // ── North merges south's durable export (gossip) ─────────────────
        let south_snap = court_store::load(&path_south).expect("north reads south");
        let added = north.merge_acts_from(&south_snap);
        assert_eq!(added, 1, "only B is new to north");
        assert!(north.seen().contains(&credit_a.work_id));
        assert!(north.seen().contains(&credit_b.work_id));
        assert_eq!(north.act_len(), 2);

        // Idempotent re-merge.
        assert_eq!(north.merge_acts_from(&south_snap), 0);

        // ── Deed ledgers remain independent (two_chain spirit) ───────────
        // Multi-node court does not collapse estate chains; anchors stay local.
        let mut north_deed = Ledger::new(Layout::founding()).under("north-court");
        let mut south_deed = Ledger::new(Layout::founding()).under("south-court");
        north_deed
            .issue("north-holder", 4)
            .expect("north issue");
        south_deed
            .issue("south-holder", 4)
            .expect("south issue");
        let h_n = north_deed.height();
        let h_s = south_deed.height();
        // Vertical knowledge only — not a capacity mint (POW++ rule).
        north_deed.anchor("south-court", h_s, &[0xab, 0xcd], "d2 multi-node demo");
        assert_eq!(north_deed.height(), h_n + 1);
        assert_eq!(south_deed.height(), h_s, "south deed chain untouched");
        assert!(
            matches!(
                north_deed.acts().last(),
                Some(isthmus::deed::Act::Anchor { chain, .. }) if chain == "south-court"
            ),
            "anchor survived on north deed ledger"
        );

        // Dual-claim events still surface from restored acts for sinks.
        let events: Vec<_> = north
            .acts()
            .iter()
            .filter_map(|a| match a {
                datum::reward::RewardAct::Credited { event, .. } => Some(event.clone()),
                datum::reward::RewardAct::EpochOpened { .. }
                | datum::reward::RewardAct::EpochClosed { .. }
                | datum::reward::RewardAct::Equivalent { .. } => None,
            })
            .collect();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.projects_game() && e.projects_edge()));

        let _ = std::fs::remove_file(&path_north);
        let _ = std::fs::remove_file(&path_south);
    }

    /// admit_event is the multi-node primitive (no body re-parse).
    #[test]
    fn admit_event_refuses_duplicate_without_body() {
        let mut book = RewardBook::new();
        let credit = book
            .credit_claim(&triangle_claim(1).encode())
            .expect("credit");
        let event = credit.to_event();
        assert!(matches!(
            book.admit_event(event),
            Err(RewardRefused::Replay { .. })
        ));
    }
}

mod d3b_multi_host_court {


    use assay::shape::triangle_claim;
    use datum::court_store;
    use datum::reward::{closed_box_claim, RewardBook, RewardRefused};
    use std::path::PathBuf;

    fn path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("datum-d3b-{tag}-{}.xdct", std::process::id()));
        p
    }

    #[test]
    fn three_host_eventual_consistency_on_work_ids() {
        let path_a = path("a");
        let path_b = path("b");
        let path_c = path("c");

        // Host A
        let mut a = RewardBook::new();
        let body_a = triangle_claim(1).encode();
        let credit_a = a.credit_claim(&body_a).expect("A");
        court_store::write(&path_a, &a).expect("write A");

        // Host B starts empty, absorbs A, adds own structure
        let mut b = court_store::load(&path_a).expect("B←A");
        assert!(b.seen().contains(&credit_a.work_id));
        let body_b = closed_box_claim(1, 2).encode();
        let credit_b = b.credit_claim(&body_b).expect("B");
        court_store::write(&path_b, &b).expect("write B");

        // Host C absorbs B (which includes A)
        let mut c = court_store::load(&path_b).expect("C←B");
        assert!(c.seen().contains(&credit_a.work_id));
        assert!(c.seen().contains(&credit_b.work_id));
        assert_eq!(c.act_len(), 2);
        court_store::write(&path_c, &c).expect("write C");

        // Host A merges C → eventual full set
        let snap_c = court_store::load(&path_c).expect("A←C");
        let added = a.merge_acts_from(&snap_c);
        assert_eq!(added, 1, "only B's act is new to A");
        assert!(a.seen().contains(&credit_a.work_id));
        assert!(a.seen().contains(&credit_b.work_id));
        assert_eq!(a.act_len(), 2);

        // All hosts refuse both replays
        for book in [&a, &b, &c] {
            assert!(matches!(
                book.clone().credit_claim(&triangle_claim(9).encode()),
                Err(RewardRefused::Replay { .. })
            ));
            assert!(matches!(
                book.clone().credit_claim(&closed_box_claim(9, 2).encode()),
                Err(RewardRefused::Replay { .. })
            ));
        }

        // Idempotent full mesh gossip
        assert_eq!(a.merge_acts_from(&b), 0);
        assert_eq!(b.merge_acts_from(&a), 0);
        assert_eq!(c.merge_acts_from(&a), 0);

        let _ = std::fs::remove_file(&path_a);
        let _ = std::fs::remove_file(&path_b);
        let _ = std::fs::remove_file(&path_c);
    }

    #[test]
    fn concurrent_hosts_merge_without_double_credit() {
        let mut north = RewardBook::new();
        let mut south = RewardBook::new();
        north
            .credit_claim(&triangle_claim(0).encode())
            .expect("n");
        south
            .credit_claim(&closed_box_claim(0, 1).encode())
            .expect("s");
        // Concurrent divergent histories
        assert_eq!(north.act_len(), 1);
        assert_eq!(south.act_len(), 1);
        let n_add = north.merge_acts_from(&south);
        let s_add = south.merge_acts_from(&north);
        assert_eq!(n_add, 1);
        assert_eq!(s_add, 1);
        assert_eq!(north.act_len(), 2);
        assert_eq!(south.act_len(), 2);
        assert_eq!(north.seen(), south.seen());
    }
}

mod d7_epoch_acts {


    use assay::shape::triangle_claim;
    use datum::court_store;
    use datum::reward::{EpochRefused, RewardAct, RewardBook};

    #[test]
    fn open_two_credits_close_records_count() {
        use datum::reward::closed_box_claim;
        let mut book = RewardBook::new();
        book.open_epoch_named("window-a").unwrap();
        book.credit_claim(&triangle_claim(0).encode()).unwrap();
        book.credit_claim(&closed_box_claim(0, 1).encode()).unwrap();
        let closed = book.close_epoch().unwrap();
        assert_eq!(closed, 0);
        assert_eq!(book.open_epoch(), None);

        match book.acts().last() {
            Some(RewardAct::EpochClosed {
                epoch: 0,
                credits_in_epoch: 2,
            }) => {}
            other => panic!("expected EpochClosed 2 credits, got {other:?}"),
        }

        // Durable round-trip preserves epoch markers.
        let bytes = court_store::encode(&book);
        let loaded = court_store::decode(&bytes).expect("decode");
        assert!(matches!(
            loaded.acts().first(),
            Some(RewardAct::EpochOpened { epoch: 0, .. })
        ));
        assert!(matches!(
            loaded.acts().last(),
            Some(RewardAct::EpochClosed {
                epoch: 0,
                credits_in_epoch: 2
            })
        ));
    }

    #[test]
    fn double_open_and_close_none_refuse() {
        let mut book = RewardBook::new();
        book.open_epoch_named("a").unwrap();
        assert!(matches!(
            book.open_epoch_named("b"),
            Err(EpochRefused::AlreadyOpen { epoch: 0 })
        ));
        book.close_epoch().unwrap();
        assert_eq!(book.close_epoch(), Err(EpochRefused::NoneOpen));
        assert_eq!(
            book.open_epoch_named(""),
            Err(EpochRefused::EmptyLabel)
        );
    }

    #[test]
    fn second_epoch_ids_monotonic() {
        let mut book = RewardBook::new();
        assert_eq!(book.open_epoch_named("e0").unwrap(), 0);
        book.close_epoch().unwrap();
        assert_eq!(book.open_epoch_named("e1").unwrap(), 1);
    }
}

mod wave4_v16_court_live {


    use assay::shape::triangle_claim;
    use datum::court_live::{export_snapshot, federate_loopback_ab, import_merge};
    use datum::reward::{closed_box_claim, RewardBook, RewardRefused};

    #[test]
    fn v16_live_tcp_two_host_merge_work_id_once() {
        let mut a = RewardBook::new();
        let body_a = triangle_claim(1).encode();
        let credit_a = a.credit_claim(&body_a).expect("A credit");

        let mut b = RewardBook::new();
        let body_b = closed_box_claim(1, 2).encode();
        let credit_b = b.credit_claim(&body_b).expect("B credit");

        // Federate over loopback TCP: each absorbs the other's acts.
        let (added_b, added_a) = federate_loopback_ab(&mut a, &mut b).expect("federate");
        assert_eq!(added_b, 1, "B should gain A's act");
        assert_eq!(added_a, 1, "A should gain B's act");

        assert!(a.seen().contains(&credit_a.work_id));
        assert!(a.seen().contains(&credit_b.work_id));
        assert!(b.seen().contains(&credit_a.work_id));
        assert!(b.seen().contains(&credit_b.work_id));
        assert_eq!(a.act_len(), 2);
        assert_eq!(b.act_len(), 2);

        // Replay refuses on both
        assert!(matches!(
            a.clone().credit_claim(&triangle_claim(9).encode()),
            Err(RewardRefused::Replay { .. })
        ));
        assert!(matches!(
            b.clone().credit_claim(&closed_box_claim(9, 2).encode()),
            Err(RewardRefused::Replay { .. })
        ));

        // Second federation adds nothing
        let (z0, z1) = federate_loopback_ab(&mut a, &mut b).expect("again");
        assert_eq!(z0, 0);
        assert_eq!(z1, 0);
    }

    #[test]
    fn v16_import_merge_from_export_bytes() {
        let mut a = RewardBook::new();
        a.credit_claim(&triangle_claim(2).encode()).unwrap();
        let bytes = export_snapshot(&a);
        let mut b = RewardBook::new();
        let n = import_merge(&mut b, &bytes).unwrap();
        assert_eq!(n, 1);
        assert_eq!(b.act_len(), 1);
        assert_eq!(import_merge(&mut b, &bytes).unwrap(), 0);
    }
}
