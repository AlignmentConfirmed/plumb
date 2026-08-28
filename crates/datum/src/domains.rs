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

/// The n-cycle universe: n vertices, n edges in a ring. The demo
/// family for proofnet and simnet — every n is a DIFFERENT structure,
/// so a client that walks n produces fresh work each round while a
/// repeated n refuses as replay.
#[must_use]
pub fn demo_cycle_universe(n: u32) -> DeclaredComplex {
    let n = n.max(3);
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

/// The full n-cycle claim, which closes.
#[must_use]
pub fn demo_cycle_claim(n: u32, transport: u64) -> DeclaredClaim {
    demo_cycle_claim_charged(n, 1, transport)
}

/// The n-cycle with charge `k` on every edge — still a perfect cycle,
/// and a DIFFERENT structure for every `k`. This is how a client
/// produces unbounded fresh work inside a bounded record size: walk
/// `n` to a bound-safe cap, then lap with a new charge. (The audit's
/// lesson: a client that only grows eventually outgrows every bound.)
#[must_use]
pub fn demo_cycle_claim_charged(n: u32, charge: i64, transport: u64) -> DeclaredClaim {
    let n = n.max(3);
    let charge = if charge == 0 { 1 } else { charge };
    DeclaredClaim {
        transport,
        complex: demo_cycle_universe(n),
        dim: 1,
        witness: (0..n).map(|i| (i, assay::whole(charge))).collect(),
    }
}

/// The hexagon: the 6-cycle, kept by name for the tests and docs.
#[must_use]
pub fn demo_hexagon_universe() -> DeclaredComplex {
    demo_cycle_universe(6)
}

/// The hexagon claim.
#[must_use]
pub fn demo_hexagon_claim(transport: u64) -> DeclaredClaim {
    demo_cycle_claim(6, transport)
}

/// The theta universe: two vertices, three parallel edges — a cycle
/// space wide enough to hold genuinely leaner and fatter closures,
/// which is what the optimization market's measurements select
/// between.
#[must_use]
pub fn demo_theta_universe() -> DeclaredComplex {
    let mut op = Vec::new();
    for edge in 0..3u32 {
        op.push(assay::complex::Entry { row: 0, col: edge, coeff: assay::whole(-1) });
        op.push(assay::complex::Entry { row: 1, col: edge, coeff: assay::whole(1) });
    }
    DeclaredComplex {
        cells: vec![2, 3],
        ops: vec![op],
    }
}

/// Theta with a filling: one 2-cell f with boundary 2·e1 − 2·e2, so
/// theta's fat and lean cycles are genuinely homologous and a
/// homology certificate has something true to prove.
#[must_use]
pub fn demo_theta_filled_universe() -> DeclaredComplex {
    let mut theta = demo_theta_universe();
    theta.cells.push(1);
    theta.ops.push(vec![
        assay::complex::Entry { row: 1, col: 0, coeff: assay::whole(2) },
        assay::complex::Entry { row: 2, col: 0, coeff: assay::whole(-2) },
    ]);
    theta
}
