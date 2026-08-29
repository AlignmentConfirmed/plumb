//! THE WIRE SUITE — records, refusals, sessions, declarations,
//! work envelopes, and the witness frame, in one binary.
//! (no_path_dependencies.rs stays standalone: it is the crate's
//! whole integration claim, referenced by name.)

#![allow(clippy::arithmetic_side_effects, clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]
mod common;

mod vectors {
    use super::common;

    // A test that cannot reach its subject must fail loudly. A silent skip
    // is the failure mode this repository exists to prevent.

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
}

mod refusals {
    use super::common;


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
            Ok(value) => panic!("{name}: accepted {value} — a reader that accepts this has a defect"),
        }
    }

    #[track_caller]
    fn admits(name: &str, bytes: &[u8]) {
        if let Err(why) = decode(bytes) {
            panic!("{name}: the admitted state was refused as {why} — this gate cannot pass");
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
}

mod laws {


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
}

mod session {
    use super::common;


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
}

mod session_laws {


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
}

mod hello {


    use isthmus::deed::Ledger;
    use isthmus::frame::{put_frame, Reader};
    use isthmus::hello::{expects_declaration, Hello};
    use isthmus::layout::Layout;

    /// This test's own bound. There is no crate-wide one — see
    /// `session.rs`'s note on why a measurement of one deployment's corpus
    /// does not belong in a protocol crate.
    const BOUND: u32 = 1 << 20;

    /// An edge with the founding encumbrances and one deed, built at
    /// runtime. Nothing here is a constant the crate carries.
    fn edge_with(holder: &str, width: u128) -> Ledger {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "both registries");
        ledger.issue(holder, width).expect("room on a fresh edge");
        ledger
    }

    #[test]
    fn a_declaration_round_trips_through_a_record() {
        let ledger = edge_with("me", 16);
        let hello = Hello::of(&ledger, "me", BOUND);

        // The declaration is the FIRST record on the edge. Its tag is
        // whatever this edge deeded, not a reserved global number.
        let deed = &ledger.deeds()[0];
        let mut wire = Vec::new();
        put_frame(ledger.layout(), deed.low(), &hello.encode(), &mut wire).expect("fits");

        let mut reader = Reader::new(&wire);
        let (tag, value) = reader.frame(ledger.layout()).expect("a well-formed record");
        assert_eq!(tag, deed.low(), "the edge chose this number, not the crate");

        let back = Hello::decode(value).expect("its own bytes");
        assert_eq!(back, hello);
        assert!(reader.is_done());
    }

    /// The bootstrap rule, whole: **position, not number.**
    ///
    /// An edge that has read nothing expects a declaration. An edge that has
    /// read anything does not. No tag is carved out of any edge to let the
    /// negotiation refer to itself.
    #[test]
    fn the_first_record_on_an_edge_is_the_declaration() {
        assert!(expects_declaration(0));
        for read in 1usize..64 {
            assert!(
                !expects_declaration(read),
                "record {read} was still treated as the opening declaration"
            );
        }
    }

    /// A declaration names the deeds its sender holds **on this edge**, so
    /// the same peer declares different numbers on different edges.
    #[test]
    fn what_a_peer_declares_is_a_property_of_the_edge() {
        let mut quiet = Ledger::new(Layout::founding());
        let mut busy = Ledger::new(Layout::founding());
        busy.encumber(1, 90, "a busier neighbour", "their advert");

        quiet.issue("kernel-a", 16).expect("room");
        busy.issue("kernel-a", 16).expect("room");

        let on_quiet = Hello::of(&quiet, "kernel-a", BOUND);
        let on_busy = Hello::of(&busy, "kernel-a", BOUND);

        assert_ne!(
            on_quiet.ranges, on_busy.ranges,
            "the same holder declared identical ranges on two different edges \
             — then the numbering is global after all"
        );
        assert_eq!(on_quiet.revisions, on_busy.revisions, "revisions are not per-edge");
    }

    /// A truncated declaration is not a partial one. A peer acting on half
    /// a declaration acts on terms the sender did not state.
    #[test]
    fn a_truncated_declaration_refuses_rather_than_reading_what_arrived() {
        let full = Hello::of(&edge_with("me", 16), "me", BOUND).encode();
        for cut in 0..full.len() {
            assert!(
                Hello::decode(&full[..cut]).is_err(),
                "a {cut}-byte prefix decoded — that is half a declaration acted on"
            );
        }
        assert!(Hello::decode(&full).is_ok(), "and the whole thing decodes");
    }

    #[test]
    fn trailing_bytes_are_not_this_declaration() {
        let mut over = Hello::of(&edge_with("me", 16), "me", BOUND).encode();
        over.push(0);
        assert!(Hello::decode(&over).is_err());
    }

    /// Revisions are compared for **equality and never ordered**. Ordering
    /// would let a peer decide it is ahead and act on the difference, which
    /// is authority this substrate does not have.
    #[test]
    fn two_peers_on_different_revisions_are_both_right() {
        let a = Hello {
            revisions: vec!["IS-1/1".into(), "IS-2/1".into()],
            ranges: vec![(64, 79)],
            max_record: 1 << 20,
            ..Default::default()
        };
        let b = Hello {
            revisions: vec!["IS-1/2".into(), "IS-2/1".into()],
            ranges: vec![(192, 199)],
            max_record: 1 << 16,
            ..Default::default()
        };

        assert_eq!(a.shared_revisions(&b), vec!["IS-2/1".to_string()]);
        assert_eq!(b.shared_revisions(&a), a.shared_revisions(&b), "symmetric");

        // Sharing nothing is not an error. The peers still exchange records;
        // each forwards what the other owns.
        let c = Hello {
            revisions: vec!["MESH-9/4".into()],
            ..Default::default()
        };
        assert!(a.shared_revisions(&c).is_empty());
    }

    /// A peer that speaks less is **limited, never refused**.
    #[test]
    fn a_peer_that_declares_nothing_still_connects() {
        let silent = Hello::default();
        for tag in 0u64..=255 {
            assert!(!silent.reads(tag), "it claims nothing");
        }
        // The only thing its empty declaration changes is what gets
        // forwarded rather than read. There is no verdict that rejects it.
        assert_eq!(Hello::bound_for(Some(&silent), 4096), 0);
        assert_eq!(
            Hello::bound_for(None, 4096),
            4096,
            "no declaration heard means the CALLER's fallback, not a crate default"
        );
    }

    #[test]
    fn declared_ranges_decide_what_a_peer_reads() {
        let peer = Hello {
            ranges: vec![(192, 199)],
            ..Default::default()
        };
        assert!(peer.reads(192));
        assert!(peer.reads(199));
        assert!(!peer.reads(191));
        assert!(!peer.reads(200));
    }

    /// A range outside the one-byte tag space is not a range.
    #[test]
    fn an_impossible_range_refuses() {
        let with = |ranges: Vec<(u64, u64)>| Hello {
            ranges,
            ..Default::default()
        };

        // Inverted is not a range.
        assert!(Hello::decode(&with(vec![(80, 64)]).encode()).is_err());

        // And the gate passes. Note `(0, 300)` is now ACCEPTED: a range
        // above 255 is only impossible on a one-byte layout, and refusing it
        // here asserted the tag width in a third place. A peer on a wider
        // layout declaring it is telling the truth.
        assert!(Hello::decode(&with(vec![(0, 300)]).encode()).is_ok());
        assert!(Hello::decode(&with(vec![(64, 79)]).encode()).is_ok());
        assert!(Hello::decode(&with(vec![(64, 64)]).encode()).is_ok());
    }
}

mod work_and_node {

    use isthmus::node::{self, CarrierOut, Role};
    use isthmus::work::{self, CLAIM_TAG, RECEIPT_TAG, SHAPE_CLAIM_TAG};

    #[test]
    fn roles_are_capabilities_not_ranks() {
        assert!(Role::Producer.produces());
        assert!(!Role::Producer.verifies());
        assert!(Role::Verifier.verifies());
        assert!(Role::Carrier.carries());
        assert!(!Role::Carrier.produces());
    }

    #[test]
    fn claim_frame_round_trips_opaque_body() {
        let body = b"not-a-proof-just-bytes";
        let mut wire = Vec::new();
        work::put_claim(body, &mut wire).expect("frame");
        let (tag, value) = work::take_frame(&wire).expect("take");
        assert_eq!(tag, CLAIM_TAG);
        assert_eq!(value, body);
    }

    #[test]
    fn carrier_forwards_foreign_tags_whole() {
        // Tag 1 relation-shaped foreign load.
        let mut wire = Vec::new();
        isthmus::frame::put_frame(
            &isthmus::layout::Layout::founding(),
            1,
            b"foreign-payload",
            &mut wire,
        )
        .expect("put");
        match node::carrier_step(&wire).expect("step") {
            CarrierOut::Forward { whole } => assert_eq!(whole, wire.as_slice()),
            CarrierOut::Deliver { .. } => panic!("foreign tag must forward"),
        }
    }

    #[test]
    fn carrier_delivers_work_tags_without_verifying() {
        let mut wire = Vec::new();
        work::put_receipt(b"opaque-receipt", &mut wire).expect("put");
        match node::carrier_step(&wire).expect("step") {
            CarrierOut::Deliver { tag, body } => {
                assert_eq!(tag, RECEIPT_TAG);
                assert_eq!(body, b"opaque-receipt");
            }
            CarrierOut::Forward { .. } => panic!("work tag should deliver"),
        }
    }

    #[test]
    fn shape_claim_frame_is_work_and_opaque() {
        let body = b"\x02shape-bytes-not-verified-here";
        let mut wire = Vec::new();
        work::put_shape_claim(body, &mut wire).expect("put");
        let (tag, value) = work::take_frame(&wire).expect("take");
        assert_eq!(tag, SHAPE_CLAIM_TAG);
        assert_eq!(value, body);
        match node::carrier_step(&wire).expect("step") {
            CarrierOut::Deliver { tag, body: b } => {
                assert_eq!(tag, SHAPE_CLAIM_TAG);
                assert_eq!(b, body);
            }
            CarrierOut::Forward { .. } => panic!("shape claim is a work tag"),
        }
    }
}

mod witness_frame {


    use isthmus::witness::{Arm, Observer, Witness};

    fn sample() -> Witness {
        Witness {
            arm: Arm::Succinct,
            observer: Observer {
                kind: 2,
                identity: [7u8; 32],
                revision: "corpus/v1".into(),
                depth: 3,
            },
            subject: [9u8; 32],
            derivation: vec![1, 2, 3],
        }
    }

    #[test]
    fn the_frame_round_trips_including_an_empty_derivation() {
        let full = sample();
        assert_eq!(Witness::decode(&full.encode()).expect("its own bytes"), full);

        let mut bare = sample();
        bare.derivation.clear();
        assert_eq!(Witness::decode(&bare.encode()).expect("bare"), bare);
    }

    #[test]
    fn an_unknown_arm_refuses_because_the_budget_is_not_guessable() {
        let mut bytes = sample().encode();
        bytes[0] = 2;
        assert!(
            Witness::decode(&bytes).is_err(),
            "a watcher must know succinct vs replay BEFORE it starts"
        );
    }

    #[test]
    fn the_revision_is_required_never_defaulted() {
        let mut unnamed = sample();
        unnamed.observer.revision = String::new();
        assert!(
            Witness::decode(&unnamed.encode()).is_err(),
            "a corpus without a revision names a moving target"
        );
    }

    #[test]
    fn truncation_refuses_rather_than_repairs() {
        let bytes = sample().encode();
        // Cut inside the observer identity: bytes were promised.
        assert!(Witness::decode(&bytes[..20]).is_err());
    }
}
