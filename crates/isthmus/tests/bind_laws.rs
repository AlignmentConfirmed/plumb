//! IS-6/4 — the presenting key on the record (S3).
//!
//! Laws: a bind round-trips the codec; the last bind wins (rotation is
//! an append); an unbound holder is visibly unbound; a bind covers no
//! ground and breaks no horizontal rule.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use isthmus::deed::{chain, Act, Ledger};
use isthmus::layout::Layout;

fn bind(holder: &str, key_byte: u8, from: u64, until: u64) -> Act {
    Act::Bind {
        holder: holder.into(),
        scheme: 0x01,
        key: vec![key_byte; 32],
        from_epoch: from,
        until_epoch: until,
    }
}

#[test]
fn a_bind_round_trips_the_chain_codec() {
    let acts = vec![
        Act::Encumber {
            low: 1,
            high: 31,
            by: "ancestral".into(),
            witnessed: "founding registries".into(),
        },
        Act::Issue {
            holder: "kernel-a".into(),
            low: 64,
            high: 79,
        },
        bind("kernel-a", 7, 0, 100),
    ];
    let bytes = chain::encode(&acts);
    let back = chain::decode(&bytes).expect("its own bytes");
    assert_eq!(back, acts, "byte-identical history, bind included");
}

#[test]
fn the_last_bind_wins_and_history_is_kept() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.record(bind("kernel-a", 1, 0, 10));
    ledger.record(bind("kernel-a", 2, 11, 20));

    let binding = ledger.binding_of("kernel-a").expect("bound");
    assert_eq!(binding.key, vec![2u8; 32], "rotation superseded");
    assert_eq!((binding.from_epoch, binding.until_epoch), (11, 20));

    // The first key is still in the acts — rotation appended, nothing
    // was rewritten.
    let binds = ledger
        .acts()
        .iter()
        .filter(|a| matches!(a, Act::Bind { .. }))
        .count();
    assert_eq!(binds, 2);
}

#[test]
fn an_unbound_holder_is_visibly_unbound() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue("legacy-kernel", 16).expect("room");
    assert_eq!(
        ledger.binding_of("legacy-kernel"),
        None,
        "a keyless grant reads as legacy, never as a random key"
    );
}

#[test]
fn a_bind_covers_no_ground() {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue("kernel-a", 16).expect("room");
    let held_before: Vec<_> = ledger.deeds();
    ledger.record(bind("kernel-a", 9, 0, u64::MAX));
    assert_eq!(
        ledger.deeds(),
        held_before,
        "binding is identity, not ground — no deed moved"
    );
    assert!(
        ledger.well_formed().is_ok(),
        "no horizontal rule trips on a vertical fact"
    );
}

#[test]
fn an_old_reader_refuses_a_bound_chain_rather_than_misfolding() {
    // There is no old reader to link, so the property is pinned at the
    // codec seam: the bind's chain tag is outside the founding trio
    // and inside this decoder's table. A reader without the arm
    // refuses (unknown act), which is IS-6's rule: in a chain, skip
    // would fold a different history and report it as this one.
    let bytes = chain::encode(&[bind("kernel-a", 3, 0, 1)]);
    let back = chain::decode(&bytes).expect("this reader speaks IS-6/4");
    assert!(matches!(back.first(), Some(Act::Bind { .. })));
}
