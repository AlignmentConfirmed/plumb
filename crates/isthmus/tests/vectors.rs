//! `IS-1` §9 — the published byte strings, asserted against this codec.
//!
//! **Only the vectors this crate owns.** §9.3 (the relation, tag 1),
//! §9.4 (the closures, tag 51) and §9.5 (the manifold, tag 5) name
//! kernel types, and this crate names none. Those five vectors stay in
//! `datum`, which is allowed to know about kernels.
//!
//! What is here is §9.1 the frame, §9.2 the exact rational, and §9.6 an
//! unknown tag — eight of the thirteen, and the eight an integrator
//! needs before anything else will work.
//!
//! Each vector is asserted **in both directions**: the encoder must
//! produce the published bytes, and the decoder must read them back to
//! the value they came from. One direction alone passes for a codec
//! that is consistently wrong.

// A test that cannot reach its subject must fail loudly. A silent skip
// is the failure mode this repository exists to prevent.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod common;
use common::{hex, show};

use isthmus::frame::{put_frame, Reader};
use isthmus::ratio::{decode, encode, Exact};
use num_bigint::BigInt;

fn exact(numer: i64, denom: i64) -> Exact {
    Exact::new(BigInt::from(numer), BigInt::from(denom))
}

fn ratio_vector(name: &str, value: Exact, published: &str) {
    let bytes = encode(&value);
    assert_eq!(
        show(&bytes),
        published,
        "{name}: encoder disagrees with IS-1 §9"
    );
    let back = decode(&hex(published))
        .unwrap_or_else(|why| panic!("{name}: decoder refused its own published bytes: {why}"));
    assert_eq!(back, value, "{name}: round trip lost the value");
}

// ---------------------------------------------------------------- §9.1

#[test]
fn v1_empty_frame() {
    let mut out = Vec::new();
    put_frame(&common::founding(), 1, &[], &mut out).expect("a zero-length value fits");
    assert_eq!(show(&out), "0100000000");
}

#[test]
fn v2_frame_with_a_value() {
    let mut out = Vec::new();
    put_frame(&common::founding(), 51, &[0xAA, 0xBB], &mut out).expect("two bytes fit");
    assert_eq!(show(&out), "3302000000aabb");
}

// ---------------------------------------------------------------- §9.2

/// **The vector to implement against first.** Zero's magnitude is the
/// empty string, not a zero byte, and its denominator is 1.
#[test]
fn v3_zero() {
    ratio_vector("V3", exact(0, 1), "00000000000100000001");
}

#[test]
fn v4_one() {
    ratio_vector("V4", exact(1, 1), "0001000000010100000001");
}

#[test]
fn v5_negative_three() {
    ratio_vector("V5", exact(-3, 1), "0101000000030100000001");
}

#[test]
fn v6_seven_fifths() {
    ratio_vector("V6", exact(7, 5), "0001000000070100000005");
}

/// **The vector that catches a fixed-width assumption.** The numerator
/// needs two bytes and the denominator one.
#[test]
fn v7_two_fifty_six_over_two_fifty_five() {
    ratio_vector("V7", exact(256, 255), "0002000000010001000000ff");
}

// ---------------------------------------------------------------- §9.6

/// The vector that proves §2, and the one a linking mesh depends on.
#[test]
fn v12_an_unknown_tag_is_stepped_over_whole() {
    let published = "c804000000deadbeef";
    let bytes = hex(published);
    assert_eq!(bytes.len(), 9, "1 tag + 4 length + 4 value");

    let mut reader = Reader::new(&bytes);
    let (tag, value) = reader.frame(&common::founding()).expect("a well-formed record");
    assert_eq!(tag, 200);
    assert_eq!(show(value), "deadbeef");
    assert!(
        reader.is_done(),
        "the record was stepped over whole, not partly"
    );
}

/// The property the skip exists for: what *follows* an unknown record
/// still reads. A reader that consumes the wrong number of bytes passes
/// the test above and fails this one.
#[test]
fn what_follows_an_unknown_tag_still_reads() {
    let mut stream = hex("c804000000deadbeef");
    stream.extend_from_slice(&hex("0100000000"));

    let mut reader = Reader::new(&stream);
    let layout = common::founding();
    let (first, _) = reader.frame(&layout).expect("the unknown record");
    let (second, value) = reader.frame(&layout).expect("the record after it");

    assert_eq!(first, 200);
    assert_eq!(second, 1, "V1 follows V12 and reads correctly");
    assert!(value.is_empty());
    assert!(reader.is_done());
}

/// Every published ratio vector, encoded and decoded in one pass, so a
/// change to the codec cannot pass by fixing the vectors one at a time.
#[test]
fn every_ratio_vector_round_trips() {
    let table = [
        ("V3", exact(0, 1), "00000000000100000001"),
        ("V4", exact(1, 1), "0001000000010100000001"),
        ("V5", exact(-3, 1), "0101000000030100000001"),
        ("V6", exact(7, 5), "0001000000070100000005"),
        ("V7", exact(256, 255), "0002000000010001000000ff"),
    ];
    for (name, value, published) in table {
        ratio_vector(name, value, published);
    }
}
