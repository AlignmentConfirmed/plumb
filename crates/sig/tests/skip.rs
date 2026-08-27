//! S2's measurement: an unsigned-era reader forwards attestations whole.
//!
//! The attestation is a tagged record in a band the substrate does not
//! own. Skip-unknown is what makes the signature layer deployable on a
//! live wire: nodes that predate signatures carry signed traffic
//! without a flag day. This test CALLS the substrate rather than
//! asserting the property in prose.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use isthmus::frame::put_frame;
use isthmus::layout::Layout;
use isthmus::{read, Verdict};

/// A bound for this test's deployment — measured here, as the crate
/// insists, not imported as somebody else's constant.
const BOUND: usize = 1 << 16;

#[test]
fn an_unsigned_era_reader_skips_an_attestation_whole() {
    let key = sig::Keypair::from_seed([3u8; 32]);

    // A claim envelope, then its attestation under a foreign tag.
    let layout = Layout::founding();
    let mut envelope = Vec::new();
    isthmus::work::put_claim(b"opaque claim body", &mut envelope).expect("frames");
    let attestation = key.attest(&envelope);

    let mut wire = envelope.clone();
    let att_tag = 200; // a band this reader does not own
    put_frame(&layout, att_tag, &attestation.encode(), &mut wire).expect("frames");

    // An unsigned-era reader owns the work tags and nothing else. It
    // reads its own record…
    let owns = isthmus::work::is_work_tag;
    match read(&layout, &wire, BOUND, owns) {
        Verdict::Accept => {}
        other => panic!("expected Accept for the claim, got {other:?}"),
    }

    // …and SKIPS the attestation whole, bytes intact for forwarding.
    let rest = wire.get(envelope.len()..).expect("rest");
    match read(&layout, rest, BOUND, owns) {
        Verdict::Skip { tag, whole } => {
            assert_eq!(tag, att_tag);
            assert_eq!(whole, rest.len(), "forwarded byte-for-byte");
        }
        other => panic!("expected Skip for the attestation, got {other:?}"),
    }

    // And the forwarded bytes still verify at a node that DOES speak
    // the scheme: transit cost the signature nothing. The value starts
    // past this layout's tag+length header.
    let header = rest.len() - sig::ATTESTATION_LEN;
    let value = rest.get(header..).expect("value");
    let back = sig::Attestation::decode(value).expect("decodes");
    back.verify(&envelope).expect("still binds the envelope");
}
