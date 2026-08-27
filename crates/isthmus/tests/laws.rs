//! LAWS — properties over a generated space, not examples over chosen inputs.
//!
//! Every other test file here is a set test for a set function: one
//! input somebody thought of, one output somebody wrote down. That grows
//! one test per function and witnesses nothing about the class, so a
//! defect nobody thought of is a defect nobody tests.
//!
//! These are laws. The count does not grow with the number of functions,
//! it grows with the number of things that must be **true**, and each is
//! checked over an enumerated space rather than a chosen point.
//!
//! ## Exhaustive, not sampled
//!
//! The generator enumerates a small space **completely**. That is a
//! stronger witness than random sampling: a random run that passes says
//! *these draws passed*, and an exhaustive run over a stated space says
//! *nothing in this space fails*. Each law prints the size of the space
//! it covered, so the claim is bounded by something readable rather than
//! by a seed.
//!
//! ## The law that matters most
//!
//! ```text
//! L3   decode(b) == Ok(v)  =>  encode(v) == b
//! ```
//!
//! **Canonicality.** If two byte strings decode to one value, one of
//! them re-encodes differently and this law names it. Every "two
//! spellings" defect in `IS-1` §4 is an instance — non-reduced, leading
//! zero, `0/n`, and negative zero — and this one statement covers all of
//! them *and* the ones nobody has thought of yet.
//!
//! Negative zero was found by hand, days after two implementations
//! agreed. This law finds it in the space below without being told it
//! exists. `measure/laws.md` in datum carries that run.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use isthmus::frame::{put_frame, Reader};
use isthmus::ratio::{decode, encode, Exact};
use isthmus::layout::Layout;
use isthmus::session::{step, Step};
use num_bigint::BigInt;

// ===================================================================
// The space
// ===================================================================

/// Bytes a magnitude may be built from. Chosen to reach the cases the
/// refusal table names — `0x00` for a leading zero, `0x02`/`0x04` for a
/// shared factor — plus a high byte so sign-extension mistakes show.
const ALPHABET: [u8; 6] = [0x00, 0x01, 0x02, 0x03, 0x04, 0xFF];

/// Every byte string of length 0, 1 and 2 over [`ALPHABET`].
fn magnitudes() -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new()];
    for a in ALPHABET {
        out.push(vec![a]);
    }
    for a in ALPHABET {
        for b in ALPHABET {
            out.push(vec![a, b]);
        }
    }
    out
}

/// Every ratio-shaped byte string over that space, including sign bytes
/// no encoder emits.
fn ratio_strings() -> Vec<Vec<u8>> {
    let mags = magnitudes();
    let mut out = Vec::new();
    for sign in [0u8, 1, 2, 0xFF] {
        for numer in &mags {
            for denom in &mags {
                let mut bytes = vec![sign];
                bytes.extend_from_slice(&(numer.len() as u32).to_le_bytes());
                bytes.extend_from_slice(numer);
                bytes.extend_from_slice(&(denom.len() as u32).to_le_bytes());
                bytes.extend_from_slice(denom);
                out.push(bytes);
            }
        }
    }
    out
}

/// Values to round-trip. Spans sign, zero, unit and multi-byte
/// magnitudes without anyone choosing which ones are interesting.
fn values() -> Vec<Exact> {
    let mut out = Vec::new();
    for numer in -9i64..=9 {
        for denom in 1i64..=9 {
            out.push(Exact::new(BigInt::from(numer), BigInt::from(denom)));
        }
    }
    for k in [255i64, 256, 257, 65535, 65536, 1 << 40] {
        out.push(Exact::new(BigInt::from(k), BigInt::from(255)));
        out.push(Exact::new(BigInt::from(-k), BigInt::from(1)));
    }
    out
}

// ===================================================================
// L1 — totality
// ===================================================================

/// **The decoder returns on every input.**
///
/// A protocol reader that panics on hostile bytes is a protocol reader
/// that can be stopped by hostile bytes. This does not assert *what* it
/// returns; the harness fails the test if any input panics, which is the
/// whole property.
#[test]
fn l1_decode_is_total() {
    let space = ratio_strings();
    for bytes in &space {
        let _ = decode(bytes);
        // And every prefix, because a truncated read is the common case
        // on a stream and it must refuse rather than reach past the end.
        for cut in 0..bytes.len() {
            let _ = decode(&bytes[..cut]);
        }
    }
    println!("L1 covered {} strings and all their prefixes", space.len());
    assert!(!space.is_empty(), "the generator produced nothing");
}

// ===================================================================
// L2 — round trip on values
// ===================================================================

/// **`decode(encode(v)) == v` for every value.**
#[test]
fn l2_every_value_survives_a_round_trip() {
    let space = values();
    for value in &space {
        let bytes = encode(value);
        match decode(&bytes) {
            Ok(back) => assert_eq!(&back, value, "round trip changed {value}"),
            Err(why) => panic!("the encoder emitted bytes the decoder refuses: {value} -> {why}"),
        }
    }
    println!("L2 covered {} values", space.len());
    assert!(space.len() > 100, "the space is too small to mean anything");
}

// ===================================================================
// L3 — canonicality. The one that finds what nobody thought of.
// ===================================================================

/// **`decode(b) == Ok(v)` implies `encode(v) == b`.**
///
/// If two byte strings decode to one value, one of them re-encodes
/// differently, and this law names it without anyone having anticipated
/// the case.
///
/// Every "two spellings" row in `IS-1` §4 is an instance:
///
/// ```text
/// non-reduced     2/4 decodes to 1/2, which encodes to different bytes
/// leading zero    0x0001 decodes to 1, which encodes to one byte
/// 0/n             decodes to 0, which encodes over denominator 1
/// negative zero   decodes to 0, which encodes with sign 0
/// ```
///
/// The last one was found by hand, days after two implementations
/// agreed on the document. It is in the space below and this law does
/// not need to be told it exists.
#[test]
fn l3_an_accepted_string_is_the_only_spelling_of_its_value() {
    let space = ratio_strings();
    let mut accepted = 0usize;
    let mut violations = Vec::new();

    for bytes in &space {
        let Ok(value) = decode(bytes) else { continue };
        accepted += 1;
        let round = encode(&value);
        if &round != bytes {
            violations.push(format!(
                "  {} decoded to {value}, which re-encodes as {}",
                hex(bytes),
                hex(&round)
            ));
        }
    }

    println!("L3 covered {} strings, accepted {accepted}", space.len());
    assert!(
        violations.is_empty(),
        "a value has more than one accepted spelling:\n{}",
        violations.join("\n")
    );

    // The gate must pass as well as fail: if nothing were accepted, the
    // law above would hold vacuously and read as canonicality.
    assert!(
        accepted > 20,
        "only {accepted} strings accepted — the law is close to vacuous"
    );
}

// ===================================================================
// L4 — the session rule, over every cut
// ===================================================================

/// **A record is `Wait` at every proper prefix and `Take` at exactly its
/// own length.**
///
/// This is `IS-2` §7 as a law rather than as four hand-picked buffers.
/// It holds for every record in the space, cut at every byte.
#[test]
fn l4_a_record_waits_until_it_is_whole() {
    let bound = 4096usize;
    let mut records = 0usize;

    // Two layouts, because the law is about records and not about a
    // five-byte header. A one-byte tag field and a four-byte one give
    // different header widths and the same behaviour, which is the whole
    // reason the header stopped being a scalar.
    for layout in [Layout::founding(), Layout::with_tag_width(4)] {
        let header = layout.header();
        for tag in [0u64, 1, 51, 64, 200, 255] {
            for len in [0usize, 1, 2, 5, 17, 64, 255] {
                let mut record = Vec::new();
                put_frame(&layout, tag, &vec![0xAB; len], &mut record).expect("fits");
                records += 1;
                let whole = header + len;
                assert_eq!(record.len(), whole, "the record is header + value");

                for cut in 0..whole {
                    assert_eq!(
                        step(&layout, &record[..cut], bound),
                        Step::Wait,
                        "tag {tag} len {len}: {cut} of {whole} bytes did not Wait"
                    );
                }
                assert_eq!(step(&layout, &record, bound), Step::Take(whole));

                // And trailing bytes do not change the head's verdict — a
                // reader must take one record, not as much as it can.
                let mut over = record.clone();
                over.extend_from_slice(&[0xFFu8; 9]);
                assert_eq!(step(&layout, &over, bound), Step::Take(whole));
            }
        }
    }
    println!("L4 covered {records} records at every cut, over two layouts");
    assert!(records > 40);
}

// ===================================================================
// L5 — the frame round trip
// ===================================================================

/// **`frame(put_frame(t, v)) == (t, v)`, and the reader lands exactly at
/// the end.**
///
/// Landing at the end is the load-bearing half: a reader that consumes
/// the wrong count returns the right tag and then desynchronises the
/// stream, which is the failure §2's skip exists to prevent.
#[test]
fn l5_a_record_reads_back_and_lands_exactly() {
    let mut cases = 0usize;
    // A tag that does NOT fit one byte is included on purpose: under the
    // founding layout it must refuse, and under a four-byte layout the
    // same tag must round-trip. That difference is the thing a `u8` tag
    // could not express.
    for layout in [Layout::founding(), Layout::with_tag_width(4)] {
        for tag in [0u64, 1, 51, 64, 200, 255, 256, 70_000] {
            for len in [0usize, 1, 4, 300] {
                let value = vec![0xCD; len];
                let other = tag ^ 0xFF;

                let mut wire = Vec::new();
                let wrote = put_frame(&layout, tag, &value, &mut wire);
                if !layout.holds(tag) {
                    assert!(
                        wrote.is_err(),
                        "tag {tag} was spelled under a {}-byte tag field",
                        layout.width_of(isthmus::layout::TAG).unwrap_or(0),
                    );
                    assert!(wire.is_empty(), "a refused record left bytes behind");
                    continue;
                }
                wrote.expect("fits");

                // Two records back to back: the second only reads if the
                // first consumed exactly its own length.
                let mut pair = wire.clone();
                if put_frame(&layout, other, &value, &mut pair).is_err() {
                    continue;
                }

                let mut reader = Reader::new(&pair);
                let (t1, v1) = reader.frame(&layout).expect("first record");
                let (t2, v2) = reader.frame(&layout).expect("second record");
                assert_eq!((t1, v1), (tag, &value[..]));
                assert_eq!((t2, v2), (other, &value[..]));
                assert!(reader.is_done(), "reader did not land at the end");
                cases += 1;
            }
        }
    }
    println!("L5 covered {cases} record pairs over two layouts");
    assert!(cases > 10);
}

// ===================================================================
// L6 — refusal is a function, not a mood
// ===================================================================

/// **The same bytes get the same verdict every time.**
///
/// A decoder carrying state across calls would make a conformance
/// corpus meaningless: a verdict would depend on what was read before
/// it.
#[test]
fn l6_a_verdict_does_not_depend_on_history() {
    let space = ratio_strings();
    let first: Vec<_> = space.iter().map(|b| decode(b)).collect();
    // Read the space again in reverse, so anything order-dependent shows.
    let second: Vec<_> = space.iter().rev().map(|b| decode(b)).collect();

    for (at, verdict) in second.into_iter().rev().enumerate() {
        assert_eq!(verdict, first[at], "verdict changed with reading order");
    }
    println!("L6 covered {} strings in both directions", space.len());
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
