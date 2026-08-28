//! Wrap portable claims in opaque highway envelopes (tags 80–82).
//!
//! The body bytes are a portable claim in the physics' own encoding
//! (`assay` today; any encoding a court can re-derive tomorrow). This
//! module never reads them — it frames them, which is the whole point:
//! what a carrier cannot read, a carrier cannot front-run.

use isthmus::layout::Tag;
use isthmus::work;
use isthmus::Malformed;

pub use isthmus::work::{classify, Envelope, CLAIM_TAG, RECEIPT_TAG, SHAPE_CLAIM_TAG};

/// The tag a posted query (X1) or conjecture (SQ4) announcement
/// travels under: beside the claim tags (80–82), the attestation tag
/// (83), and witness (84). Canonical here so a court and a kernel
/// that never links the court still agree on the byte.
pub const QUERY_TAG: Tag = 85;

/// The tag an attestation record travels under: beside the claim
/// tags it attests to (80–82), inside the work band (80–127).
/// Canonical here for the same reason as [`QUERY_TAG`] — a kernel
/// checking or producing an attestation frame must agree on this byte
/// without linking `datum`.
pub const ATTESTATION_TAG: Tag = 83;

/// A boundary-domain claim (PoWC), enveloped for the highway.
pub fn claim(body: &[u8]) -> Result<Vec<u8>, Malformed> {
    let mut out = Vec::new();
    work::put_claim(body, &mut out)?;
    Ok(out)
}

/// A shape-domain claim (PoUW), enveloped for the highway.
pub fn shape(body: &[u8]) -> Result<Vec<u8>, Malformed> {
    let mut out = Vec::new();
    work::put_shape_claim(body, &mut out)?;
    Ok(out)
}

/// A receipt, enveloped for the highway.
pub fn receipt(body: &[u8]) -> Result<Vec<u8>, Malformed> {
    let mut out = Vec::new();
    work::put_receipt(body, &mut out)?;
    Ok(out)
}

/// Open one envelope: the tag, and the body it carried, unread.
pub fn open(bytes: &[u8]) -> Result<(Tag, &[u8]), Malformed> {
    work::take_frame(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn an_envelope_round_trips_its_body_unread() {
        let body = b"opaque physics bytes the sdk must not interpret";
        let wire = shape(body).expect("frames");
        let (tag, back) = open(&wire).expect("opens");
        assert_eq!(tag, SHAPE_CLAIM_TAG);
        assert_eq!(back, body);
    }

    #[test]
    fn claim_and_receipt_carry_their_own_tags() {
        let (t1, _) = open(&claim(b"a").expect("frames")).expect("opens");
        let (t2, _) = open(&receipt(b"b").expect("frames")).expect("opens");
        assert_eq!(t1, CLAIM_TAG);
        assert_eq!(t2, RECEIPT_TAG);
    }
}
