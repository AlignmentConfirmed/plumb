//! Independent node roles on a decentralized mesh.
//!
//! There is no coordinator and no node identity registry. A peer is
//! what it can demonstrate. Any process may hold any role; roles are
//! capabilities, not ranks.
//!
//! | role | does | does not |
//! |---|---|---|
//! | [`Role::Producer`] | multi-axial work bodies | settle rewards |
//! | [`Role::Verifier`] | re-derive claims, emit receipts | trust a handed token |
//! | [`Role::Carrier`] | forward frames it does not own | inspect payloads |

use crate::frame::Malformed;
use crate::layout::Tag;
use crate::work::{self, Envelope};

/// What this process is doing on the highway right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Builds multi-axial work claim bodies (closed boundaries).
    Producer,
    /// Re-derives work and emits watcher receipts.
    Verifier,
    /// Forwards opaque frames; never reads values it does not own.
    Carrier,
}

impl Role {
    /// Whether this role may attempt to **produce** claim bodies.
    #[must_use]
    pub fn produces(self) -> bool {
        matches!(self, Role::Producer)
    }

    /// Whether this role may attempt to **verify** claim bodies.
    #[must_use]
    pub fn verifies(self) -> bool {
        matches!(self, Role::Verifier)
    }

    /// Whether this role may **forward** unknown frames.
    #[must_use]
    pub fn carries(self) -> bool {
        matches!(self, Role::Carrier)
    }
}

/// A carrier's only lawful action on arriving bytes: classify and, if
/// not owned, expose the whole record for forwarding.
///
/// Refuses malformed headers. Never returns a proof verdict.
pub fn carrier_step(bytes: &[u8]) -> Result<CarrierOut<'_>, Malformed> {
    match work::classify(bytes)? {
        Envelope::Mine { tag, body } => Ok(CarrierOut::Deliver { tag, body }),
        Envelope::Forward { whole } => Ok(CarrierOut::Forward { whole }),
    }
}

/// What a carrier does next with one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierOut<'a> {
    /// Work envelope for a local producer/verifier (still opaque here).
    Deliver {
        /// Claim or receipt tag.
        tag: Tag,
        /// Opaque body.
        body: &'a [u8],
    },
    /// Forward these exact bytes unchanged.
    Forward {
        /// Whole record.
        whole: &'a [u8],
    },
}
