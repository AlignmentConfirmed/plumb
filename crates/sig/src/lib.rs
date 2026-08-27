//! # SIG — the identity physics (scheme `0x01`).
//!
//! Ed25519 over a BLAKE3 **envelope hash**, per `IMPLEMENTATION.md`.
//! The hash covers the whole opaque frame — `tag ‖ LE32(len) ‖ value` —
//! so a checker binds a presenter to bytes it already owns **without
//! reading the payload**. Signature checking is not claim inspection:
//! a carrier may refuse a mis-signed envelope at admission and remain
//! a carrier.
//!
//! The attestation travels as its own tagged record *beside* the
//! envelope, never inside it, so an unsigned-era reader skips it whole
//! (measured in `tests/skip.rs`).
//!
//! ## Scheme agility
//!
//! Every attestation opens with a scheme byte. `0x01` is
//! Ed25519/BLAKE3; an unknown scheme is a named refusal, never a
//! guess — a successor scheme (see `IMPLEMENTATION.md`)
//! enters by chain act, not by wire break.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Scheme byte: Ed25519 signature over a BLAKE3 envelope hash.
pub const SCHEME_ED25519_BLAKE3: u8 = 0x01;

/// Attestation value length: scheme ‖ signer ‖ signature.
pub const ATTESTATION_LEN: usize = 1 + 32 + 64;

/// The BLAKE3 hash of a whole envelope — the bytes a signature binds.
///
/// Callers pass the **frame**, not the value: covering the tag and
/// length means a signature moved onto a different record fails, even
/// when the payloads happen to match.
#[must_use]
pub fn envelope_hash(frame: &[u8]) -> [u8; 32] {
    *blake3::hash(frame).as_bytes()
}

/// Why a signing key could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyBroken {
    /// The operating system's entropy source refused.
    NoEntropy,
}

/// A solver's signing identity. The public half is the identity; the
/// seed never travels.
pub struct Keypair {
    signing: SigningKey,
}

impl Keypair {
    /// Deterministic: the same seed is the same identity. This is the
    /// restore path — a beta tester's identity is a 32-byte seed they
    /// keep, not a file format.
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&seed),
        }
    }

    /// A fresh identity from operating-system entropy.
    pub fn generate() -> Result<Self, KeyBroken> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).map_err(|_| KeyBroken::NoEntropy)?;
        Ok(Self::from_seed(seed))
    }

    /// The public identity: what a grant's `holder_key` records.
    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Sign an envelope: the attestation for one frame.
    #[must_use]
    pub fn attest(&self, frame: &[u8]) -> Attestation {
        let digest = envelope_hash(frame);
        let signature = self.signing.sign(&digest);
        Attestation {
            scheme: SCHEME_ED25519_BLAKE3,
            signer: self.public(),
            signature: signature.to_bytes(),
        }
    }
}

/// A signature beside an envelope: scheme, signer, signature bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// Which scheme signed. `0x01` is Ed25519/BLAKE3.
    pub scheme: u8,
    /// The public identity that claims this envelope.
    pub signer: [u8; 32],
    /// The signature over [`envelope_hash`] of the frame.
    pub signature: [u8; 64],
}

/// Why an attestation did not verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigRefused {
    /// A scheme this build does not speak. Named, never guessed: two
    /// peers on different schemes disagree about what a signature
    /// means, and neither is wrong.
    UnknownScheme(u8),
    /// The signer bytes are not a valid Ed25519 public key.
    BadSigner,
    /// The signature does not bind this signer to this envelope.
    Forged,
    /// The attestation value is not the right shape.
    Truncated,
}

impl Attestation {
    /// The record value: `scheme ‖ signer ‖ signature`, fixed width.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ATTESTATION_LEN);
        out.push(self.scheme);
        out.extend_from_slice(&self.signer);
        out.extend_from_slice(&self.signature);
        out
    }

    /// Read an attestation value. Refuses the wrong width rather than
    /// guessing at trailing bytes — a truncated attestation is not
    /// this attestation.
    pub fn decode(value: &[u8]) -> Result<Self, SigRefused> {
        if value.len() != ATTESTATION_LEN {
            return Err(SigRefused::Truncated);
        }
        let scheme = *value.first().ok_or(SigRefused::Truncated)?;
        let mut signer = [0u8; 32];
        signer.copy_from_slice(value.get(1..33).ok_or(SigRefused::Truncated)?);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(value.get(33..97).ok_or(SigRefused::Truncated)?);
        Ok(Self {
            scheme,
            signer,
            signature,
        })
    }

    /// Verify this attestation against an envelope's bytes.
    ///
    /// The check reads the frame as bytes and never decodes the value:
    /// this is the operation a carrier is allowed.
    pub fn verify(&self, frame: &[u8]) -> Result<(), SigRefused> {
        if self.scheme != SCHEME_ED25519_BLAKE3 {
            return Err(SigRefused::UnknownScheme(self.scheme));
        }
        let key =
            VerifyingKey::from_bytes(&self.signer).map_err(|_| SigRefused::BadSigner)?;
        let digest = envelope_hash(frame);
        let signature = Signature::from_bytes(&self.signature);
        key.verify(&digest, &signature)
            .map_err(|_| SigRefused::Forged)
    }
}

#[cfg(test)]
mod tests {
    // Tests are allowed to panic: a test that cannot reach its subject
    // must say so loudly rather than pass quietly.
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    fn frame(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![tag];
        #[allow(clippy::unwrap_used)]
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn round_trip_signs_and_verifies() {
        let key = Keypair::from_seed([7u8; 32]);
        let envelope = frame(82, b"opaque shape claim bytes");
        let att = key.attest(&envelope);
        att.verify(&envelope).expect("binds");
        let decoded = Attestation::decode(&att.encode()).expect("codec");
        assert_eq!(decoded, att);
        decoded.verify(&envelope).expect("still binds after codec");
    }

    #[test]
    fn moving_a_signature_to_another_record_forges() {
        let key = Keypair::from_seed([7u8; 32]);
        let a = frame(82, b"the work that was signed");
        let b = frame(80, b"the work that was signed");
        let att = key.attest(&a);
        assert_eq!(
            att.verify(&b),
            Err(SigRefused::Forged),
            "same payload, different tag — the envelope hash covers the tag"
        );
    }

    #[test]
    fn another_signer_is_forged_and_unknown_scheme_is_named() {
        let ours = Keypair::from_seed([1u8; 32]);
        let theirs = Keypair::from_seed([2u8; 32]);
        let envelope = frame(80, b"claim");
        let mut att = theirs.attest(&envelope);
        att.signer = ours.public();
        assert_eq!(att.verify(&envelope), Err(SigRefused::Forged));

        let mut wrong_scheme = ours.attest(&envelope);
        wrong_scheme.scheme = 0x02;
        assert_eq!(
            wrong_scheme.verify(&envelope),
            Err(SigRefused::UnknownScheme(0x02))
        );
    }

    #[test]
    fn seed_is_identity_and_truncation_refuses() {
        let a = Keypair::from_seed([9u8; 32]);
        let b = Keypair::from_seed([9u8; 32]);
        assert_eq!(a.public(), b.public(), "same seed, same identity");

        let att = a.attest(&frame(80, b"x")).encode();
        assert_eq!(
            Attestation::decode(att.get(..ATTESTATION_LEN - 1).expect("slice")),
            Err(SigRefused::Truncated)
        );
    }
}
