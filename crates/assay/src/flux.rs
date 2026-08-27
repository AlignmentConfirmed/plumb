//! The oriented boundary, and the flux through it.
//!
//! A manifold's boundary is `2n` faces — two per axis, one at the low
//! end and one at the high end. Each carries an exact rational flux.
//! The divergence on an axis is the signed sum of its two faces, with
//! the high face positive and the low face negative, and the manifold
//! closes when **every axis's divergence is exactly zero.**
//!
//! ## Per axis, and the measured reason
//!
//! The substrate this engine serves learned the same lesson one layer
//! up, twice. A loop law checks the total around a cycle, and a
//! distortion that cancels — a mirror, any involution — leaves the
//! total clean while every crossing is wrong. *Zero holonomy does not
//! mean no distortion; it means the distortions cancel.*
//!
//! Summing the axes here would be exactly that mistake in this crate:
//! a `+1` on one axis against a `−1` on another totals to zero while
//! the manifold is open in two directions at once. So the divergence
//! is a per-axis [`crate::Extent`] and the closure test is per
//! component.
//!
//! **And no total is taken anywhere, including to describe that
//! state.** The crate briefly split its open verdict by whether the
//! axes summed to zero; the residue `[+1, −1]` already says it, and
//! computing the sum to label it was the flattening the crate exists
//! to refuse. You fold a sheet of paper, not a sphere.

use crate::{zero, Exact, Extent};

/// Which face of an axis a facet is.
///
/// The sign in the signed sum, and nothing else: `High` enters
/// positive, `Low` enters negative. Orientation is what makes the sum
/// a *divergence* rather than a total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Orientation {
    /// The face at the low end of the axis. Enters negative.
    Low,
    /// The face at the high end. Enters positive.
    High,
}

impl Orientation {
    /// The other face of the same axis.
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Orientation::Low => Orientation::High,
            Orientation::High => Orientation::Low,
        }
    }
}

/// One oriented face of a manifold, carrying its flux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    /// Which axis this face is normal to.
    pub axis: usize,
    /// Which end of that axis.
    pub orientation: Orientation,
    /// The flux through this face, exact.
    pub flux: Exact,
}

impl Facet {
    /// A facet.
    pub fn new(axis: usize, orientation: Orientation, flux: Exact) -> Self {
        Self {
            axis,
            orientation,
            flux,
        }
    }

    /// The flux as it enters the signed sum: `+flux` at the high face,
    /// `−flux` at the low.
    pub fn signed(&self) -> Exact {
        match self.orientation {
            Orientation::High => self.flux.clone(),
            Orientation::Low => -self.flux.clone(),
        }
    }
}

/// A manifold's oriented boundary.
///
/// Held **unfolded**: the facets keep their axis, and the divergence is
/// computed per axis. Nothing here projects the faces into a flat list
/// of numbers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Boundary {
    axes: usize,
    facets: Vec<Facet>,
}

impl Boundary {
    /// An empty boundary over `axes` axes.
    ///
    /// Empty is a real state and it is **not** closure — see
    /// [`crate::Convergence::Unmeasured`].
    pub fn new(axes: usize) -> Self {
        Self {
            axes,
            facets: Vec::new(),
        }
    }

    /// Add a face. A facet naming an axis this boundary does not span
    /// is **refused**, not silently widened: a boundary that grew an
    /// axis because somebody described a face on it would be a boundary
    /// whose dimensionality depends on the order things arrived.
    pub fn face(&mut self, facet: Facet) -> bool {
        if facet.axis >= self.axes {
            return false;
        }
        self.facets.push(facet);
        true
    }

    /// How many axes this boundary spans.
    pub fn axes(&self) -> usize {
        self.axes
    }

    /// The faces, in the order they were described.
    pub fn facets(&self) -> &[Facet] {
        &self.facets
    }

    /// Whether no face has been described.
    pub fn is_empty(&self) -> bool {
        self.facets.is_empty()
    }

    /// The first axis missing a face, with the orientation that is
    /// absent.
    ///
    /// A boundary needs **both** ends of every axis to be a closed
    /// surface. One face is not a surface, and its flux would cancel
    /// against nothing — so this is checked before any sum is taken,
    /// or a half-described box would mint a witness.
    pub fn incomplete_axis(&self) -> Option<(usize, Orientation)> {
        for axis in 0..self.axes {
            for orientation in [Orientation::Low, Orientation::High] {
                let present = self
                    .facets
                    .iter()
                    .any(|f| f.axis == axis && f.orientation == orientation);
                if !present {
                    return Some((axis, orientation));
                }
            }
        }
        None
    }

    /// The per-axis divergence: **an [`Extent`], never a scalar.**
    ///
    /// For each axis, the sum of its high faces minus the sum of its
    /// low faces. Axes are kept apart, which is the whole of rule 3 in
    /// one return type.
    pub fn divergence(&self) -> Extent {
        let mut per_axis: Vec<Exact> = (0..self.axes).map(|_| zero()).collect();
        for facet in &self.facets {
            if let Some(slot) = per_axis.get_mut(facet.axis) {
                *slot = slot.clone() + facet.signed();
            }
        }
        Extent::new(per_axis)
    }

    /// How many faces this axis carries at each end, as `(low, high)`.
    ///
    /// A face may be subdivided, so an axis can legitimately hold more
    /// than one facet per orientation. The counts matter because they
    /// decide whether a re-gauge on that axis is observable — see
    /// [`Boundary::regauged`].
    pub fn faces_on(&self, axis: usize) -> (usize, usize) {
        let low = self
            .facets
            .iter()
            .filter(|f| f.axis == axis && f.orientation == Orientation::Low)
            .count();
        let high = self
            .facets
            .iter()
            .filter(|f| f.axis == axis && f.orientation == Orientation::High)
            .count();
        (low, high)
    }

    /// Whether every axis carries as many low faces as high ones.
    ///
    /// **The condition under which a re-gauge is unobservable.** See
    /// [`Boundary::regauged`] for why it is a condition and not a
    /// guarantee.
    pub fn is_balanced(&self) -> bool {
        (0..self.axes).all(|axis| {
            let (low, high) = self.faces_on(axis);
            low == high
        })
    }

    /// Shift every facet on `axis` by a constant — **a re-gauge.**
    ///
    /// ## The invariance is conditional, and the condition is the count
    ///
    /// A re-gauge moves every facet on the axis by the same amount, and
    /// the low faces enter the signed sum negatively. So the divergence
    /// on that axis changes by
    ///
    /// ```text
    /// by × (high faces − low faces)
    /// ```
    ///
    /// which is zero **exactly when the axis is balanced** — as a box
    /// is, with one face at each end. It is not zero on an axis whose
    /// faces have been subdivided unevenly, and there the gauge is
    /// observable.
    ///
    /// This was measured rather than assumed: the first version of the
    /// law claimed invariance unconditionally and failed against a
    /// boundary with two high faces and one low. The claim was wider
    /// than the mechanism, so the mechanism is stated instead — an
    /// engine that promised invariance it does not have would let a
    /// caller re-gauge its way from divergent to closed.
    ///
    /// [`Boundary::is_balanced`] is the predicate; a caller that needs
    /// the invariance can ask for it.
    #[must_use]
    pub fn regauged(&self, axis: usize, by: &Exact) -> Boundary {
        Boundary {
            axes: self.axes,
            facets: self
                .facets
                .iter()
                .map(|facet| {
                    if facet.axis == axis {
                        Facet {
                            axis: facet.axis,
                            orientation: facet.orientation,
                            flux: facet.flux.clone() + by,
                        }
                    } else {
                        facet.clone()
                    }
                })
                .collect(),
        }
    }
}
