//! The negotiation — settlement positions and folds over multi-axial space.
//!
//! Ruled: *you cannot have a scalar in a negotiation, nor a boolean
//! gate hold it — at light speed both get destroyed.*
//!
//! ## Why they are destroyed, stated mechanically
//!
//! A scalar carries no structure to merge: two offers arriving from
//! two directions collapse to one number only by an order-dependent
//! fold. A boolean gate is a verdict about an *instant* — and two
//! parties separated by propagation delay share no instant. Each end
//! fires its gate on the other's stale scalar; the verdict traces
//! diverge; neither party can say which negotiation happened. This is
//! not hypothetical: `tests/negotiation.rs` constructs the old
//! scalar-boolean fold and measures its verdict trace differing under
//! reordering of the same deltas.
//!
//! The repository already holds this law one layer up: revisions are
//! *compared for equality and never ordered*, because ordering across
//! separated parties is authority nobody has. The negotiation gets the
//! same treatment.
//!
//! ## The structure that survives
//!
//! A party's [`Position`] is per-pole offers of exact rationals, and
//! offers **merge by per-pole maximum** — an offer once made is not
//! retracted mid-flight. Max-merge is commutative, associative and
//! idempotent, so the merged position is invariant under every
//! arrival order and every duplication: the light-speed conditions are
//! exactly the ones it is built for. The [`Ask`] is fixed by the
//! survey (geometry does not negotiate); the [`Balance`] is a fold of
//! position against ask, per pole, and *clearing* is a **fixpoint the
//! gate may witness but never conduct**: the chain records a grant
//! when the balance clears, and until then the proposal is a standing
//! position on the docket — open, not refused.
//!
//! Positions are **partially ordered**. A party short on one pole and
//! long on another is incomparable with the ask — not below it — and
//! the board's answer to incomparability is a [`Counter`] naming both
//! sides: what is short, what is long, which is the material of a
//! trade. `Underfunded` — the scalar demand, met-or-refused — is gone.

use std::collections::BTreeMap;

use isthmus::ratio::Exact;
use num_traits::Zero;

/// A pole of value. Named by string until the capacity tranche lands
/// kernel-side names; the structure does not change when they arrive.
pub type Pole = String;

/// One party's standing offers, per pole.
///
/// **Monotone**: merging never lowers a pole. That single property is
/// what buys order-invariance — see [`Position::merge`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Position {
    offered: BTreeMap<Pole, Exact>,
}

impl Position {
    /// An empty position: nothing offered on any pole.
    pub fn new() -> Self {
        Self::default()
    }

    /// Raise this position's offer on one pole.
    ///
    /// A *delta* — the unit that crosses the wire. Deltas may arrive
    /// in any order, duplicated, interleaved with the counterparty's;
    /// the merged position is the same in every case.
    pub fn offer(&mut self, pole: &str, amount: Exact) {
        let slot = self.offered.entry(pole.to_owned()).or_insert_with(Exact::zero);
        if amount > *slot {
            *slot = amount;
        }
    }

    /// Merge another position in: per-pole maximum.
    ///
    /// Commutative, associative, idempotent — the three properties
    /// that make the fold arrival-order-invariant, held as laws in
    /// `tests/negotiation.rs` rather than trusted from this comment.
    pub fn merge(&mut self, other: &Position) {
        for (pole, amount) in &other.offered {
            self.offer(pole, amount.clone());
        }
    }

    /// What this position offers on a pole. Absent is zero — an offer,
    /// unlike a reading, has a lawful nothing.
    pub fn offered(&self, pole: &str) -> Exact {
        self.offered.get(pole).cloned().unwrap_or_else(Exact::zero)
    }

    /// Every pole this position touches.
    pub fn poles(&self) -> impl Iterator<Item = &Pole> {
        self.offered.keys()
    }
}

/// What the survey demands, per pole. Fixed by geometry — the ask does
/// not negotiate, the position does.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ask {
    asked: BTreeMap<Pole, Exact>,
}

impl Ask {
    /// Demand an amount on a pole.
    pub fn demand(&mut self, pole: &str, amount: Exact) {
        self.asked.insert(pole.to_owned(), amount);
    }

    /// What is asked on a pole.
    pub fn asked(&self, pole: &str) -> Exact {
        self.asked.get(pole).cloned().unwrap_or_else(Exact::zero)
    }

    /// Every pole the ask names.
    pub fn poles(&self) -> impl Iterator<Item = &Pole> {
        self.asked.keys()
    }
}

/// The fold of a position against an ask: per-pole surplus or deficit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    per_pole: BTreeMap<Pole, Exact>,
}

/// The board's answer to an incomparable position: what is short and
/// what is long. **Not a refusal** — the material of a trade, returned
/// to a standing docket entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counter {
    /// Poles where the position is below the ask, and by how much.
    pub short: Vec<(Pole, Exact)>,
    /// Poles where the position exceeds the ask — what the party has
    /// to trade with.
    pub long: Vec<(Pole, Exact)>,
}

/// Fold a position against an ask.
pub fn balance(position: &Position, ask: &Ask) -> Balance {
    let mut per_pole = BTreeMap::new();
    for pole in ask.poles().chain(position.poles()) {
        per_pole
            .entry(pole.clone())
            .or_insert_with(|| position.offered(pole) - ask.asked(pole));
    }
    Balance { per_pole }
}

impl Balance {
    /// Whether every pole is non-negative — the fixpoint.
    ///
    /// A gate may **witness** this; it does not conduct the
    /// negotiation. Until it holds, the proposal stands on the docket
    /// and the answer is the [`Counter`].
    pub fn clears(&self) -> bool {
        self.per_pole.values().all(|net| !net.is_negative())
    }

    /// The counter this balance implies, or nothing if it clears.
    pub fn counter(&self) -> Option<Counter> {
        if self.clears() {
            return None;
        }
        let mut short = Vec::new();
        let mut long = Vec::new();
        for (pole, net) in &self.per_pole {
            if net.is_negative() {
                short.push((pole.clone(), -net.clone()));
            } else if net > &Exact::zero() {
                long.push((pole.clone(), net.clone()));
            }
        }
        Some(Counter { short, long })
    }

    /// The net on one pole.
    pub fn net(&self, pole: &str) -> Exact {
        self.per_pole.get(pole).cloned().unwrap_or_else(Exact::zero)
    }
}

/// The partial order on positions relative to an ask.
///
/// `a` and `b` may each cover poles the other does not: a party short
/// on pole one and long on pole two is **incomparable** with the ask —
/// neither above nor below it. There is no total order here, for the
/// same reason revisions are never ordered: imposing one would let a
/// gate decide an instant two separated parties do not share.
pub fn comparable(a: &Balance, b: &Balance) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    let mut seen = Ordering::Equal;
    for pole in a.per_pole.keys().chain(b.per_pole.keys()) {
        let x = a.net(pole);
        let y = b.net(pole);
        let here = x.cmp(&y);
        match (seen, here) {
            (Ordering::Equal, other) => seen = other,
            (Ordering::Less, Ordering::Greater) | (Ordering::Greater, Ordering::Less) => {
                return None // incomparable — the trading case
            }
            _ => {}
        }
    }
    Some(seen)
}

/// Negative check for `Exact` without importing `Signed` everywhere.
trait IsNegative {
    fn is_negative(&self) -> bool;
}
impl IsNegative for Exact {
    fn is_negative(&self) -> bool {
        self < &Exact::zero()
    }
}