//! `IS-4` §5 — the witness frame.
//!
//! Three roles, and none of them is optional:
//!
//! ```text
//! OBSERVER   holds the subject, at the depth it has standing to see
//! WITNESS    the claim, and the derivation that reached it
//! WATCHER    re-derives, and returns a verdict. Holds nothing.
//! ```
//!
//! This module implements the **witness** — the frame. The observer is
//! an identity the frame *names* (naming is not carrying); the watcher
//! is whoever re-derives, and lives above the substrate (the court's
//! side), bound by `IS-4` §6: may not observe, may not repair, may not
//! require canonical form, may not return a bare verdict.
//!
//! ```text
//! witness  = arm u8 ‖ observer ‖ subject ‖ derivation
//! observer = kind u8 ‖ identity[32] ‖ LE16(len) revision ‖ depth u8
//! subject  = identity[32]
//! ```
//!
//! The frame sits in the granting range of whoever owns the claim —
//! a witness about a claim is a frame of the claim's vocabulary, so
//! this module defines **no tag**.

use crate::frame::Malformed;

/// Which arm a witness is — the watcher's budget, declared up front.
///
/// A replay witness is not a lesser witness; it is a different
/// purchase. Collapsing the two would let a watcher think it had a
/// cheap check when it had an expensive one, so any other byte
/// refuses: this is the budget, and a guess at it is not recoverable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// Checking costs less than producing. Buys economy.
    Succinct,
    /// Checking costs what producing costs. Buys tamper-evidence.
    Replay,
}

/// Who held the subject, at what revision, at what depth.
///
/// The identity is **consulted, not shipped**. The revision is
/// required, never defaulted — a corpus without one names a moving
/// target, and a verdict that cannot say which observer it was
/// reached against cannot be compared with another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observer {
    /// What kind of thing the identity names — a grantee's own code.
    pub kind: u8,
    /// What to consult: a corpus digest, a record address, a
    /// reference cell.
    pub identity: [u8; 32],
    /// Which revision of it. Required; the empty revision refuses.
    pub revision: String,
    /// The depth the claim was reached at. A watcher reading deeper
    /// than the witness was taken at is not checking the same claim.
    pub depth: u8,
}

/// The claim, and the derivation that reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness {
    /// Succinct or replay — declared, never guessed.
    pub arm: Arm,
    /// Who held the subject.
    pub observer: Observer,
    /// What was witnessed, as a 32-byte identity (a digest).
    pub subject: [u8; 32],
    /// The grantee's derivation, opaque here (`IS-3` §5.2: what a
    /// value means inside a granted range is the grantee's).
    pub derivation: Vec<u8>,
}

impl Witness {
    /// Write the frame's *value*. Wrap it with
    /// [`crate::frame::put_frame`] under the claim's own tag.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(match self.arm {
            Arm::Succinct => 0,
            Arm::Replay => 1,
        });
        out.push(self.observer.kind);
        out.extend_from_slice(&self.observer.identity);
        let revision = self.observer.revision.as_bytes();
        let len = u16::try_from(revision.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(revision.get(..usize::from(len)).unwrap_or(revision));
        out.push(self.observer.depth);
        out.extend_from_slice(&self.subject);
        out.extend_from_slice(&self.derivation);
        out
    }

    /// Read a witness value. Refuse-not-repair: an unknown arm, an
    /// absent revision, or a truncated identity each refuse by name —
    /// the derivation is the tail and is whatever remains, including
    /// nothing.
    pub fn decode(value: &[u8]) -> Result<Self, Malformed> {
        let mut at = 0usize;
        let take = |at: &mut usize, n: usize| -> Result<&[u8], Malformed> {
            let end = at.saturating_add(n);
            let piece = value
                .get(*at..end)
                .ok_or(Malformed::TrailingBytes { left: n })?;
            *at = end;
            Ok(piece)
        };
        let arm = match take(&mut at, 1)?.first().copied() {
            Some(0) => Arm::Succinct,
            Some(1) => Arm::Replay,
            _ => return Err(Malformed::TrailingBytes { left: 1 }),
        };
        let kind = *take(&mut at, 1)?.first().unwrap_or(&0);
        let mut identity = [0u8; 32];
        identity.copy_from_slice(take(&mut at, 32)?);
        let len_bytes = take(&mut at, 2)?;
        let len = usize::from(u16::from_le_bytes([
            len_bytes.first().copied().unwrap_or(0),
            len_bytes.get(1).copied().unwrap_or(0),
        ]));
        let revision = String::from_utf8(take(&mut at, len)?.to_vec())
            .map_err(|_| Malformed::TrailingBytes { left: len })?;
        if revision.is_empty() {
            // Required, never defaulted: a corpus without a revision
            // names a moving target.
            return Err(Malformed::TrailingBytes { left: 0 });
        }
        let depth = *take(&mut at, 1)?.first().unwrap_or(&0);
        let mut subject = [0u8; 32];
        subject.copy_from_slice(take(&mut at, 32)?);
        let derivation = value.get(at..).unwrap_or(&[]).to_vec();
        Ok(Self {
            arm,
            observer: Observer {
                kind,
                identity,
                revision,
                depth,
            },
            subject,
            derivation,
        })
    }
}
