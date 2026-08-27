//! Per-axis space, and no way to flatten it.
//!
//! The board priced estates on `volume()` — the product of a region's
//! extents. `[2, 8]` and `[4, 4]` both give **16**, so two applications
//! wanting different shapes were priced identically, reported the same
//! accounting, and were indistinguishable to every downstream reader.
//!
//! That is folding a space that is not there. A product across axes is
//! a paper operation: it means something only once the object has been
//! flattened onto a line, and flattening is what breaks a polytope.
//!
//! [`Extent`] is what replaced it. One component per axis, in axis
//! order, and **there is no `product()` and no `total()`** — the same
//! discipline `assay::Extent` keeps for flux. A method nobody can call
//! is a method nobody calls by accident.
//!
//! ## Ordering is partial, and that is the point
//!
//! `[2, 8]` and `[4, 4]` are **not comparable**: neither fits inside
//! the other. A scalar comparison would have answered *equal* and a
//! caller would have believed it. [`Extent::fits_in`] answers what is
//! actually true — one extent fits inside another when it fits on
//! **every** axis — and [`Extent::compare`] returns `None` when neither
//! does.
//!
//! This is the same shape as the substrate's causal order (`Frontier`)
//! and the board's per-pole balance: where a fold would give a total
//! order over incomparable things, the component reading gives a
//! partial one and says so.

/// Space, per axis. Never a product.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extent {
    axes: Vec<u128>,
}

impl Extent {
    /// An extent from its per-axis components, in axis order.
    pub fn new(axes: Vec<u128>) -> Self {
        Self { axes }
    }

    /// The extent of a deed's region: one component per axis.
    pub fn of(region: &[(isthmus::layout::Tag, isthmus::layout::Tag)]) -> Self {
        Self {
            axes: region
                .iter()
                .map(|(low, high)| {
                    u128::from(*high)
                        .saturating_sub(u128::from(*low))
                        .saturating_add(1)
                })
                .collect(),
        }
    }

    /// The whole space a court spans, per axis.
    pub fn of_court(court: &isthmus::deed::Ledger) -> Self {
        Self {
            axes: court
                .axes()
                .iter()
                .map(|axis| u128::from(axis.max).saturating_add(1))
                .collect(),
        }
    }

    /// How many axes this reading spans.
    pub fn axes(&self) -> usize {
        self.axes.len()
    }

    /// The components, in axis order.
    pub fn components(&self) -> &[u128] {
        &self.axes
    }

    /// One axis's extent, if this reading spans it.
    ///
    /// `None` rather than `0` or `1`: an axis a reading never measured
    /// is a question it cannot answer, and either default would be
    /// indistinguishable from an answer.
    pub fn component(&self, axis: usize) -> Option<u128> {
        self.axes.get(axis).copied()
    }

    /// Whether this reading spans no axes.
    pub fn is_empty(&self) -> bool {
        self.axes.is_empty()
    }

    /// Whether every component is zero-width.
    pub fn is_nothing(&self) -> bool {
        !self.axes.is_empty() && self.axes.iter().all(|e| *e == 0)
    }

    /// Whether this fits inside `outer` — **on every axis**.
    ///
    /// The predicate a price should have been using. `[2, 8]` does not
    /// fit in `[4, 4]` and `[4, 4]` does not fit in `[2, 8]`, which a
    /// product cannot express because it calls them equal.
    pub fn fits_in(&self, outer: &Extent) -> bool {
        self.axes.len() == outer.axes.len()
            && self
                .axes
                .iter()
                .zip(outer.axes.iter())
                .all(|(mine, theirs)| mine <= theirs)
    }

    /// The partial order. **`None` means incomparable, not equal.**
    pub fn compare(&self, other: &Extent) -> Option<std::cmp::Ordering> {
        if self.axes.len() != other.axes.len() {
            return None;
        }
        let mut larger = false;
        let mut smaller = false;
        for (mine, theirs) in self.axes.iter().zip(other.axes.iter()) {
            match mine.cmp(theirs) {
                std::cmp::Ordering::Greater => larger = true,
                std::cmp::Ordering::Less => smaller = true,
                std::cmp::Ordering::Equal => {}
            }
        }
        match (larger, smaller) {
            (false, false) => Some(std::cmp::Ordering::Equal),
            (true, false) => Some(std::cmp::Ordering::Greater),
            (false, true) => Some(std::cmp::Ordering::Less),
            (true, true) => None,
        }
    }

    /// What `outer` has left after `self` is taken from it, per axis.
    ///
    /// Saturating per component. `None` if the two do not span the same
    /// axes — subtracting readings of different dimensionality would be
    /// asserting a measurement on an axis one of them never saw.
    pub fn taken_from(&self, outer: &Extent) -> Option<Extent> {
        (self.axes.len() == outer.axes.len()).then(|| Extent {
            axes: outer
                .axes
                .iter()
                .zip(self.axes.iter())
                .map(|(o, m)| o.saturating_sub(*m))
                .collect(),
        })
    }

    /// Componentwise minimum — mergeable bulk `A ∧ B` (sphere-merge SM1).
    ///
    /// `None` on arity mismatch. Never a product.
    pub fn meet(&self, other: &Extent) -> Option<Extent> {
        (self.axes.len() == other.axes.len()).then(|| Extent {
            axes: self
                .axes
                .iter()
                .zip(other.axes.iter())
                .map(|(a, b)| (*a).min(*b))
                .collect(),
        })
    }

    /// Componentwise saturating difference `self − other` (residual).
    ///
    /// Same as `other.taken_from(self)`: what remains of `self` after
    /// removing `other`. `None` on arity mismatch.
    pub fn saturating_sub(&self, other: &Extent) -> Option<Extent> {
        other.taken_from(self)
    }

    /// Componentwise sum (for stacking credit vectors). Arity must match.
    pub fn saturating_add(&self, other: &Extent) -> Option<Extent> {
        (self.axes.len() == other.axes.len()).then(|| Extent {
            axes: self
                .axes
                .iter()
                .zip(other.axes.iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect(),
        })
    }
}

impl std::fmt::Display for Extent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (at, component) in self.axes.iter().enumerate() {
            if at > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{component}")?;
        }
        write!(f, "]")
    }
}
