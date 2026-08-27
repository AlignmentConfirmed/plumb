//! UC4 + UC6 — registered domains, resolved from the chain; fuel
//! priced like any other axis.
//!
//! ```text
//! holder ──Act::Declare(tag, definition)──► chain
//!                                             │
//! claim under tag ──► court: resolve tag ─────┘
//!                     definition from CHAIN STATE — no rebuild
//!                     claim's universe must BE the registered one
//!                     witness must close, under a priced fuel budget
//! ```
//!
//! The resolver's rule lives in `isthmus::deed::Ledger::declaration_of`:
//! a definition counts only while its declarer holds the tag's live
//! deed — a vocabulary does not outlive its grant.

use assay::complex::{ComplexBroken, DeclaredClaim, DeclaredComplex};
use isthmus::deed::Ledger;
use isthmus::layout::Tag;

use crate::extent::Extent;

/// Why a claim under a registered tag refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainRefused {
    /// No current holder of the tag has published a definition. An
    /// unregistered tag is not an error on the wire — carriers still
    /// forward it — but a court cannot judge what nothing defines.
    Unregistered,
    /// The registered definition does not decode as a complex.
    /// Registration is not trust: publishing nonsense buys the right
    /// to have nonsense refused.
    BadDefinition(ComplexBroken),
    /// The claim's body is not a declared-domain body.
    NotDeclared(ComplexBroken),
    /// The claim declares a different universe than the chain
    /// registered for this tag. Closing in your own private geometry
    /// proves nothing about the registered one.
    WrongUniverse,
    /// The claim refused under the registered universe (open
    /// boundary, non-canonical, fuel — the evaluator's own names).
    Broken(ComplexBroken),
}

/// Verify a claim under a **registered** tag, against chain state
/// alone, within `fuel`. Returns the fuel actually spent — what the
/// board prices.
pub fn verify_registered(
    ledger: &Ledger,
    tag: Tag,
    body: &[u8],
    fuel: u64,
) -> Result<u64, DomainRefused> {
    let definition = ledger
        .declaration_of(tag)
        .ok_or(DomainRefused::Unregistered)?;
    let registered =
        DeclaredComplex::decode(&definition).map_err(DomainRefused::BadDefinition)?;
    let claim = DeclaredClaim::decode(body).map_err(DomainRefused::NotDeclared)?;
    if claim.complex != registered {
        return Err(DomainRefused::WrongUniverse);
    }
    claim.verify(fuel).map_err(DomainRefused::Broken)
}

/// UC6 — the fuel budget a priced space grants, read off a board
/// price's axis. A space that never priced fuel grants nothing, and
/// an evaluation over the grant refuses **with the budget named**
/// ([`ComplexBroken::FuelExhausted`]).
#[must_use]
pub fn fuel_budget(price: &Extent, axis: usize) -> u64 {
    price
        .components()
        .get(axis)
        .copied()
        .map(|c| u64::try_from(c).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
