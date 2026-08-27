//! Credit useful work against **deed-priced multi-axis space**, then
//! land the grant as a **block** of settlement acts.
//!
//! ```text
//! work body → RewardBook::credit_claim   (standing / per-axis credit)
//!          → settle_against(proposal.price)   (price is Extent, not u128)
//!          → board::clears + validate
//!          → block::produce(proposal.acts)    (append-only court growth)
//! ```
//!
//! **H4 — standing vs estate.**
//! - **Standing** is cumulative multi-axis credit in [`RewardBook`]:
//!   what useful work has earned, per axis / per orb, never a scalar
//!   Υ token and never a product across axes.
//! - **Estate** is the deed-priced region: survey yields an [`Extent`]
//!   price (one component per axis). Funding is
//!   `price.fits_in(credit)` on every axis — partial orders, not
//!   folded volume. Incomparable estates (`[2,8]` vs `[4,4]`) do not
//!   clear each other.
//!
//! **H5 — settlement is on-chain growth.**
//! When work settles space, the court grows by appending the proposal's
//! acts through [`crate::block::produce`]. Credit without
//! [`land`] / [`block::produce`] is incomplete: the grant must become
//! settlement acts at a height. Nothing is erased.

use isthmus::deed::Ledger;

use crate::block::{self, Block, BlockRefused};
use crate::board::{self, Application, EnactRefused, Proposal};
use crate::extent::Extent;
use crate::merge::{self, MergeAdmit, MergePayout, MergeRefused};
use crate::negotiation::Position;
use crate::reward::{Credit, RewardBook, RewardRefused};

/// Why pay-and-enact / land failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettleRefused {
    /// Work credit path refused.
    Work(RewardRefused),
    /// Board enactment refused (counter or invalid future chain).
    Board(EnactRefused),
    /// Block production refused (should not happen after validate).
    Block(BlockRefused),
}

/// Credit one work body into the book.
pub fn credit(book: &mut RewardBook, body: &[u8]) -> Result<Credit, SettleRefused> {
    book.credit_claim(body).map_err(SettleRefused::Work)
}

/// Require the book to cover `price` on every axis.
pub fn require_cover(book: &RewardBook, price: &Extent) -> Result<(), SettleRefused> {
    book.settle_against(price).map_err(SettleRefused::Work)
}

/// After credit covers the survey price and the position clears the ask,
/// enact the proposal onto a new court (in-memory ledger).
pub fn enact_if_funded(
    book: &RewardBook,
    court: &Ledger,
    proposal: &Proposal,
    position: &Position,
) -> Result<Ledger, SettleRefused> {
    require_cover(book, &proposal.price)?;
    board::enact(court, proposal, position).map_err(SettleRefused::Board)
}

/// Survey → credit work → cover multi-axis price → **produce a block**
/// of the proposal's acts (H4 + H5 in one path).
///
/// Returns the proposal, the produced block (new court + acts), and the
/// credits applied from this call.
pub fn land(
    book: &mut RewardBook,
    court: &Ledger,
    application: &Application,
    work_bodies: &[&[u8]],
) -> Result<(Proposal, Block, Vec<Credit>), SettleRefused> {
    let proposal = board::survey(court, application)
        .map_err(|t| SettleRefused::Board(EnactRefused::Turned(t)))?;
    let credits = credit_stack(book, work_bodies)?;
    require_cover(book, &proposal.price)?;
    board::clears(&proposal, &application.position)
        .map_err(|c| SettleRefused::Board(EnactRefused::NotCleared(c)))?;
    if let Err(turned) = board::validate(court, &proposal) {
        return Err(SettleRefused::Board(EnactRefused::Turned(turned)));
    }
    let block = block::produce(court, proposal.acts.clone()).map_err(SettleRefused::Block)?;
    Ok((proposal, block, credits))
}

/// Survey → credit work bodies until price is covered → enact (ledger only).
///
/// Prefer [`land`] when the authority wants an explicit block record.
pub fn join_with_work(
    book: &mut RewardBook,
    court: &Ledger,
    application: &Application,
    work_bodies: &[&[u8]],
) -> Result<(Proposal, Ledger, Vec<Credit>), SettleRefused> {
    let proposal = board::survey(court, application)
        .map_err(|t| SettleRefused::Board(EnactRefused::Turned(t)))?;
    let credits = credit_stack(book, work_bodies)?;
    let court = enact_if_funded(book, court, &proposal, &application.position)?;
    Ok((proposal, court, credits))
}

fn credit_stack(
    book: &mut RewardBook,
    work_bodies: &[&[u8]],
) -> Result<Vec<Credit>, SettleRefused> {
    let mut credits = Vec::new();
    for body in work_bodies {
        match book.credit_claim(body) {
            Ok(c) => credits.push(c),
            // Already-seen structure is not fatal when stacking a list
            // that may include duplicates; coverage is checked after.
            Err(RewardRefused::Replay { .. }) => {}
            Err(e) => return Err(SettleRefused::Work(e)),
        }
    }
    Ok(credits)
}

/// Why sphere-merge settlement failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeSettleRefused {
    /// Merge admit / split refused.
    Merge(MergeRefused),
    /// Principal book cannot cover a stated price on S*.
    Principal(RewardRefused),
}

/// Inputs for [`settle_merge`] (SM8–SM9).
pub struct MergeSettle<'a> {
    /// Multi-axial capacity of sphere A.
    pub a: &'a Extent,
    /// Multi-axial capacity of sphere B.
    pub b: &'a Extent,
    /// Optional PoWC boundaries (both must Closed).
    pub powc: Option<(&'a assay::Boundary, &'a assay::Boundary)>,
    /// Optional PoUW domain-tagged bodies.
    pub pouw: Option<(&'a [u8], &'a [u8])>,
    /// Sphere precedence for standoff discipline.
    pub precedence: isthmus::sphere::Precedence,
    /// Allow merge despite ordered fault.
    pub allow_ordered: bool,
    /// Optional price on S* the principal book must cover after allocate.
    pub price_on_star: Option<&'a Extent>,
}

/// Admit merge, derive payout, route principal + carry to three books (SM8–SM9).
///
/// - `principal` receives bulk S* (converged sphere economics)
/// - `carry_a` / `carry_b` receive residual vectors (carry payout)
pub fn settle_merge(
    req: &MergeSettle<'_>,
    principal: &mut RewardBook,
    carry_a: &mut RewardBook,
    carry_b: &mut RewardBook,
) -> Result<(MergeAdmit, MergePayout), MergeSettleRefused> {
    let admit = merge::admit_merge(
        req.a,
        req.b,
        req.powc,
        req.pouw,
        req.precedence,
        req.allow_ordered,
    )
    .map_err(MergeSettleRefused::Merge)?;
    let payout = MergePayout::from_split(&admit.split);
    merge::apply_payout(principal, carry_a, carry_b, &payout);
    if let Some(price) = req.price_on_star {
        principal
            .settle_against(price)
            .map_err(MergeSettleRefused::Principal)?;
    }
    Ok((admit, payout))
}

/// Document: price and credit are both multi-axis; scalar product is absent.
#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod h4_discipline {
    use super::*;
    use crate::extent::Extent;

    #[test]
    fn incomparable_prices_are_not_equal() {
        let a = Extent::new(vec![2, 8]);
        let b = Extent::new(vec![4, 4]);
        assert!(a.compare(&b).is_none());
        assert!(!a.fits_in(&b));
        assert!(!b.fits_in(&a));
    }

    #[test]
    fn credit_must_match_price_arity() {
        let mut book = RewardBook::new();
        // 2-axis closed box → credit [1,1]
        let body = crate::reward::closed_box_claim(1, 1).encode();
        match book.credit_claim(&body) {
            Ok(_) => {}
            Err(e) => panic!("credit: {e:?}"),
        }
        // 1-axis price cannot be covered by 2-axis credit
        assert!(!book.covers(&Extent::new(vec![1])));
        assert!(book.covers(&Extent::new(vec![1, 1])));
    }
}
