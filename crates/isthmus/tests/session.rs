//! `IS-2` §7 — the four rows, and the stall the rule closes.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod common;
use common::hex;

use isthmus::session::{max_held, step, whole_records, Step, Unsatisfiable};
use isthmus::Verdict;

/// A bound this test supplies, because there is no crate-wide one. The
/// protocol says a bound exists and is declared; the number is the
/// deployment's, and here the deployment is this file.
const BOUND: usize = 1 << 20;

/// The layout this file frames under. A record has no shape without
/// one, so every call names it.
fn lay() -> isthmus::layout::Layout {
    common::founding()
}

/// What this peer owns on this edge. A closure over a deed range, not a
/// global predicate — there is no `isthmus_owns` any more, because which
/// tags this crate reads is a per-edge fact and used to be a const.
fn owns_64_to_79(tag: isthmus::layout::Tag) -> bool {
    (64..=79).contains(&tag)
}

#[test]
fn the_four_rows() {
    let bound = BOUND;

    // len > bound -> REFUSE. No arrival can satisfy this.
    let overlong = hex("01ffffff7f");
    assert!(matches!(
        step(&lay(), &overlong, bound),
        Step::Refuse(Unsatisfiable::Overlong { .. })
    ));

    // buffer < 5 -> WAIT. The header is incomplete.
    assert_eq!(step(&lay(), &hex("0104"), bound), Step::Wait);
    assert_eq!(step(&lay(), &[], bound), Step::Wait);

    // buffer < 5 + len -> WAIT. The value is incomplete.
    assert_eq!(step(&lay(), &hex("0104000000aabb"), bound), Step::Wait);

    // otherwise -> TAKE.
    assert_eq!(step(&lay(), &hex("0104000000deadbeef"), bound), Step::Take(9));
}

/// The defect: a header declaring more than can ever arrive used to sit
/// at the head of the buffer forever, and every later feed re-parsed
/// from the same offset and returned nothing.
///
/// **The session stalled and reported nothing — neither accepting nor
/// refusing.** Both ancestors otherwise hold *refuse, never guess*.
#[test]
fn an_unsatisfiable_header_refuses_rather_than_stalling() {
    let bound = 1024;
    let mut buffer = hex("01ffffff7f");

    let first = step(&lay(), &buffer, bound);
    assert!(matches!(first, Step::Refuse(_)));

    // Feeding more bytes does not change the answer, which is the point:
    // the refusal is decidable from the header alone.
    buffer.extend_from_slice(&[0xAA; 4096]);
    assert_eq!(step(&lay(), &buffer, bound), first);
}

/// The other half of the defect: because an overlong header refuses
/// **at the header**, before any value is held, a session's held bytes
/// never exceed one maximal record.
#[test]
fn the_buffer_is_bounded_by_the_rule_rather_than_by_a_second_rule() {
    let bound = 64;
    assert_eq!(max_held(&lay(), bound), lay().header() + bound);

    // A record exactly at the bound is taken.
    let mut at_bound = vec![7u8];
    at_bound.extend_from_slice(&(bound as u32).to_le_bytes());
    at_bound.extend_from_slice(&vec![0u8; bound]);
    assert_eq!(step(&lay(), &at_bound, bound), Step::Take(lay().header() + bound));

    // One byte past it refuses, and refuses before the value arrives.
    let mut past = vec![7u8];
    past.extend_from_slice(&((bound + 1) as u32).to_le_bytes());
    assert_eq!(past.len(), lay().header(), "no value has been held yet");
    assert!(matches!(step(&lay(), &past, bound), Step::Refuse(_)));
}

#[test]
fn whole_records_reports_how_far_it_got_and_why_it_stopped() {
    let bound = BOUND;

    // Two whole records, then a partial header.
    let mut stream = hex("0100000000");
    stream.extend_from_slice(&hex("c804000000deadbeef"));
    stream.extend_from_slice(&hex("3302"));

    let (consumed, rest) = whole_records(&lay(), &stream, bound);
    assert_eq!(consumed, 5 + 9);
    assert_eq!(rest, Step::Wait);

    // Same stream, but the tail is unsatisfiable rather than incomplete.
    let mut stuck = hex("0100000000");
    stuck.extend_from_slice(&hex("33ffffff7f"));
    let (consumed, rest) = whole_records(&lay(), &stuck, bound);
    assert_eq!(consumed, 5, "the first record is still available");
    assert!(
        matches!(rest, Step::Refuse(_)),
        "and the reader is told the rest will never complete"
    );
}

/// `IS-1` §10 — and the pair most readers conflate.
#[test]
fn skip_and_wait_are_different_answers() {
    let bound = BOUND;

    // Tag 200 is not this crate's. It is a record we will NEVER own.
    let skip = isthmus::read(&lay(), &hex("c804000000deadbeef"), bound, owns_64_to_79);
    assert_eq!(skip, Verdict::Skip { tag: 200, whole: 9 });

    // The same record, one byte short. It has not finished ARRIVING.
    let wait = isthmus::read(&lay(), &hex("c804000000deadbe"), bound, owns_64_to_79);
    assert_eq!(wait, Verdict::Wait);

    assert_ne!(skip, wait, "conflating these drops data or stalls forever");

    // Tag 64 is ours.
    let accept = isthmus::read(&lay(), &hex("4000000000"), bound, owns_64_to_79);
    assert_eq!(accept, Verdict::Accept);

    // And a fourth answer, distinct from all three.
    let refuse = isthmus::read(&lay(), &hex("40ffffff7f"), bound, owns_64_to_79);
    assert!(matches!(refuse, Verdict::Refuse(_)));
}
