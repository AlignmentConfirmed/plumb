//! O1 — the yield rebate: elegance paid at discovery.
//!
//! On a **demand-posed** space, the payout is a function of measured
//! efficiency:
//!
//! ```text
//! payout = base + (max_fuel − spent_fuel)·r_f + (max_bytes − spent_bytes)·r_b
//! ```
//!
//! Both savings are **consensus facts**: metered fuel is a
//! deterministic function of witness structure (every court re-derives
//! the identical number), and canonical byte length is canonical.
//! Never cpu cycles, never memory — machine facts are unverifiable by
//! a federation.
//!
//! The one hard gate: **the universe must be the poser's.** An answer
//! in any other universe refuses, however beautifully it closes —
//! on self-posed work a rebate is free money, and the strand corpus
//! recorded why: *a node authoring its own task solves it for free.*

use assay::complex::{ComplexBroken, DeclaredClaim};

use crate::query::Query;
use crate::reward::{Credit, RewardBook, RewardRefused};

/// The priced budget and rates of one demand-posed space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounty {
    /// The question this bounty funds (X1).
    pub query_id: [u8; 32],
    /// Largest verification fuel the space pays for.
    pub max_fuel: u64,
    /// Largest claim body, in canonical bytes.
    pub max_bytes: u64,
    /// The guaranteed payout for simply closing the boundary.
    pub base: u128,
    /// Yield per unit of unspent fuel.
    pub per_saved_fuel: u128,
    /// Yield per unspent byte.
    pub per_saved_byte: u128,
}

impl Bounty {
    /// What the poser must escrow: the payout at the (unreachable)
    /// perfect answer. Bounded, so underwriting is well-defined; the
    /// residue refunds.
    #[must_use]
    pub fn escrow_bound(&self) -> u128 {
        self.base
            .saturating_add(u128::from(self.max_fuel).saturating_mul(self.per_saved_fuel))
            .saturating_add(u128::from(self.max_bytes).saturating_mul(self.per_saved_byte))
    }

    /// The payout for a measured answer. Saturating — exact until
    /// astronomically large, never wrapping.
    #[must_use]
    pub fn payout(&self, spent_fuel: u64, spent_bytes: u64) -> u128 {
        let saved_fuel = u128::from(self.max_fuel.saturating_sub(spent_fuel));
        let saved_bytes = u128::from(self.max_bytes.saturating_sub(spent_bytes));
        self.base
            .saturating_add(saved_fuel.saturating_mul(self.per_saved_fuel))
            .saturating_add(saved_bytes.saturating_mul(self.per_saved_byte))
    }
}

/// Why an answer to a bounty refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerRefused {
    /// The body is not a declared-domain claim — a bounty prices a
    /// declared universe, so the answer must live in one.
    NotDeclared(ComplexBroken),
    /// The answer's universe is not the one the POSER fixed. However
    /// beautifully it closes, closing your own geometry earns no
    /// rebate here.
    NotThePosersUniverse,
    /// The body exceeds the priced byte budget — refused with the
    /// price named, like fuel.
    Oversized {
        /// The priced budget.
        max_bytes: u64,
        /// What arrived.
        got: u64,
    },
    /// Verification refused (open boundary, fuel exhaustion with its
    /// budget named, non-canonical — the evaluator's own refusals).
    Broken(ComplexBroken),
    /// The book refused (replay, unsettled dependency).
    Book(RewardRefused),
}

/// What a settled answer earned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The credit on the book, as any settlement.
    pub credit: Credit,
    /// Verification fuel actually spent — the consensus meter.
    pub spent_fuel: u64,
    /// Canonical body size.
    pub spent_bytes: u64,
    /// The payout, base plus yield.
    pub payout: u128,
}

/// Settle a claim body against a demand-posed bounty.
///
/// The order is deliberate: the poser's-universe gate first (cheap,
/// and the rule that makes rebates sane), then the byte budget, then
/// metered verification under the fuel budget, then the book (replay
/// law and all), then the payout arithmetic.
pub fn settle_answer(
    bounty: &Bounty,
    query: &Query,
    body: &[u8],
    book: &mut RewardBook,
) -> Result<Answer, AnswerRefused> {
    let claim = DeclaredClaim::decode(body).map_err(AnswerRefused::NotDeclared)?;
    if claim.complex.encode() != query.statement {
        return Err(AnswerRefused::NotThePosersUniverse);
    }
    let got = body.len() as u64;
    if got > bounty.max_bytes {
        return Err(AnswerRefused::Oversized {
            max_bytes: bounty.max_bytes,
            got,
        });
    }
    let spent_fuel = claim
        .verify(bounty.max_fuel)
        .map_err(AnswerRefused::Broken)?;
    let credit = book.credit_claim(body).map_err(AnswerRefused::Book)?;
    let payout = bounty.payout(spent_fuel, got);
    Ok(Answer {
        credit,
        spent_fuel,
        spent_bytes: got,
        payout,
    })
}

use assay::complex::ProofClaim;
use assay::work::WorkId;
use assay::Exact;

/// O2 — a standing bounty on a settled work: exhibit a chain closing
/// the same boundary in the same universe, at least this much leaner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementBounty {
    /// The settled work to refine.
    pub target: WorkId,
    /// The strict improvement threshold, in percent of the original's
    /// verification fuel — the anti-dust rule (O3 of the docket).
    pub min_improvement_percent: u8,
    /// The standing reward.
    pub reward: u128,
}

/// Why a refinement refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefineRefused {
    /// The bounty targets work this book has not settled.
    UnsettledTarget,
    /// The target's content address does not decode as a declared
    /// claim — nothing to re-meter against.
    TargetNotDeclared(ComplexBroken),
    /// The refinement body is not a declared claim.
    NotDeclared(ComplexBroken),
    /// The refinement lives in a different universe than the target.
    NotTheSameUniverse,
    /// The refinement is not leaner by the threshold — refused with
    /// every number named, so an almost-improvement knows exactly how
    /// far it fell short.
    NotAnImprovement {
        /// Required percent.
        needed_percent: u8,
        /// The original's measured fuel.
        original_fuel: u64,
        /// The refinement's measured fuel.
        refined_fuel: u64,
    },
    /// Fewer fuel units but more bytes: a trade, not a refinement.
    ByteRegression {
        /// The original's canonical size.
        original_bytes: u64,
        /// The refinement's.
        refined_bytes: u64,
    },
    /// The refinement itself refused verification.
    Broken(ComplexBroken),
    /// The book refused (replay: an identical resubmission earns
    /// nothing, exactly as the docket demands).
    Book(RewardRefused),
    /// O4 — the homology certificate does not fill the difference:
    /// its prescribed boundary must be exactly `original − refined`.
    CertificateWrongDifference,
    /// O4 — the certificate refused verification.
    CertificateBroken(ComplexBroken),
    /// O4 — the certificate lives in a different universe.
    CertificateWrongUniverse,
}

/// What a settled refinement earned and recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refined {
    /// The new credit — the leaner chain is work in its own right.
    pub credit: Credit,
    /// Fuel saved, measured (original re-metered from its own
    /// content address — the ledger stores no costs; it stores the
    /// structures, and costs re-derive).
    pub saved_fuel: u64,
    /// Bytes saved.
    pub saved_bytes: u64,
    /// The standing reward.
    pub payout: u128,
    /// O4 — whether a homology certificate verified: the refinement
    /// is not merely same-boundary but provably the same class,
    /// `∂h = original − refined` exhibited and checked.
    pub homologous: bool,
}

/// The sparse difference of two canonical chains, canonical.
fn chain_difference(
    a: &[(u32, Exact)],
    b: &[(u32, Exact)],
) -> Vec<(u32, Exact)> {
    let mut acc: std::collections::BTreeMap<u32, Exact> = std::collections::BTreeMap::new();
    for (cell, coeff) in a {
        let slot = acc.entry(*cell).or_insert_with(|| assay::whole(0));
        *slot += coeff.clone();
    }
    for (cell, coeff) in b {
        let slot = acc.entry(*cell).or_insert_with(|| assay::whole(0));
        *slot -= coeff.clone();
    }
    acc.into_iter()
        .filter(|(_, coeff)| !num_traits::Zero::is_zero(coeff))
        .collect()
}

/// Settle a refinement against a standing bounty.
///
/// The original's cost is **re-derived from its content address** —
/// a `work_id` IS the canonical structure, so the ledger needs no
/// cost table: decode the settled original, re-meter it, compare.
pub fn settle_refinement(
    bounty: &RefinementBounty,
    body: &[u8],
    certificate: Option<&[u8]>,
    book: &mut RewardBook,
    fuel_ceiling: u64,
) -> Result<Refined, RefineRefused> {
    if !book.seen().contains(&bounty.target) {
        return Err(RefineRefused::UnsettledTarget);
    }
    let original = DeclaredClaim::decode(bounty.target.as_bytes())
        .map_err(RefineRefused::TargetNotDeclared)?;
    let refined = DeclaredClaim::decode(body).map_err(RefineRefused::NotDeclared)?;
    if refined.complex != original.complex || refined.dim != original.dim {
        return Err(RefineRefused::NotTheSameUniverse);
    }

    let original_fuel = original
        .verify(fuel_ceiling)
        .map_err(RefineRefused::TargetNotDeclared)?;
    let refined_fuel = refined.verify(fuel_ceiling).map_err(RefineRefused::Broken)?;

    // The strict threshold: refined ≤ original · (100 − N) / 100,
    // in exact integer arithmetic.
    let needed = bounty.min_improvement_percent;
    let lhs = u128::from(refined_fuel).saturating_mul(100);
    let rhs = u128::from(original_fuel).saturating_mul(u128::from(100u8.saturating_sub(needed)));
    if lhs > rhs {
        return Err(RefineRefused::NotAnImprovement {
            needed_percent: needed,
            original_fuel,
            refined_fuel,
        });
    }
    let original_bytes = bounty.target.as_bytes().len() as u64;
    let refined_bytes = body.len() as u64;
    if refined_bytes > original_bytes {
        return Err(RefineRefused::ByteRegression {
            original_bytes,
            refined_bytes,
        });
    }

    // O4 — the optional quality tier: a certificate is a PROOF CLAIM
    // whose prescribed boundary is exactly original − refined, one
    // dimension up. The SQ1 evaluator does the rest.
    let homologous = match certificate {
        None => false,
        Some(cert_body) => {
            let proof =
                ProofClaim::decode(cert_body).map_err(RefineRefused::CertificateBroken)?;
            if proof.complex != original.complex {
                return Err(RefineRefused::CertificateWrongUniverse);
            }
            let difference = chain_difference(&original.witness, &refined.witness);
            if proof.dim != original.dim.saturating_add(1) || proof.target != difference {
                return Err(RefineRefused::CertificateWrongDifference);
            }
            proof
                .verify(fuel_ceiling)
                .map_err(RefineRefused::CertificateBroken)?;
            true
        }
    };

    // The leaner chain is new work by content address (T2 untouched);
    // an identical resubmission refuses right here as replay.
    let credit = book.credit_claim(body).map_err(RefineRefused::Book)?;
    let saved_fuel = original_fuel.saturating_sub(refined_fuel);
    let saved_bytes = original_bytes.saturating_sub(refined_bytes);

    // O3 — the append, never a rewrite.
    book.record_equivalence(
        bounty.target.clone(),
        credit.work_id.clone(),
        saved_fuel,
        saved_bytes,
    )
    .map_err(RefineRefused::Book)?;

    Ok(Refined {
        credit,
        saved_fuel,
        saved_bytes,
        payout: bounty.reward,
        homologous,
    })
}
