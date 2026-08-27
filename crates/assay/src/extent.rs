//! Unfolded coordinates: one component per axis, never collapsed.
//!
//! Rule 3. An [`Extent`] is a list of exact rationals indexed by axis,
//! and **nothing in this crate sums across it. At all.** There is no
//! method to do it with.
//!
//! ## Why folding is a real defect and not a style
//!
//! A multi-component reading multiplied out into one number stops being
//! able to say which component carried what. That is not a hypothetical:
//! the kernel's own `omega_per_component` is the folded read of
//! `omega_sheared`, and its documentation says so —
//!
//! > multiplying out is where the reading stops being able to say which
//! > pole carried what
//!
//! An engine that folded before comparing to zero would accept a `+1`
//! on one axis against a `−1` on another. Here it is
//! [`crate::Convergence::Open`], with `[+1, −1]` as the residue —
//! **the residue says it, no label needs computing.**
//!
//! The crate carried a `sum()` for exactly one purpose: labelling that
//! state. Struck. You fold a sheet of paper, not a sphere, and a
//! quantity summed across axes that are not commensurable is the
//! flattening that breaks a polytope.

use crate::{zero, Exact};
use num_traits::Zero;

/// A per-axis reading. **Unfolded** — one component per axis, in axis
/// order, and no operation here collapses them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extent {
    components: Vec<Exact>,
}

impl Extent {
    /// An extent from its components, in axis order.
    pub fn new(components: Vec<Exact>) -> Self {
        Self { components }
    }

    /// `axes` components, every one exactly zero.
    pub fn zeroed(axes: usize) -> Self {
        Self {
            components: (0..axes).map(|_| zero()).collect(),
        }
    }

    /// How many axes this reading spans.
    pub fn axes(&self) -> usize {
        self.components.len()
    }

    /// The components, in axis order.
    pub fn components(&self) -> &[Exact] {
        &self.components
    }

    /// One component, if the axis exists.
    ///
    /// `Option`, not a panic and not a zero: an axis this reading does
    /// not span is a question about a direction it never measured, and
    /// answering `0` would be indistinguishable from having measured
    /// exactly nothing there.
    pub fn component(&self, axis: usize) -> Option<&Exact> {
        self.components.get(axis)
    }

    /// Whether **every** component is exactly zero.
    ///
    /// The closure test. Per axis, and there is no total to take it on
    /// — see the module note.
    ///
    /// An extent with no components is **not** zero here; it is empty,
    /// and emptiness is [`crate::Convergence::Unmeasured`]'s business.
    /// A predicate that answered `true` for nothing measured would be a
    /// gate that cannot fail.
    pub fn is_zero(&self) -> bool {
        !self.components.is_empty() && self.components.iter().all(num_traits::Zero::is_zero)
    }

    // `sum()` was here.
    //
    // It added component to component across axes, and its only caller
    // split `Convergence`'s open state by whether the total was zero.
    // Flux on one axis and flux on another are not commensurable: the
    // sum means something only if the manifold has already been
    // flattened onto a line, and **you fold a sheet of paper, not a
    // sphere.** Folding is what breaks a polytope.
    //
    // So the crate offered the exact operation it exists to refuse,
    // and used it to label its own refusals. Struck rather than
    // documented-as-dangerous: a method nobody can call is a method
    // nobody calls by accident.

    /// Add another reading, component by component.
    ///
    /// Extents of different lengths do not add: **`None`**, rather than
    /// padding the shorter with zeros. Padding would silently assert
    /// that the shorter reading measured zero on axes it never spanned,
    /// which is the difference between "no flux there" and "no
    /// measurement there".
    pub fn add(&self, other: &Extent) -> Option<Extent> {
        (self.axes() == other.axes()).then(|| Extent {
            components: self
                .components
                .iter()
                .zip(other.components.iter())
                .map(|(here, there)| here + there)
                .collect(),
        })
    }

    /// Negate every component.
    #[must_use]
    pub fn negated(&self) -> Extent {
        Extent {
            components: self.components.iter().map(|c| -c.clone()).collect(),
        }
    }

    /// Whether this reading spans no axes at all.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

/// Whether an exact rational is exactly zero.
///
/// A free function so callers never reach for a tolerance. There is no
/// epsilon in this crate: the arithmetic is exact, so "close to zero"
/// is not a state that exists.
pub fn is_exactly_zero(value: &Exact) -> bool {
    value.is_zero()
}
