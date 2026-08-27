//! Block production: append settlement acts to a court chain (H5).
//!
//! Independent nodes propose acts; the authority accepts only well-formed
//! futures (same induction the board uses). Nothing is erased — only
//! appended. This is the highway's "block" in miniature: a batch of
//! acts that advances the append-only record.
//!
//! Settlement of useful work lands here: a funded board proposal's
//! `acts` are the payload of a block (`settle::land`).

use isthmus::deed::{Act, Flaw, Ledger};

/// Why a block was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRefused {
    /// Empty batch — a block that changes nothing is not a production.
    Empty,
    /// Future chain after append is not well-formed.
    Invalid(Flaw),
}

/// A produced block: acts that landed, in order.
#[derive(Debug, Clone)]
pub struct Block {
    /// Acts appended by this production.
    pub acts: Vec<Act>,
    /// Court act count after production.
    pub height: usize,
    /// Height before this block (exclusive start of the batch).
    pub prior_height: usize,
    /// The court after the block (replay of prior + new acts).
    pub court: Ledger,
}

/// Produce a block: prior court acts ‖ new acts must be well-formed.
///
/// Returns the new court; the caller replaces their court with it.
/// Does not mutate the input.
pub fn produce(court: &Ledger, acts: Vec<Act>) -> Result<Block, BlockRefused> {
    if acts.is_empty() {
        return Err(BlockRefused::Empty);
    }
    let prior_height = court.acts().len();
    let mut history: Vec<Act> = court.acts().to_vec();
    history.extend(acts.iter().cloned());
    let future = Ledger::replay(court.layout().clone(), history);
    future.well_formed().map_err(BlockRefused::Invalid)?;
    Ok(Block {
        prior_height,
        height: future.acts().len(),
        court: future,
        acts,
    })
}

/// Whether this block advanced the chain (at least one new act).
impl Block {
    /// Number of acts this block contributed.
    pub fn batch_len(&self) -> usize {
        self.acts.len()
    }

    /// Acts in the court that belong to this block only.
    pub fn landed_slice(&self) -> &[Act] {
        self.court
            .acts()
            .get(self.prior_height..)
            .unwrap_or(&[])
    }
}
