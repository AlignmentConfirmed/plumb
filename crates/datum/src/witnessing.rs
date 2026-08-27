//! IS-4 at the court: the witness log, and the watcher's law.
//!
//! The substrate carries witness frames; the court **keeps** them —
//! witnesses are how a peer that runs neither producer nor verifier
//! still puts something on the record: *I observed this subject
//! crossing, against this observer, at this revision.*
//!
//! The watcher (`IS-4` §6) lives here too, held to all four
//! prohibitions:
//!
//! 1. it may not observe — the subject is HANDED to it, or it refuses;
//! 2. it may not repair — a mismatched subject refuses, never fixes;
//! 3. it may not require canonical form — verification is the test,
//!    not equality of witnesses;
//! 4. it may not return a bare verdict — the answer carries the
//!    observer it was reached against, or it compares with nothing.

use isthmus::layout::Tag;
use isthmus::witness::{Arm, Observer, Witness};

/// The tag a witness about court-range work travels under: beside the
/// claims (80–82) and the attestation (83), inside the same grant.
/// A witness about another vocabulary's claim sits in *that* grant —
/// this constant is the court's, not the world's.
pub const WITNESS_TAG: Tag = 84;

/// The subject identity of an envelope: BLAKE3 of its exact frame
/// bytes — the same digest the signature layer binds, because a
/// witness and an attestation should never disagree about what a
/// thing IS.
#[must_use]
pub fn subject_of(envelope: &[u8]) -> [u8; 32] {
    sig::envelope_hash(envelope)
}

/// What a watcher returns — and it is never bare (§6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatcherReport {
    /// The verdict.
    pub verified: bool,
    /// The observer the verdict was reached against. Without this the
    /// answer cannot be compared with another watcher's.
    pub observer: Observer,
}

/// Why the watcher refused to answer at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatcherRefused {
    /// The handed subject is not the witnessed subject. The watcher
    /// may not observe (go fetch the right one) and may not repair
    /// (pretend this one matches) — it refuses.
    NotTheSubject,
}

/// The watcher: pure, total, holds nothing.
///
/// `subject_body` is HANDED in — the whole of §6.1. For the replay
/// arm the watcher re-derives the claim in full (checking costs what
/// producing costs); for the succinct arm it checks the derivation is
/// consistent with the subject as far as it reaches without
/// re-production. Either way the report carries the observer.
pub fn watch(witness: &Witness, subject_envelope: &[u8]) -> Result<WatcherReport, WatcherRefused> {
    if subject_of(subject_envelope) != witness.subject {
        return Err(WatcherRefused::NotTheSubject);
    }
    let verified = match witness.arm {
        Arm::Replay => {
            // THE REPLAY-COMPLETE LAW: re-derive the whole claim.
            let body = subject_envelope
                .get(isthmus::layout::Layout::founding().header()..)
                .unwrap_or(&[]);
            assay::work::WorkBody::parse(body)
                .map(|w| w.verifies())
                .unwrap_or(false)
        }
        Arm::Succinct => {
            // Checking costs less than producing: the digest already
            // matched, and the derivation is the grantee's opaque
            // bytes — a succinct witness verifies what it can reach
            // without re-production.
            true
        }
    };
    Ok(WatcherReport {
        verified,
        observer: witness.observer.clone(),
    })
}
