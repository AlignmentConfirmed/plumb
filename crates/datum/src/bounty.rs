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
