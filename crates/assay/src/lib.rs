//! # Assay — THE CONVERGENCE ENGINE
//!
//! Given a manifold's oriented boundary, does the flux through it
//! cancel? If it cancels **on every axis**, the manifold closes, and
//! this crate mints [`Upsilon`] — the only proof of closure there is.
//!
//! ```text
//! isthmus  ->  nothing          the superhighway (never imports assay)
//! kernels  ->  ASSAY            the exits, which query this
//! assay    ->  nothing          multi-axial physical laws (leaf)
//! nodes    ->  assay + isthmus  produce / verify; wire claims re-derive
//! datum    ->  isthmus + assay + edges  court + rewards + measurements
//! ```
//!
//! [`work::Claim`] is the portable multi-axial body. [`Upsilon`] never
//! crosses a wire — verifiers re-derive closure from the claim.
//!
//! ## What this crate holds, and what it refuses to hold
//!
//! It holds the physical laws. It does not hold the network or the
//! game state, and the dependency graph is what enforces that rather
//! than a convention: **assay imports nothing from `isthmus` or any
//! kernel.** It cannot read a framing tag, cannot execute a
//! kernel state transition, and cannot be told what to conclude by
//! anything that can.
//!
//! The direction matters. A kernel asks assay whether its manifold
//! closes; assay does not know kernels exist.
//!
//! ## The five rules this crate is built to
//!
//! 1. **Leaf node.** No mesh, no kernel. `tests/isolation.rs` reads
//!    `Cargo.toml` and fails on any of them, or on any `path = `.
//! 2. **Exact rational arithmetic only.** `Ratio<BigInt>`, unbounded
//!    on both numerator and denominator. **No floating point exists in
//!    this crate**, and the isolation test reads the source for `f32`
//!    and `f64` rather than trusting a lint that can be allowed away.
//!    A flux lost to rounding is a closure nobody can check.
//! 3. **Unfolded.** Coordinates are never collapsed or projected into
//!    a flat matrix. An [`Extent`] keeps one component per axis, and
//!    nothing in this crate sums across axes — see [`flux`] for the
//!    measured reason why that is not a stylistic preference.
//! 4. **Structural states, not boolean gates.** Nothing here returns
//!    `bool` for a threshold. [`Convergence`] is an enum whose arms
//!    carry the residue, because a verdict that cannot say *how far
//!    off* is a verdict a caller has to guess behind.
//! 5. **The witness.** [`Upsilon`] is zero-sized, carries no token, no
//!    identifier and no payload, and **cannot be constructed outside
//!    this crate**.
//!
//! ## Why [`Upsilon`] is unforgeable
//!
//! ```text
//! pub struct Upsilon(());
//!               ^^^^^^^^ the field is private
//! ```
//!
//! A tuple struct with a private field cannot be built by anyone who
//! is not inside the module that declares it. So an outside crate
//! **cannot write `Upsilon(())`** — the compiler refuses it — and the
//! only way to hold one is to have been handed it by [`assess`].
//!
//! It is worth being exact about what that proves. The value is minted
//! at run time, from a real measurement; what the compiler enforces is
//! the **monopoly on minting**. Possessing an `Upsilon` is therefore
//! proof that *this crate* concluded closure, not proof that closure
//! was concluded at compile time. Anything stronger would be a claim
//! wider than the mechanism.
//!
//! Being zero-sized is what makes it useless to forge by other means:
//! there is no field to tamper with, no token to replay, and nothing
//! to serialise. It cannot cross a wire, which is deliberate — a proof
//! that could travel would be a proof a peer could be handed rather
//! than reach.

#![deny(missing_docs)]

pub mod complex;
pub mod credit_event;
pub mod exact_codec;
pub mod extent;
pub mod freshness;
pub mod flux;
pub mod homology;
pub mod rewrite;
pub mod shape;
pub mod simplex;
pub mod snf;
pub mod work;

pub use credit_event::{ClaimClasses, CreditEvent};
pub use extent::Extent;
pub use freshness::{cover_suffices, equal_share, OnceCredit, Replay};
pub use flux::{Boundary, Facet, Orientation};
pub use shape::{Shape, ShapeClaim};
pub use work::{Claim, WorkBody, WorkId};

use num_bigint::BigInt;
use num_rational::Ratio;

/// An exact rational. Unbounded numerator, unbounded denominator.
///
/// The only number in this crate. There is no floating-point type
/// anywhere in `assay`, and `tests/isolation.rs` reads the source to
/// keep it that way.
pub type Exact = Ratio<BigInt>;

/// Exactly zero.
pub fn zero() -> Exact {
    Ratio::from_integer(BigInt::from(0))
}

/// A whole number as an exact rational.
pub fn whole(n: i64) -> Exact {
    Ratio::from_integer(BigInt::from(n))
}

/// An exact rational from a numerator and denominator.
///
/// `None` for a zero denominator, which is not a number. Refusing is
/// the total path: a convergence engine that panics on a hostile
/// manifold is one a hostile manifold can stop.
pub fn exact(numer: i64, denom: i64) -> Option<Exact> {
    (denom != 0).then(|| Ratio::new(BigInt::from(numer), BigInt::from(denom)))
}

/// **The proof of closure.** Zero-sized, unforgeable, and mintable only
/// by [`assess`].
///
/// See the crate documentation for why the private field is the whole
/// mechanism and for what it does and does not prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Upsilon(());

/// What the boundary says. **Never a `bool`.**
///
/// Rule 4: a threshold that answers yes or no forces a caller to guess
/// what it was near. Every arm here carries the residue that produced
/// it, so a caller that wants to know how far from closure it is can
/// read it rather than re-derive it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Convergence {
    /// **Every axis cancels exactly.** The manifold closes, and the
    /// witness is minted.
    Closed(Upsilon),

    /// **At least one axis does not cancel.** The manifold is open, and
    /// the residue says on which axes and by how much.
    ///
    /// ## This was two arms, split by a fold, and that was the defect
    ///
    /// It read `Cancelling` when the axes summed to zero and
    /// `Divergent` otherwise — and computing that sum adds flux on one
    /// axis to flux on another. **Those are not commensurable
    /// quantities.** The operation only means anything if the manifold
    /// has already been flattened onto a line, which is a paper
    /// operation: you fold a sheet, not a sphere, and folding is what
    /// breaks every polytope and every higher-dimensional manifold
    /// here.
    ///
    /// So the crate used a fold to *name* a state whose whole
    /// significance is that folds are wrong. The residue already
    /// carries everything either arm carried; splitting it by a
    /// meaningless total added a distinction with no topological
    /// content and endorsed the operation the crate exists to refuse.
    ///
    /// One open arm. A caller that wants to know how the manifold is
    /// open reads the residue per axis, which is where the answer was
    /// all along.
    Open {
        /// The per-axis residue: what did not cancel, and where.
        residue: Extent,
    },

    /// **An axis was described on one side only.**
    ///
    /// A boundary needs both orientations on every axis to be a
    /// boundary. One face is not a closed surface, and the flux
    /// through it cancels trivially against nothing.
    Incomplete {
        /// The axis missing a face.
        axis: usize,
        /// The orientation that is absent.
        missing: Orientation,
    },

    /// **Nothing was measured.**
    ///
    /// An empty boundary has zero divergence on every axis, and
    /// minting a proof of closure from it would be a gate that cannot
    /// fail. Closure is a property of a surface; no surface is not a
    /// closed one.
    Unmeasured,
}

impl Convergence {
    /// The witness, if the manifold closed.
    ///
    /// A method rather than a `bool` accessor: a caller either holds
    /// the proof or does not, and there is no third thing to ask.
    pub fn witness(&self) -> Option<Upsilon> {
        match self {
            Convergence::Closed(upsilon) => Some(*upsilon),
            _ => None,
        }
    }

    /// The per-axis residue, when there is one.
    ///
    /// `None` for [`Convergence::Closed`] — a closed manifold's residue
    /// is zero on every axis, and returning a vector of zeros would
    /// invite a caller to compare it against something.
    pub fn residue(&self) -> Option<&Extent> {
        match self {
            Convergence::Open { residue } => Some(residue),
            _ => None,
        }
    }
}

/// **The gauge-invariant reading.** Assess a boundary and, if it closes
/// on every axis, mint [`Upsilon`].
///
/// The order of the arms is the order of the refusals, and each one is
/// checked before the next can be reached:
///
/// 1. no facets at all — [`Convergence::Unmeasured`]
/// 2. an axis with a face missing — [`Convergence::Incomplete`]
/// 3. some axis's flux is non-zero — [`Convergence::Open`], carrying
///    the per-axis residue
/// 4. every axis is exactly zero — [`Convergence::Closed`]
///
/// **Gauge invariance.** The reading depends only on the *differences*
/// the facets carry, so adding one constant to every facet on an axis
/// — a re-gauge — moves the high face and the low face by the same
/// amount and cancels in the signed sum. Only disagreement is
/// observable, which is the same property the substrate's cocycle
/// verification rests on, in the same shape.
pub fn assess(boundary: &Boundary) -> Convergence {
    if boundary.is_empty() {
        return Convergence::Unmeasured;
    }
    if let Some((axis, missing)) = boundary.incomplete_axis() {
        return Convergence::Incomplete { axis, missing };
    }

    let residue = boundary.divergence();
    if residue.is_zero() {
        return Convergence::Closed(Upsilon(()));
    }
    // NO TOTAL IS TAKEN. There was one here, to tell two open arms
    // apart, and it added flux across axes that are not commensurable
    // — the fold this crate exists to refuse, used to label its own
    // refusals. The residue says everything the label said.
    Convergence::Open { residue }
}
