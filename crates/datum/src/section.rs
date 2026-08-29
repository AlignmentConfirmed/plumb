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
use std::collections::BTreeMap;

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

/// One grade of the section: a `(registered-domain tag, homological
/// dimension)` whose homology `H_dim` the credit accumulates in. Torsion is
/// a property of a *grade's* homology (it only appears in the SNF basis, not
/// on an individual cell), so the section is keyed here, per grade — the
/// type-correct home of the free⊕torsion structure `AxialCredit` carries.
pub type GradeId = (u64, u32);

/// The **multi-axial settlement section** (Phase 6, #62): the convergent
/// credit, keyed by grade, each cell an [`AxialCredit`] in that grade's
/// homology `H_k = ℤ^free ⊕ (⊕ ℤ/m_iℤ)`.
///
/// This is the book's convergence object (§6h): claims deposit into it in
/// any order and it **converges**, because [`AxialCredit::accumulate`] is
/// commutative and associative (confluent). Convergence over the torsion
/// axes is *guaranteed by finiteness* (they live in `ℤ/m_iℤ`); the free
/// axes grow (the density market).
///
/// The section is the **value** layer of the guard/section split. The
/// monotonic exactly-once **guard** — which claims have already been
/// deposited — is a *separate* layer that makes deposits idempotent (since
/// `⊕` is not), and lives with the book's `seen` set, not here. Together:
/// guard for exactly-once, group for any-order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    grades: BTreeMap<GradeId, AxialCredit>,
}

impl Section {
    /// An empty section.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Deposit `delta` into `grade` (typed by `shape`). Order-independent:
    /// the settled section does not depend on the order deposits arrive,
    /// which is the convergence guarantee that lets disjoint commits run
    /// concurrently. The caller has already passed the exactly-once guard.
    pub fn deposit(&mut self, grade: GradeId, shape: &GradeShape, delta: &AxialCredit) {
        self.grades
            .entry(grade)
            .or_insert_with(|| AxialCredit::zero(shape))
            .accumulate(delta, shape);
    }

    /// The accumulated credit at `grade`, if the section carries any.
    #[must_use]
    pub fn at(&self, grade: GradeId) -> Option<&AxialCredit> {
        self.grades.get(&grade)
    }

    /// The number of grades this section spans.
    #[must_use]
    pub fn spanned(&self) -> usize {
        self.grades.len()
    }

    /// The grades in canonical (sorted) order — the deterministic basis for
    /// an order-independent anchor commitment (#65).
    pub fn iter(&self) -> impl Iterator<Item = (&GradeId, &AxialCredit)> {
        self.grades.iter()
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

    #[test]
    fn the_section_converges_regardless_of_deposit_order() {
        // THE #62 guarantee at the section level: the same deposits into the
        // same grades, in ANY order, fold to the identical section — so
        // disjoint commits parallelize and every node reaches one limit.
        let g0: GradeId = (100, 0); // domain 100, dim 0 — H_0 with a ℤ/6
        let g1: GradeId = (100, 1); // domain 100, dim 1 — free
        let g2: GradeId = (200, 0); // a different domain, orthogonal grade
        let s0 = shape(1, &[6]);
        let s1 = shape(2, &[]);
        let s2 = shape(0, &[4]);
        let deposits = [
            (g0, &s0, AxialCredit::of(vec![3], vec![4])),
            (g1, &s1, AxialCredit::of(vec![7, -2], vec![])),
            (g0, &s0, AxialCredit::of(vec![1], vec![5])),
            (g2, &s2, AxialCredit::of(vec![], vec![3])),
            (g1, &s1, AxialCredit::of(vec![10, 8], vec![])),
        ];

        let mut forward = Section::new();
        for (grade, shp, delta) in &deposits {
            forward.deposit(*grade, shp, delta);
        }
        let mut reverse = Section::new();
        for (grade, shp, delta) in deposits.iter().rev() {
            reverse.deposit(*grade, shp, delta);
        }
        assert_eq!(forward, reverse, "commit order cannot change the section");

        // And the values are the abelian sums, torsion reduced per grade.
        assert_eq!(forward.spanned(), 3);
        assert_eq!(forward.at(g0).map(AxialCredit::torsion), Some(&[3u64][..]), "(4+5)%6=3");
        assert_eq!(forward.at(g1).map(AxialCredit::free), Some(&[17i128, 6][..]));
        assert_eq!(forward.at(g2).map(AxialCredit::torsion), Some(&[3u64][..]));
    }

    #[test]
    fn a_grades_torsion_converges_to_identity_over_its_order() {
        // Convergence is guaranteed by finiteness: m deposits of a ℤ/mℤ
        // cycle return the grade to the additive identity.
        let g: GradeId = (1, 0);
        let s = shape(0, &[5]);
        let mut section = Section::new();
        for _ in 0..5 {
            section.deposit(g, &s, &AxialCredit::of(vec![], vec![1]));
        }
        assert_eq!(section.at(g).map(AxialCredit::torsion), Some(&[0u64][..]), "5 ≡ 0 (mod 5)");
    }

    #[test]
    fn disjoint_grades_do_not_interfere() {
        let s = shape(1, &[]);
        let mut section = Section::new();
        section.deposit((1, 0), &s, &AxialCredit::of(vec![5], vec![]));
        section.deposit((2, 0), &s, &AxialCredit::of(vec![9], vec![]));
        assert_eq!(section.at((1, 0)).map(AxialCredit::free), Some(&[5i128][..]));
        assert_eq!(section.at((2, 0)).map(AxialCredit::free), Some(&[9i128][..]));
        assert_eq!(section.at((3, 0)), None, "an untouched grade carries nothing");
    }
}
