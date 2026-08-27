//! D-L7 — fund/settlement epoch open/close as ledgered acts.
//!
//! ```bash
//! cargo test --test d7_epoch_acts
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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
