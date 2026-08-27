//! `IS-1` §4 — every refusal, **and an admitted state for each**.
//!
//! A refusal table alone is not a gate. A reader that refuses every
//! input passes a table of refusals perfectly, and a reader that accepts
//! every input passes a table of acceptances. Each row below constructs
//! both, so the gate is shown to separate rather than merely to fire.
//!
//! This repository shipped a gate that could not fail five times before
//! adopting the rule, and once shipped one that could not pass.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod common;
use common::hex;

use isthmus::frame::{Malformed, Reader};
use isthmus::ratio::decode;

/// Build a ratio value by hand, so a test can write bytes no encoder
/// would emit — unreduced terms, a leading zero, a sign byte outside
/// `{0,1}`. Both ancestors normalise on construction, so a
/// non-canonical frame has to be written on purpose.
fn raw(sign: u8, numer: &[u8], denom: &[u8]) -> Vec<u8> {
    let mut out = vec![sign];
    for part in [numer, denom] {
        out.extend_from_slice(&(part.len() as u32).to_le_bytes());
        out.extend_from_slice(part);
    }
    out
}

#[track_caller]
fn refuses(name: &str, bytes: &[u8], expected: Malformed) {
    match decode(bytes) {
        Err(got) => assert_eq!(got, expected, "{name}: refused, but named the wrong rule"),
        Ok(value) => panic!("{name}: ACCEPTED {value} — a reader that accepts this has a defect"),
    }
}

#[track_caller]
fn admits(name: &str, bytes: &[u8]) {
    if let Err(why) = decode(bytes) {
        panic!("{name}: the admitted state was REFUSED as {why} — this gate cannot pass");
    }
}

// ================================================================
// The eight rows IS-1/1 §4 publishes
// ================================================================

#[test]
fn row_zero_denominator() {
    refuses(
        "1/0",
        &raw(0, &[0x01], &[]),
        Malformed::ZeroDenominator,
    );
    admits("1/1", &raw(0, &[0x01], &[0x01]));
}

#[test]
fn row_leading_zero_in_a_magnitude() {
    refuses(
        "0x0001 / 2",
        &raw(0, &[0x00, 0x01], &[0x02]),
        Malformed::LeadingZero,
    );
    admits("1/2", &raw(0, &[0x01], &[0x02]));
}

/// §3 says magnitudes *carry* no leading zeros, which states what an
/// encoder does. A second implementation read exactly that and accepted
/// `01/2`. **A rule stated about the writer is not a rule about the
/// reader.**
#[test]
fn row_leading_zero_is_a_rule_about_the_reader_too() {
    let with = raw(0, &[0x00, 0x01], &[0x02]);
    let without = raw(0, &[0x01], &[0x02]);
    assert_ne!(with, without, "two spellings of one value");
    assert!(decode(&with).is_err());
    assert_eq!(decode(&without).expect("canonical"), decode(&without).expect("canonical"));
}

#[test]
fn row_not_reduced() {
    refuses("2/4", &raw(0, &[0x02], &[0x04]), Malformed::NotReduced);
    admits("1/2", &raw(0, &[0x01], &[0x02]));
    // And the refusal is not a silent repair: nothing returns 1/2 here.
    assert!(decode(&raw(0, &[0x02], &[0x04])).is_err());
}

#[test]
fn row_zero_over_anything_but_one() {
    refuses("0/5", &raw(0, &[], &[0x05]), Malformed::NonCanonicalZero);
    admits("0/1", &raw(0, &[], &[0x01]));
}

#[test]
fn row_sign_byte_outside_zero_and_one() {
    refuses("sign 2", &raw(2, &[0x01], &[0x01]), Malformed::SignByte(2));
    refuses(
        "sign 255",
        &raw(255, &[0x01], &[0x01]),
        Malformed::SignByte(255),
    );
    admits("sign 0", &raw(0, &[0x01], &[0x01]));
    admits("sign 1", &raw(1, &[0x03], &[0x01]));
}

#[test]
fn row_declared_length_exceeds_the_record() {
    // Declares a 255-byte numerator and supplies one byte.
    let bytes = hex("00ff00000001");
    match decode(&bytes) {
        Err(Malformed::LengthExceedsRecord {
            declared,
            available,
        }) => {
            assert_eq!(declared, 255);
            assert_eq!(available, 1);
        }
        other => panic!("expected LengthExceedsRecord, got {other:?}"),
    }
    admits("a length that matches", &raw(0, &[0x01], &[0x01]));
}

#[test]
fn row_trailing_bytes() {
    let mut over = raw(0, &[0x01], &[0x01]);
    over.push(0xFF);
    refuses("1/1 with a spare byte", &over, Malformed::TrailingBytes { left: 1 });
    admits("1/1 exactly", &raw(0, &[0x01], &[0x01]));
}

#[test]
fn row_nested_record_with_the_wrong_tag() {
    let mut frame = Vec::new();
    let layout = common::founding();
    isthmus::frame::put_frame(&layout, 51, &[0xAA], &mut frame).expect("fits");

    let mut wrong = Reader::new(&frame);
    assert_eq!(
        wrong.nested(&layout, 1),
        Err(Malformed::UnexpectedTag {
            expected: 1,
            found: 51
        }),
        "inside a known layout an unexpected tag is a disagreement about \
         the layout, not an unknown frame to skip"
    );

    let mut right = Reader::new(&frame);
    assert_eq!(right.nested(&layout, 51), Ok(&[0xAA][..]));
}

// ================================================================
// The ninth row, owed as IS-1/2
// ================================================================

/// Found by writing this crate — a third implementation of a document
/// two implementations had already agreed on.
///
/// `01 ‖ 0 ‖ 1` and `00 ‖ 0 ‖ 1` are two byte strings for one value.
/// Every other such pair in §4 refuses; this one is not in the table.
#[test]
fn row_negative_zero_is_not_in_is_1_slash_1() {
    refuses("-0/1", &raw(1, &[], &[0x01]), Malformed::NegativeZero);
    admits("0/1", &raw(0, &[], &[0x01]));

    // The reason it matters, stated as bytes rather than as an argument:
    // an encoder can never produce this, so accepting it means accepting
    // a spelling only a hostile or broken peer emits.
    let canonical = isthmus::ratio::encode(&isthmus::ratio::Exact::from(num_bigint::BigInt::from(0)));
    assert_eq!(canonical, raw(0, &[], &[0x01]));
    assert_ne!(canonical, raw(1, &[], &[0x01]));
}

// ================================================================
// The gate separates — one pass over the whole table
// ================================================================

/// Every refused input and every admitted input, run together. A codec
/// that drifts toward *refuse everything* or *accept everything* fails
/// here even if the individual rows were adjusted to match it.
#[test]
fn the_table_separates() {
    let refused: Vec<Vec<u8>> = vec![
        raw(0, &[0x01], &[]),             // zero denominator
        raw(0, &[0x00, 0x01], &[0x02]),   // leading zero
        raw(0, &[0x02], &[0x04]),         // not reduced
        raw(0, &[], &[0x05]),             // 0/5
        raw(2, &[0x01], &[0x01]),         // sign byte
        hex("00ff00000001"),              // length past the record
        raw(1, &[], &[0x01]),             // negative zero
    ];
    let admitted: Vec<Vec<u8>> = vec![
        raw(0, &[], &[0x01]),             // 0
        raw(0, &[0x01], &[0x01]),         // 1
        raw(1, &[0x03], &[0x01]),         // -3
        raw(0, &[0x07], &[0x05]),         // 7/5
        raw(0, &[0x01, 0x00], &[0xFF]),   // 256/255
    ];

    let refused_count = refused.iter().filter(|b| decode(b).is_err()).count();
    let admitted_count = admitted.iter().filter(|b| decode(b).is_ok()).count();

    assert_eq!(refused_count, refused.len(), "an input got through");
    assert_eq!(admitted_count, admitted.len(), "a valid input was refused");
    assert!(
        refused_count > 0 && admitted_count > 0,
        "a gate that only ever fires one way is not a gate"
    );
}
