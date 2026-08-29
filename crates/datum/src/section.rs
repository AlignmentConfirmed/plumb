//! The value of the multi-axial settlement section at one cell (Phase 6,
//! #62): a credit in the grade's homology group
//!
//! ```text
//!   H_k(C) = ℤ^{b_k}  ⊕  (⊕_i ℤ/m_iℤ)
//! ```
//!
//! Free axes count in ℤ; torsion axes count in ℤ/m_iℤ, where the invariant
//! factors `m_i` come from the SNF we already extract
//! ([`crate::geometry::grade_shapes`], cached at `Act::Declare`). This is
//! the mathematics behind the section: the book is a section of the graded
//! homology bundle, and a cell's credit lives in the homology of its grade.
//!
//! **Accumulation is commutative and associative on every axis** — free
//! addition in ℤ and modular addition in ℤ/m_iℤ both are — so the settled
//! credit at a cell is **independent of the order** contributions arrive.
//! That order-independence is the convergence (A4) guarantee: disjoint
//! commits can run in parallel and every node still folds to the identical
//! section, with no global lock. Authority is the closure of each
//! contribution; convergence is this abelian, order-free accumulation.

use crate::geometry::GradeShape;

/// A credit valued in one grade's homology group `ℤ^free ⊕ (⊕ ℤ/m_iℤ)`.
/// Typed by the [`GradeShape`] of its grade, which supplies the free rank
/// and the torsion moduli `m_i`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxialCredit {
    /// Free axes — ℤ-valued, unbounded (saturating at the `i128` rails).
    free: Vec<i128>,
    /// Torsion axes — axis `i` is a residue in `[0, m_i)`.
    torsion: Vec<u64>,
}

impl AxialCredit {
    /// The zero credit for `shape`: 0 on every free and torsion axis — the
    /// additive identity of the grade's homology group.
    #[must_use]
    pub fn zero(shape: &GradeShape) -> Self {
        Self {
            free: vec![0; shape.free_rank],
            torsion: vec![0; shape.torsion.len()],
        }
    }

    /// A raw contribution: `free` amounts in ℤ and `torsion` amounts (not
    /// yet reduced — [`AxialCredit::accumulate`] reduces them mod `m_i`).
    #[must_use]
    pub fn of(free: Vec<i128>, torsion: Vec<u64>) -> Self {
        Self { free, torsion }
    }

    /// Accumulate `delta` into this credit under `shape`: free axes add in
    /// ℤ (saturating), torsion axis `i` adds in `ℤ/m_iℤ` (wraps modulo the
    /// invariant factor). Axes past either rank are ignored, so a
    /// malformed contribution can never panic — it simply cannot credit an
    /// axis the grade does not have.
    pub fn accumulate(&mut self, delta: &AxialCredit, shape: &GradeShape) {
        for (axis, &d) in delta.free.iter().enumerate() {
            if let Some(slot) = self.free.get_mut(axis) {
                *slot = slot.saturating_add(d);
            }
        }
        for (axis, &d) in delta.torsion.iter().enumerate() {
            let modulus = shape.torsion.get(axis).copied().unwrap_or(1).max(1);
            if let Some(slot) = self.torsion.get_mut(axis) {
                let sum = u128::from(*slot) + u128::from(d);
                // sum % modulus < modulus ≤ u64::MAX, so this never truncates.
                *slot = u64::try_from(sum % u128::from(modulus)).unwrap_or(0);
            }
        }
    }

    /// The free axes' ℤ values.
    #[must_use]
    pub fn free(&self) -> &[i128] {
        &self.free
    }

    /// The torsion axes' residues (axis `i` in `[0, m_i)`).
    #[must_use]
    pub fn torsion(&self) -> &[u64] {
        &self.torsion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(free_rank: usize, torsion: &[u64]) -> GradeShape {
        GradeShape {
            free_rank,
            torsion: torsion.to_vec(),
        }
    }

    #[test]
    fn a_torsion_axis_counts_in_z_mod_m() {
        // Grade with one ℤ/4ℤ axis: m credits return to zero.
        let s = shape(0, &[4]);
        let mut c = AxialCredit::zero(&s);
        c.accumulate(&AxialCredit::of(vec![], vec![3]), &s);
        assert_eq!(c.torsion(), &[3]);
        c.accumulate(&AxialCredit::of(vec![], vec![3]), &s);
        assert_eq!(c.torsion(), &[2], "3 + 3 = 6 ≡ 2 (mod 4)");
        c.accumulate(&AxialCredit::of(vec![], vec![2]), &s);
        assert_eq!(c.torsion(), &[0], "2 + 2 = 4 ≡ 0 (mod 4): back to identity");
    }

    #[test]
    fn a_free_axis_counts_in_z_unbounded() {
        let s = shape(1, &[]);
        let mut c = AxialCredit::zero(&s);
        for _ in 0..3 {
            c.accumulate(&AxialCredit::of(vec![1_000_000_000], vec![]), &s);
        }
        assert_eq!(c.free(), &[3_000_000_000], "free axis does not wrap");
    }

    #[test]
    fn accumulation_is_order_independent_the_convergence_guarantee() {
        // The heart of A4 under parallel commit: a ⊕ b == b ⊕ a on BOTH
        // free (ℤ) and torsion (ℤ/mℤ) axes, so the settled section does not
        // depend on which disjoint commit landed first.
        let s = shape(2, &[6, 5]);
        let a = AxialCredit::of(vec![7, -3], vec![4, 3]);
        let b = AxialCredit::of(vec![10, 8], vec![5, 4]);

        let mut ab = AxialCredit::zero(&s);
        ab.accumulate(&a, &s);
        ab.accumulate(&b, &s);

        let mut ba = AxialCredit::zero(&s);
        ba.accumulate(&b, &s);
        ba.accumulate(&a, &s);

        assert_eq!(ab, ba, "commit order cannot change the settled section");
        // And the values are correct: free adds in ℤ, torsion mod m.
        assert_eq!(ab.free(), &[17, 5]);
        assert_eq!(ab.torsion(), &[3, 2], "(4+5)%6=3, (3+4)%5=2");
    }

    #[test]
    fn accumulation_is_associative() {
        let s = shape(1, &[7]);
        let (a, b, c) = (
            AxialCredit::of(vec![2], vec![3]),
            AxialCredit::of(vec![5], vec![6]),
            AxialCredit::of(vec![1], vec![4]),
        );
        let mut left = AxialCredit::zero(&s); // (a ⊕ b) ⊕ c
        left.accumulate(&a, &s);
        left.accumulate(&b, &s);
        left.accumulate(&c, &s);
        let mut right = AxialCredit::zero(&s); // a ⊕ (b ⊕ c)
        let mut bc = AxialCredit::zero(&s);
        bc.accumulate(&b, &s);
        bc.accumulate(&c, &s);
        right.accumulate(&a, &s);
        // bc is already reduced; feeding it back accumulates identically.
        right.accumulate(&AxialCredit::of(bc.free().to_vec(), bc.torsion().to_vec()), &s);
        assert_eq!(left, right);
    }

    #[test]
    fn free_and_torsion_axes_are_orthogonal() {
        // A purely free contribution never perturbs a torsion axis, and
        // vice versa — the direct-sum decomposition, no cross-contamination.
        let s = shape(1, &[4]);
        let mut c = AxialCredit::zero(&s);
        c.accumulate(&AxialCredit::of(vec![9], vec![0]), &s);
        assert_eq!(c.free(), &[9]);
        assert_eq!(c.torsion(), &[0], "free credit left the torsion axis at 0");
        c.accumulate(&AxialCredit::of(vec![0], vec![3]), &s);
        assert_eq!(c.free(), &[9], "torsion credit left the free axis unchanged");
        assert_eq!(c.torsion(), &[3]);
    }
}
