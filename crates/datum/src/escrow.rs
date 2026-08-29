//! Court-side balance accounting over the chain's stake acts (`IS-6/7`,
//! Phase 5 Fork B).
//!
//! The chain records how much a holder has **locked**
//! ([`Ledger::escrow_of`]) and how much has been **slashed**
//! ([`Ledger::slashed_of`]) — both pure folds over `Act::Escrow` /
//! `Release` / `Slash`. What the chain deliberately does not record is
//! how much a holder has **earned**: the reward book credits work by
//! content address, not by name (crate docs: rewards are *"not a
//! name"*). So a holder's spendable balance is not a pure chain fold —
//! it is `earned − locked − slashed`, and `earned` must be supplied by
//! the court from its own accounting.
//!
//! This module is the arithmetic and the two rules a court enforces (a
//! lock may not exceed available balance; a slash may not exceed what is
//! locked). Building the [`Act`] is separate from appending it: the
//! caller records the returned act on its ledger, exactly as
//! registration does with a live bind. where `earned` comes from — and
//! whether attributing earned credit per holder is worth changing the
//! name-free settlement record — is the court's wiring, tracked at #51.

use isthmus::deed::{Act, Ledger};

/// A stake act a court refused to build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscrowRefused {
    /// A lock would exceed the holder's available (unlocked, unslashed)
    /// balance — the Fork-B guarantee that a stake is backed by real
    /// earned value, never conjured.
    InsufficientBalance {
        /// What the holder could still lock.
        available: u128,
        /// What the holder asked to lock.
        requested: u128,
    },
    /// A slash would destroy more than the holder has locked — a slash
    /// cannot manufacture a debt, only burn an existing stake.
    NothingToSlash {
        /// What the holder currently has locked.
        locked: u128,
        /// What the slash asked to destroy.
        requested: u128,
    },
}

/// A holder's spendable balance given what it has `earned`:
/// `available = earned − locked − slashed`. Saturating throughout, so a
/// court whose `earned` figure lags the chain reads zero rather than
/// underflowing — it never invents balance.
#[must_use]
pub fn available_balance(earned: u128, ledger: &Ledger, holder: &str) -> u128 {
    earned
        .saturating_sub(ledger.escrow_of(holder))
        .saturating_sub(ledger.slashed_of(holder))
}

/// Build the [`Act::Escrow`] for a voluntary self-lock, refusing if it
/// would exceed the holder's available balance. The court appends the
/// returned act to its ledger; the balance check is the court's rule,
/// not the chain's (the chain only records that the holder locked this).
///
/// # Errors
/// [`EscrowRefused::InsufficientBalance`] if `amount` exceeds
/// [`available_balance`].
pub fn lock(earned: u128, ledger: &Ledger, holder: &str, amount: u128) -> Result<Act, EscrowRefused> {
    let available = available_balance(earned, ledger, holder);
    if amount > available {
        return Err(EscrowRefused::InsufficientBalance {
            available,
            requested: amount,
        });
    }
    Ok(Act::Escrow {
        holder: holder.to_owned(),
        amount,
    })
}

/// Build the [`Act::Release`] that unlocks a holder's entire stake back
/// to spendable balance — a refund, always available (a holder may
/// always stop buying priority).
#[must_use]
pub fn release(holder: &str) -> Act {
    Act::Release {
        holder: holder.to_owned(),
    }
}

/// Build the [`Act::Slash`] destroying `amount` of a holder's locked
/// stake, refusing if it exceeds what is locked. The caller must have
/// already verified the consensus-verifiable offence (#56 — an
/// attested-but-false proof, a double submission, a broken signed
/// commitment); this enforces only the arithmetic floor, that a slash
/// cannot destroy more stake than exists.
///
/// # Errors
/// [`EscrowRefused::NothingToSlash`] if `amount` exceeds the holder's
/// currently locked stake.
pub fn slash(ledger: &Ledger, holder: &str, amount: u128) -> Result<Act, EscrowRefused> {
    let locked = ledger.escrow_of(holder);
    if amount > locked {
        return Err(EscrowRefused::NothingToSlash {
            locked,
            requested: amount,
        });
    }
    Ok(Act::Slash {
        holder: holder.to_owned(),
        amount,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use isthmus::layout::Layout;

    fn ledger() -> Ledger {
        Ledger::new(Layout::founding())
    }

    #[test]
    fn available_balance_is_earned_minus_locked_minus_slashed() {
        let mut l = ledger();
        // Earned 100, nothing locked → all available.
        assert_eq!(available_balance(100, &l, "a"), 100);

        l.record(lock(100, &l, "a", 40).expect("within balance"));
        assert_eq!(l.escrow_of("a"), 40);
        assert_eq!(available_balance(100, &l, "a"), 60, "locked is not spendable");

        l.record(slash(&l, "a", 10).expect("within locked"));
        // locked now 30, slashed 10 → available 100 − 30 − 10 = 60.
        assert_eq!(available_balance(100, &l, "a"), 60);

        l.record(release("a"));
        // locked 0, slashed 10 (permanent) → available 90.
        assert_eq!(available_balance(100, &l, "a"), 90);
    }

    #[test]
    fn a_lock_cannot_exceed_available_balance() {
        let mut l = ledger();
        // Lock 60 of 100, leaving 40 available.
        l.record(lock(100, &l, "a", 60).expect("first lock fits"));
        // A second lock of 50 must refuse: only 40 remains.
        let refused = lock(100, &l, "a", 50);
        assert_eq!(
            refused,
            Err(EscrowRefused::InsufficientBalance {
                available: 40,
                requested: 50,
            }),
            "a stake must be backed by earned balance, never conjured"
        );
        // Exactly the remaining 40 is allowed.
        assert!(lock(100, &l, "a", 40).is_ok());
    }

    #[test]
    fn a_lock_of_zero_earned_always_refuses_a_positive_amount() {
        let l = ledger();
        assert_eq!(
            lock(0, &l, "newcomer", 1),
            Err(EscrowRefused::InsufficientBalance {
                available: 0,
                requested: 1,
            }),
            "a holder that has earned nothing can stake nothing"
        );
    }

    #[test]
    fn a_slash_cannot_exceed_what_is_locked() {
        let mut l = ledger();
        l.record(lock(100, &l, "a", 30).expect("fits"));
        assert_eq!(
            slash(&l, "a", 31),
            Err(EscrowRefused::NothingToSlash {
                locked: 30,
                requested: 31,
            }),
            "a slash burns an existing stake, it cannot manufacture a debt"
        );
        assert!(slash(&l, "a", 30).is_ok(), "the whole stake may be slashed");
    }
}
