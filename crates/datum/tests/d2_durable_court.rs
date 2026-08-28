//! Diamond ♦2 — durable multi-node POW++ court (D-L2 + D-L3).
//!
//! ```text
//! D-L2  credit → encode/write → load → same work_id Replay
//! D-L3  north + south books; file handoff; merge_acts; anchors independent
//! ```
//!
//! **Not** live quay (edge implementor owns D-E1). Court only.
//!
//! ```bash
//! cargo test --test d2_durable_court
//! ```

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

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
