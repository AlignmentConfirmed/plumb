//! Sphere merge: bulk S*, residual carry, PoUW/PoWC admit, payout.
//!
//! Product law: `decide/sphere-merge.md` (SM1–SM11).
//!
//! ```text
//! M  = A ∧ B          componentwise min
//! R_A = A − M         residual of A
//! R_B = B − M
//! S* = M              converged sphere
//! Reward = Reward(S*) + Carry(R_A) + Carry(R_B)
//! ```

use assay::shape::ShapeClaim;
use assay::work::{WorkBody, WorkId};
use assay::{assess, Boundary, Convergence};
use isthmus::sphere::Precedence;

use crate::extent::Extent;

/// Default bulk-dominance policy: residual may not dominate on any axis
/// where both sides hold capacity — bulk ≥ 9/10 of max(A,B) when expressed
/// as bulk/(bulk+residual) ≥ 9/10, i.e. residual ≤ bulk/9.
///
/// Implemented as: for each axis with max(A,B) > 0,
/// `10 * M_i >= 9 * max(A_i, B_i)` (integer form of M/max ≥ 9/10).
pub const DEFAULT_BULK_NUM: u128 = 9;
pub const DEFAULT_BULK_DEN: u128 = 10;

/// Why a merge split or admit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeRefused {
    /// Extents (or claims) do not share arity / shape.
    Arity {
        /// Left axis count.
        left: usize,
        /// Right axis count.
        right: usize,
    },
    /// Bulk fails the dominance policy (too much residual).
    BulkTooThin {
        /// Axis that failed (0-based).
        axis: usize,
        /// Bulk on that axis.
        bulk: u128,
        /// max(A,B) on that axis.
        span: u128,
    },
    /// PoWC: a side does not close (or residual not fully named).
    PowcOpen,
    /// PoUW: shape does not admit / work body fails verify.
    PouwOpen,
    /// Ordered standoff: a party already saw the other — merge refused
    /// without explicit fault override (SM7).
    OrderedFault {
        /// Precedence classification.
        order: Precedence,
    },
    /// Empty bulk — nothing to converge.
    EmptyBulk,
}

/// Result of splitting two multi-axial spheres (SM1–SM2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSplit {
    /// Converged bulk S* = A ∧ B.
    pub bulk: Extent,
    /// Residual of A: A − M.
    pub residual_a: Extent,
    /// Residual of B: B − M.
    pub residual_b: Extent,
}

impl MergeSplit {
    /// Split two extents into bulk and residuals.
    pub fn of(a: &Extent, b: &Extent) -> Result<Self, MergeRefused> {
        let bulk = a.meet(b).ok_or(MergeRefused::Arity {
            left: a.axes(),
            right: b.axes(),
        })?;
        let residual_a = a.saturating_sub(&bulk).ok_or(MergeRefused::Arity {
            left: a.axes(),
            right: b.axes(),
        })?;
        let residual_b = b.saturating_sub(&bulk).ok_or(MergeRefused::Arity {
            left: a.axes(),
            right: b.axes(),
        })?;
        if bulk.is_nothing() || bulk.is_empty() {
            return Err(MergeRefused::EmptyBulk);
        }
        Ok(Self {
            bulk,
            residual_a,
            residual_b,
        })
    }

    /// Per-axis merge ratios in parts per `den` (default 10 → tenths).
    ///
    /// `ratio_i = floor(M_i * den / max(A_i,B_i,1))`. Never a product
    /// across axes. Returns `None` on arity issues (should not after `of`).
    pub fn ratios_per(
        &self,
        a: &Extent,
        b: &Extent,
        den: u128,
    ) -> Option<Vec<u128>> {
        if a.axes() != b.axes() || a.axes() != self.bulk.axes() || den == 0 {
            return None;
        }
        Some(
            self.bulk
                .components()
                .iter()
                .zip(a.components().iter())
                .zip(b.components().iter())
                .map(|((m, aa), bb)| {
                    let span = (*aa).max(*bb).max(1);
                    m.saturating_mul(den) / span
                })
                .collect(),
        )
    }

    /// Whether bulk dominates residual under θ = num/den (SM3).
    ///
    /// For each axis with span = max(A,B) > 0:
    /// `num * span <= den * M` fails → [`MergeRefused::BulkTooThin`].
    /// Zero-span axes are skipped.
    pub fn bulk_dominates(
        &self,
        a: &Extent,
        b: &Extent,
        num: u128,
        den: u128,
    ) -> Result<(), MergeRefused> {
        if den == 0 || a.axes() != b.axes() || a.axes() != self.bulk.axes() {
            return Err(MergeRefused::Arity {
                left: a.axes(),
                right: b.axes(),
            });
        }
        for (i, ((m, aa), bb)) in self
            .bulk
            .components()
            .iter()
            .zip(a.components().iter())
            .zip(b.components().iter())
            .enumerate()
        {
            let span = (*aa).max(*bb);
            if span == 0 {
                continue;
            }
            // M/span >= num/den  ⇔  M * den >= span * num
            if m.saturating_mul(den) < span.saturating_mul(num) {
                return Err(MergeRefused::BulkTooThin {
                    axis: i,
                    bulk: *m,
                    span,
                });
            }
        }
        Ok(())
    }

    /// Default θ = 9/10.
    pub fn bulk_dominates_default(&self, a: &Extent, b: &Extent) -> Result<(), MergeRefused> {
        self.bulk_dominates(a, b, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN)
    }
}

/// Split then enforce bulk policy.
pub fn split_with_policy(
    a: &Extent,
    b: &Extent,
    num: u128,
    den: u128,
) -> Result<MergeSplit, MergeRefused> {
    let split = MergeSplit::of(a, b)?;
    split.bulk_dominates(a, b, num, den)?;
    Ok(split)
}

// ─── SM4 PoWC admit ──────────────────────────────────────────────────

/// Admit merge bulk from two multi-axial boundaries (PoWC).
///
/// Both must [`Convergence::Closed`] on every axis. Residual capacity
/// for economics still comes from the **extent** pair via [`MergeSplit`].
pub fn admit_powc(a: &Boundary, b: &Boundary) -> Result<(), MergeRefused> {
    match (assess(a), assess(b)) {
        (Convergence::Closed(_), Convergence::Closed(_)) => Ok(()),
        _ => Err(MergeRefused::PowcOpen),
    }
}

// ─── SM5 PoUW admit ──────────────────────────────────────────────────

/// Admit merge from two shape claims (PoUW). Both must verify.
pub fn admit_pouw(a: &ShapeClaim, b: &ShapeClaim) -> Result<(WorkId, WorkId), MergeRefused> {
    a.verify().map_err(|_| MergeRefused::PouwOpen)?;
    b.verify().map_err(|_| MergeRefused::PouwOpen)?;
    Ok((a.work_id(), b.work_id()))
}

/// Admit from opaque highway/assay bodies (domain-tagged).
pub fn admit_pouw_bodies(a: &[u8], b: &[u8]) -> Result<(WorkId, WorkId), MergeRefused> {
    let wa = WorkBody::parse(a).map_err(|_| MergeRefused::PouwOpen)?;
    let wb = WorkBody::parse(b).map_err(|_| MergeRefused::PouwOpen)?;
    if !wa.verifies() || !wb.verifies() {
        return Err(MergeRefused::PouwOpen);
    }
    Ok((wa.work_id(), wb.work_id()))
}

// ─── SM6 Merge offer / admit record ──────────────────────────────────

/// How merge was admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitBy {
    /// Multi-axial flux closed on both sides.
    Powc,
    /// Shape structure verified on both sides.
    Pouw,
    /// Both mechanisms held.
    Both,
}

/// An admitted merge: bulk/residuals + provenance (in-memory settlement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeAdmit {
    /// Split of the two spheres.
    pub split: MergeSplit,
    /// Mechanism(s) that admitted.
    pub by: AdmitBy,
    /// PoUW work ids when applicable.
    pub work_a: Option<WorkId>,
    /// Peer work id.
    pub work_b: Option<WorkId>,
    /// Sphere precedence when standoff was considered.
    pub precedence: Precedence,
}

/// Build an admit from extents + optional PoWC/PoUW + standoff (SM6–SM7).
pub fn admit_merge(
    a: &Extent,
    b: &Extent,
    powc: Option<(&Boundary, &Boundary)>,
    pouw: Option<(&[u8], &[u8])>,
    precedence: Precedence,
    allow_ordered: bool,
) -> Result<MergeAdmit, MergeRefused> {
    if matches!(
        precedence,
        Precedence::HereSawThere | Precedence::ThereSawHere
    ) && !allow_ordered
    {
        return Err(MergeRefused::OrderedFault {
            order: precedence,
        });
    }

    let mut by_powc = false;
    let mut by_pouw = false;
    let mut work_a = None;
    let mut work_b = None;

    if let Some((ba, bb)) = powc {
        admit_powc(ba, bb)?;
        by_powc = true;
    }
    if let Some((pa, pb)) = pouw {
        let (wa, wb) = admit_pouw_bodies(pa, pb)?;
        work_a = Some(wa);
        work_b = Some(wb);
        by_pouw = true;
    }
    if !by_powc && !by_pouw {
        return Err(MergeRefused::PouwOpen);
    }

    let split = split_with_policy(a, b, DEFAULT_BULK_NUM, DEFAULT_BULK_DEN)?;
    let by = match (by_powc, by_pouw) {
        (true, true) => AdmitBy::Both,
        (true, false) => AdmitBy::Powc,
        (false, true) => AdmitBy::Pouw,
        (false, false) => unreachable!(),
    };
    Ok(MergeAdmit {
        split,
        by,
        work_a,
        work_b,
        precedence,
    })
}

// ─── SM8–SM10 Payout ─────────────────────────────────────────────────

/// Economic split of a merge (SM8–SM10).
///
/// - **principal** accrues on bulk S* (converged sphere economics)
/// - **carry_a / carry_b** are residual vectors — never folded to a scalar
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePayout {
    /// Reward vector on S* (copy of bulk for capacity accounting).
    pub principal: Extent,
    /// Carry to residual A.
    pub carry_a: Extent,
    /// Carry to residual B.
    pub carry_b: Extent,
}

impl MergePayout {
    /// Derive payout from an admitted split.
    ///
    /// Principal = bulk; carry = residuals. No product, no dust fold.
    pub fn from_split(split: &MergeSplit) -> Self {
        Self {
            principal: split.bulk.clone(),
            carry_a: split.residual_a.clone(),
            carry_b: split.residual_b.clone(),
        }
    }

    /// Total multi-axial obligation as three legs (caller routes them).
    pub fn legs(&self) -> (&Extent, &Extent, &Extent) {
        (&self.principal, &self.carry_a, &self.carry_b)
    }
}

/// Apply payout to three books: principal, carry_a, carry_b.
///
/// Stacks componentwise onto each book's running total **without**
/// work_id (economic allocation after admit). For work-gated credit use
/// [`crate::reward::RewardBook`] separately with the merge work bodies.
pub fn apply_payout(
    principal_book: &mut crate::reward::RewardBook,
    carry_a_book: &mut crate::reward::RewardBook,
    carry_b_book: &mut crate::reward::RewardBook,
    payout: &MergePayout,
) {
    principal_book.add_extent(&payout.principal);
    carry_a_book.add_extent(&payout.carry_a);
    carry_b_book.add_extent(&payout.carry_b);
}
