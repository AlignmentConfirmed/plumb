//! LAWS for the session: the records out do not depend on the chunking.
//!
//! `step` was tested on buffers built whole. A stream does not arrive
//! whole — it arrives cut wherever the carrier felt like cutting, and
//! **the cut points are the input space nobody hand-picks**. So the laws
//! quantify over chunkings: every split point, every fixed chunk size,
//! byte-at-a-time. Exhaustive over a stated space, not sampled.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::frame::put_frame;
use isthmus::layout::Layout;
use isthmus::session::{max_held, After, Record, Session};

/// A stream of five records with awkward shapes: empty value, one byte,
/// exactly-header-sized value, a large value, empty again.
fn stream(layout: &Layout) -> (Vec<u8>, Vec<Record>) {
    let cases: Vec<(u64, Vec<u8>)> = vec![
        (1, vec![]),
        (200, vec![0xAA]),
        (51, vec![0xBB; layout.header()]),
        (64, vec![0xCC; 300]),
        (7, vec![]),
    ];
    let mut wire = Vec::new();
    let mut expect = Vec::new();
    for (tag, value) in cases {
        put_frame(layout, tag, &value, &mut wire).expect("fits");
        expect.push(Record { tag, value });
    }
    (wire, expect)
}

/// Feed `wire` in chunks of exactly `size` and collect what comes out.
fn in_chunks(layout: &Layout, bound: usize, wire: &[u8], size: usize) -> Vec<Record> {
    let mut session = Session::new(layout.clone(), bound);
    let mut out = Vec::new();
    for chunk in wire.chunks(size.max(1)) {
        let delivery = session.feed(chunk);
        assert_eq!(delivery.after, After::Waiting, "a clean stream refused");
        out.extend(delivery.records);
        // The bounded-buffer invariant holds after EVERY feed, not at
        // the end.
        assert!(
            session.pending() <= max_held(layout, bound),
            "held {} bytes past the maximum",
            session.pending()
        );
    }
    out
}

// ===================================================================
// S1 — the chunking is invisible
// ===================================================================

/// **Every chunk size yields the same records.** The carrier's accident
/// is removed; the sender's intent survives.
#[test]
fn s1_records_do_not_depend_on_the_chunking() {
    for layout in [Layout::founding(), Layout::with_tag_width(4)] {
        let (wire, expect) = stream(&layout);
        let bound = 4096;

        for size in 1..=wire.len() {
            let got = in_chunks(&layout, bound, &wire, size);
            assert_eq!(
                got, expect,
                "chunk size {size}: different records came out"
            );
        }
        println!(
            "S1: {} chunk sizes over a {}-byte stream, header {}",
            wire.len(),
            wire.len(),
            layout.header()
        );
    }
}

/// **Every two-part split yields the same records** — including splits
/// inside the header, which is where a reader that peeks before it has
/// enough goes wrong.
#[test]
fn s1b_every_split_point_is_survivable() {
    let layout = Layout::founding();
    let (wire, expect) = stream(&layout);
    let bound = 4096;

    for cut in 0..=wire.len() {
        let mut session = Session::new(layout.clone(), bound);
        let mut got = session.feed(&wire[..cut]).records;
        got.extend(session.feed(&wire[cut..]).records);
        assert_eq!(got, expect, "split at {cut}: different records");
    }
}

// ===================================================================
// S2 — never versus not yet, held as state
// ===================================================================

/// A stream that stops mid-record **waits**: everything whole is
/// delivered, the tail is held, and nothing is refused.
#[test]
fn s2_an_incomplete_record_waits_and_loses_nothing() {
    let layout = Layout::founding();
    let (wire, expect) = stream(&layout);
    let bound = 4096;

    // Stop three bytes short of the end.
    let short = &wire[..wire.len() - 3];
    let mut session = Session::new(layout.clone(), bound);
    let delivery = session.feed(short);

    assert_eq!(delivery.after, After::Waiting);
    assert_eq!(delivery.records, expect[..expect.len() - 1]);
    assert!(session.pending() > 0, "the tail is held, not dropped");

    // The rest arrives; the last record completes.
    let rest = session.feed(&wire[wire.len() - 3..]);
    assert_eq!(rest.records, expect[expect.len() - 1..]);
    assert_eq!(session.pending(), 0);
}

/// An unsatisfiable header **refuses**, the refusal is terminal, and
/// the records completed before it are still delivered.
#[test]
fn s2b_a_poisoned_edge_stays_poisoned_and_drops_nothing_it_completed() {
    let layout = Layout::founding();
    let bound = 64;

    let mut wire = Vec::new();
    put_frame(&layout, 1, &[0xAA; 10], &mut wire).expect("fits");
    put_frame(&layout, 2, &[0xBB; 10], &mut wire).expect("fits");
    // A header declaring more than the bound, then bytes that would have
    // been a fine record if anything after a bad header were readable.
    wire.push(3);
    wire.extend_from_slice(&(u32::MAX).to_le_bytes());
    put_frame(&layout, 4, &[0xCC], &mut wire).expect("fits");

    let mut session = Session::new(layout.clone(), bound);
    let delivery = session.feed(&wire);

    // Both whole records arrived. The refusal is about what follows
    // them, not about them.
    assert_eq!(delivery.records.len(), 2);
    assert!(matches!(delivery.after, After::Refused(_)));
    assert!(session.refused().is_some());

    // Terminal: no later feed resurrects the edge, and no later bytes
    // are read. Resynchronising would mean trusting the declared length
    // just refused — there is no frame boundary that does not come from
    // a length.
    for _ in 0..3 {
        let after = session.feed(&[0x00; 32]);
        assert!(after.records.is_empty(), "a dead edge emitted a record");
        assert!(matches!(after.after, After::Refused(_)));
    }
    assert_eq!(session.pending(), 0, "a dead edge holds no garbage");
}

// ===================================================================
// S3 — the bootstrap rule reads the session's position
// ===================================================================

/// The first record on an edge is the declaration — **position, not a
/// reserved number** — and the position comes from the session.
#[test]
fn s3_the_declaration_slot_is_the_sessions_count() {
    let layout = Layout::founding();
    let (wire, _) = stream(&layout);
    let mut session = Session::new(layout.clone(), 4096);

    assert!(isthmus::hello::expects_declaration(
        usize::try_from(session.records_read()).expect("fits")
    ));

    let _ = session.feed(&wire);
    assert_eq!(session.records_read(), 5);
    assert!(!isthmus::hello::expects_declaration(
        usize::try_from(session.records_read()).expect("fits")
    ));
}

// ===================================================================
// S4 — a session per edge, and the edges do not share fate
// ===================================================================

/// Two sessions on one peer: one edge poisons, the other keeps
/// delivering. **This is the runtime half of the per-edge split** — the
/// compile-time half is datum's feature gates, and this is the same
/// property held while running.
#[test]
fn s4_one_edge_dying_does_not_touch_another() {
    let layout = Layout::founding();
    let mut healthy = Session::new(layout.clone(), 64);
    let mut doomed = Session::new(layout.clone(), 64);

    let mut poison = vec![9u8];
    poison.extend_from_slice(&(u32::MAX).to_le_bytes());
    let dead = doomed.feed(&poison);
    assert!(matches!(dead.after, After::Refused(_)));

    // The other edge neither knows nor cares.
    let mut wire = Vec::new();
    put_frame(&layout, 1, &[0xAA], &mut wire).expect("fits");
    let delivery = healthy.feed(&wire);
    assert_eq!(delivery.records.len(), 1);
    assert_eq!(delivery.after, After::Waiting);
}
