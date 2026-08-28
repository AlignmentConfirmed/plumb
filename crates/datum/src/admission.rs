//! S4–S7 — the court and carrier enforcement of signatures.
//!
//! ```text
//! envelope (tags 80/82)  +  attestation (tag 83, beside it)
//!         │                        │
//!         └────────► admit ◄───────┘
//!                      │
//!     1. attestation verifies over the ENVELOPE BYTES   (S4/S5)
//!     2. the signer's key is bound on the chain          (S4)
//!     3. the court's epoch is inside the bind window     (S4)
//!     4. an unknown scheme is a named refusal            (S7)
//! ```
//!
//! The check never decodes the envelope's value — it hashes bytes and
//! reads chain state. That is what makes it a **carrier's** operation
//! too (S5): signature checking is not claim inspection, so a carrier
//! that refuses a mis-signed envelope at admission is still a carrier.

use isthmus::deed::Ledger;
use isthmus::layout::Tag;

/// The tag an attestation record travels under: inside the work band
/// (80–127, assay's grant on the founding edge), beside the claim
/// tags 80–82 it attests to.
pub const ATTESTATION_TAG: Tag = 83;

/// Why admission refused. Every arm is a decision, not a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRefused {
    /// The attestation record's value is not an attestation.
    Malformed,
    /// A scheme this court does not speak — named, never guessed.
    UnknownScheme(u8),
    /// The signature does not bind this signer to this envelope.
    Forged,
    /// No holder on the chain is bound to the signer's key: an
    /// anonymous key is not a party a grant can be held to.
    Unbound,
    /// The signer is bound, but the court's epoch is outside the
    /// binding's window: stale (or premature) presentation.
    Stale {
        /// The court's current epoch.
        epoch: u64,
        /// The binding's window.
        window: (u64, u64),
    },
}

// K1: the ledger-fact lookup underneath this (chain key -> bound
// holder) is leaf-portable — a kernel checking a receipt needs it too
// — so its canonical copy now lives in `sdk::grant`, alongside the
// rest of the ledger-fact authorization checks. Re-exported here so
// every existing `admission::holder_of_key` call site is unchanged.
pub use sdk::grant::holder_of_key;

/// Admit or refuse one signed envelope. The whole of S4, and — because
/// it never decodes the payload — the whole of S5.
///
/// `epoch` is the court's current epoch (the reward book's open
/// epoch, or 0 before any opened): freshness is a chain fact, not a
/// transport secret.
pub fn admit(
    ledger: &Ledger,
    epoch: u64,
    envelope: &[u8],
    attestation_value: &[u8],
) -> Result<String, AdmissionRefused> {
    let attestation = sig::Attestation::decode(attestation_value)
        .map_err(|_| AdmissionRefused::Malformed)?;
    match attestation.verify(envelope) {
        Ok(()) => {}
        Err(sig::SigRefused::UnknownScheme(s)) => {
            return Err(AdmissionRefused::UnknownScheme(s))
        }
        Err(_) => return Err(AdmissionRefused::Forged),
    }
    let holder = holder_of_key(ledger, attestation.scheme, &attestation.signer)
        .ok_or(AdmissionRefused::Unbound)?;
    // The unwrap-free read: binding_of answered once inside
    // holder_of_key; ask again for the window.
    let binding = ledger
        .binding_of(&holder)
        .ok_or(AdmissionRefused::Unbound)?;
    if epoch < binding.from_epoch || epoch > binding.until_epoch {
        return Err(AdmissionRefused::Stale {
            epoch,
            window: (binding.from_epoch, binding.until_epoch),
        });
    }
    Ok(holder)
}

/// S6 — the digest an anchor records, chosen at the court edge.
///
/// The wire stays digest-agnostic (`isthmus` computes no digest and
/// names none); the **court** anchors with BLAKE3. Anchoring a chain
/// means digesting the exact stored bytes of its first `height` acts'
/// encoding — reproducible by any party holding the same prefix.
#[must_use]
pub fn anchor_digest(chain_bytes: &[u8]) -> Vec<u8> {
    sig::envelope_hash(chain_bytes).to_vec()
}
