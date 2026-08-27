//! D-L3b — multi-host court eventual consistency (file carriers, no sockets).
//!
//! ```text
//! host A credits → write XDCT_A
//! host B loads A, merges, credits distinct work → write XDCT_B
//! host A loads B, merges → both work_ids present; replay both
//! ```
//!
//! Beyond single-process d2 handoff: **three-way** A↔B convergence.
//! Sockets / live multi-machine remain D-S4 / later.
//!
//! ```bash
//! cargo test --test d3b_multi_host_court
//! ```

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

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
