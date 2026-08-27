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

/// The demo universe: six vertices, six edges in a cycle. A fixture
/// for the proofnet script and the tests — the first universe a beta
/// court registers, and small enough to read.
#[must_use]
pub fn demo_hexagon_universe() -> DeclaredComplex {
    let n = 6u32;
    let mut op = Vec::new();
    for i in 0..n {
        let (source, target) = (i, (i + 1) % n);
        let mut pair = vec![
            assay::complex::Entry {
                row: target,
                col: i,
                coeff: assay::whole(1),
            },
            assay::complex::Entry {
                row: source,
                col: i,
                coeff: assay::whole(-1),
            },
        ];
        pair.sort_by_key(|e| (e.col, e.row));
        op.extend(pair);
    }
    DeclaredComplex {
        cells: vec![n, n],
        ops: vec![op],
    }
}

/// The demo claim: the full hexagon cycle, which closes.
#[must_use]
pub fn demo_hexagon_claim(transport: u64) -> DeclaredClaim {
    DeclaredClaim {
        transport,
        complex: demo_hexagon_universe(),
        dim: 1,
        witness: (0..6).map(|i| (i, assay::whole(1))).collect(),
    }
}
