//! Wave 4 V16 / D-L3c — court live multi-host federation (TCP XDCT).
//!
//! Beyond d3b file carriers: loopback TCP exchange of durable snapshots.
//! work_id once on both hosts after merge; replay refuses both.
//!
//! ```bash
//! cargo test --test wave4_v16_court_live
//! ```

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

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
