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

/// The scheme-`0x01` attestation value length: `scheme ‖ signer(32) ‖
/// signature(64)`. Fixed only for `0x01`; a topological successor
/// (scheme ≥ `0x02`) carries hundreds of bytes to kB and is framed
/// length-prefixed instead (see [`Attestation::encode`]). This
/// constant is the `0x01` layout, not a universal wire law.
pub const ATTESTATION_LEN: usize = 1 + 32 + 64;

/// The fixed signer width for scheme `0x01` (an Ed25519 public key).
const ED25519_SIGNER_LEN: usize = 32;
/// The fixed signature width for scheme `0x01`.
const ED25519_SIGNATURE_LEN: usize = 64;

/// The BLAKE3 hash of a whole envelope — the bytes a signature binds.
///
/// Callers pass the **frame**, not the value: covering the tag and
/// length means a signature moved onto a different record fails, even
/// when the payloads happen to match.
#[must_use]
pub fn envelope_hash(frame: &[u8]) -> [u8; 32] {
    *blake3::hash(frame).as_bytes()
}

/// A fresh session token from operating-system entropy (IS-2/2):
/// eight bytes a court challenges with, once per session. What makes
/// a replayed session die is that this value never repeats.
pub fn session_token() -> Result<[u8; 8], KeyBroken> {
    let mut token = [0u8; 8];
    getrandom::getrandom(&mut token).map_err(|_| KeyBroken::NoEntropy)?;
    Ok(token)
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

    /// The seed this identity was made from — the whole of what
    /// [`Keypair::from_seed`] needs to restore it. The only reason to
    /// call this is to persist an identity `generate`d fresh; nothing
    /// else in this crate reads it back out.
    #[must_use]
    pub fn seed(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    /// Sign an envelope: the attestation for one frame.
    #[must_use]
    pub fn attest(&self, frame: &[u8]) -> Attestation {
        let digest = envelope_hash(frame);
        let signature = self.signing.sign(&digest);
        Attestation {
            scheme: SCHEME_ED25519_BLAKE3,
            signer: self.public().to_vec(),
            signature: signature.to_bytes().to_vec(),
        }
    }
}

/// A signature beside an envelope: scheme, signer, signature bytes.
///
/// Signer and signature are variable-length so a topological scheme
/// (≥ `0x02`, hundreds of bytes to kB) travels the same wire as
/// Ed25519's fixed 32/64 — the scheme byte, not a fixed struct width,
/// is what makes the format agile. The on-wire layout is
/// scheme-dispatched (see [`Attestation::encode`]): `0x01` keeps its
/// original fixed layout byte-for-byte; every other scheme is
/// length-prefixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// Which scheme signed. `0x01` is Ed25519/BLAKE3.
    pub scheme: u8,
    /// The public identity that claims this envelope.
    pub signer: Vec<u8>,
    /// The signature over [`envelope_hash`] of the frame.
    pub signature: Vec<u8>,
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
    /// The record value.
    ///
    /// Scheme-dispatched so agility costs the legacy scheme nothing:
    /// - **`0x01`** — the original fixed layout, `scheme ‖ signer(32)
    ///   ‖ signature(64)`, byte-for-byte identical to the pre-agility
    ///   wire (the pinned facilitator conformance vector does not
    ///   move).
    /// - **any other scheme** — self-describing, `scheme ‖
    ///   LE32(signer_len) ‖ signer ‖ LE32(sig_len) ‖ signature`, so a
    ///   variable-length topological signature carries its own shape.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        if self.scheme == SCHEME_ED25519_BLAKE3 {
            let mut out = Vec::with_capacity(ATTESTATION_LEN);
            out.push(self.scheme);
            out.extend_from_slice(&self.signer);
            out.extend_from_slice(&self.signature);
            return out;
        }
        let mut out = Vec::new();
        out.push(self.scheme);
        let signer_len = u32::try_from(self.signer.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&signer_len.to_le_bytes());
        out.extend_from_slice(&self.signer);
        let sig_len = u32::try_from(self.signature.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&sig_len.to_le_bytes());
        out.extend_from_slice(&self.signature);
        out
    }

    /// Read an attestation value, scheme-dispatched to match
    /// [`Attestation::encode`]. Refuses trailing or truncated bytes
    /// rather than guessing — a mis-sized attestation is not this
    /// attestation.
    pub fn decode(value: &[u8]) -> Result<Self, SigRefused> {
        let scheme = *value.first().ok_or(SigRefused::Truncated)?;
        if scheme == SCHEME_ED25519_BLAKE3 {
            if value.len() != ATTESTATION_LEN {
                return Err(SigRefused::Truncated);
            }
            let signer = value
                .get(1..1 + ED25519_SIGNER_LEN)
                .ok_or(SigRefused::Truncated)?
                .to_vec();
            let signature = value
                .get(1 + ED25519_SIGNER_LEN..ATTESTATION_LEN)
                .ok_or(SigRefused::Truncated)?
                .to_vec();
            return Ok(Self {
                scheme,
                signer,
                signature,
            });
        }
        let mut at = 1usize;
        let take_len = |value: &[u8], at: &mut usize| -> Result<usize, SigRefused> {
            let end = at.saturating_add(4);
            let bytes = value.get(*at..end).ok_or(SigRefused::Truncated)?;
            let mut four = [0u8; 4];
            four.copy_from_slice(bytes);
            *at = end;
            Ok(u32::from_le_bytes(four) as usize)
        };
        let take = |value: &[u8], at: &mut usize, n: usize| -> Result<Vec<u8>, SigRefused> {
            let end = at.saturating_add(n);
            let piece = value.get(*at..end).ok_or(SigRefused::Truncated)?;
            *at = end;
            Ok(piece.to_vec())
        };
        let signer_len = take_len(value, &mut at)?;
        let signer = take(value, &mut at, signer_len)?;
        let sig_len = take_len(value, &mut at)?;
        let signature = take(value, &mut at, sig_len)?;
        if at != value.len() {
            return Err(SigRefused::Truncated);
        }
        Ok(Self {
            scheme,
            signer,
            signature,
        })
    }

    /// Verify this attestation against an envelope's bytes.
    ///
    /// The check reads the frame as bytes and never decodes the value:
    /// this is the operation a carrier is allowed. Only `0x01` has a
    /// verifier here; a topological scheme is a named refusal until
    /// its research bar is met (`IMPLEMENTATION.md` §7), never a guess.
    pub fn verify(&self, frame: &[u8]) -> Result<(), SigRefused> {
        if self.scheme != SCHEME_ED25519_BLAKE3 {
            return Err(SigRefused::UnknownScheme(self.scheme));
        }
        let signer: [u8; ED25519_SIGNER_LEN] = self
            .signer
            .as_slice()
            .try_into()
            .map_err(|_| SigRefused::BadSigner)?;
        let key = VerifyingKey::from_bytes(&signer).map_err(|_| SigRefused::BadSigner)?;
        let signature_bytes: [u8; ED25519_SIGNATURE_LEN] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| SigRefused::Forged)?;
        let digest = envelope_hash(frame);
        let signature = Signature::from_bytes(&signature_bytes);
        key.verify(&digest, &signature).map_err(|_| SigRefused::Forged)
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
        att.signer = ours.public().to_vec();
        assert_eq!(att.verify(&envelope), Err(SigRefused::Forged));

        let mut wrong_scheme = ours.attest(&envelope);
        wrong_scheme.scheme = 0x02;
        assert_eq!(
            wrong_scheme.verify(&envelope),
            Err(SigRefused::UnknownScheme(0x02))
        );
    }

    #[test]
    fn generate_draws_fresh_entropy_and_seed_restores_it() {
        let a = Keypair::generate().expect("os entropy");
        let b = Keypair::generate().expect("os entropy");
        assert_ne!(
            a.public(),
            b.public(),
            "two generated identities must not collide — this is not a fixture seed"
        );
        let restored = Keypair::from_seed(a.seed());
        assert_eq!(restored.public(), a.public(), "seed() round-trips through from_seed");
    }

    #[test]
    fn a_variable_length_scheme_round_trips_and_zero_one_stays_byte_identical() {
        // 0x01 encodes to exactly the legacy 97-byte fixed layout.
        let key = Keypair::from_seed([3u8; 32]);
        let legacy = key.attest(&frame(80, b"claim")).encode();
        assert_eq!(legacy.len(), ATTESTATION_LEN, "0x01 keeps its fixed width");
        assert_eq!(legacy.first().copied(), Some(SCHEME_ED25519_BLAKE3));

        // A synthetic topological attestation of ~800 bytes — the size
        // §7 warns about — round-trips through the length-prefixed path.
        let big = Attestation {
            scheme: 0x02,
            signer: vec![0xAB; 140],
            signature: vec![0xCD; 660],
        };
        let wire = big.encode();
        assert!(wire.len() > ATTESTATION_LEN, "a topological witness dwarfs 0x01");
        let back = Attestation::decode(&wire).expect("variable-length round-trips");
        assert_eq!(back, big);
        // ...and trailing bytes on the variable path still refuse.
        let mut trailing = wire.clone();
        trailing.push(0);
        assert_eq!(Attestation::decode(&trailing), Err(SigRefused::Truncated));
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
