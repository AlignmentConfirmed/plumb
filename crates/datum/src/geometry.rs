//! The bridge from **verification** geometry (`assay`) to **scheduling**
//! geometry (`sched`): the transport metric a court orders claims by is
//! *read* from the same exact complex the court verifies, never invented.
//!
//! Two facts a scheduler needs from a claim's boundary:
//! - its **graded torsion** `T_d` (how knotted each homological dimension
//!   is) — computed here, needs no shared basis; and
//! - its **support** (which generators it touches) — deferred, because a
//!   `DeclaredComplex`'s cell indices are *local* (`0..cells[k]`), so two
//!   complexes' "cell 3" are different generators. Curvature-intersection
//!   across claims is only meaningful once generators carry a **global**
//!   identity (the registered-domain / genesis basis). Until that lands,
//!   emitting local cell ids as a shared axis would falsely collide, so
//!   this module computes only what is basis-free and exact.
//!
//! Read-only over the verification geometry: it computes nothing the court
//! does not already compute (SNF invariant factors, via [`betti`]), and
//! names no settlement type — it maps a `DeclaredComplex` to plain
//! integers the ephemeral scheduler consumes.

use assay::complex::DeclaredComplex;
use assay::homology::betti;

/// The graded torsion of a complex: `graded_torsion(C)[d]` is the number of
/// nontrivial invariant factors of `H_d(C)` — the **torsion rank** at grade
/// `d`. A quantized, float-free measure of how knotted that dimension is,
/// aligned by index to feed `sched::Claim.torsion`, where `T_d` lifts the
/// quantum of dimension-`d` axes (#58). A torsion-free complex yields all
/// zeros (a flat ×1 lift on every grade).
///
/// The **size** of the torsion — the order `m` of each `ℤ/mℤ` summand — is
/// deliberately *not* used: one `ℤ/2` and one `ℤ/10⁶` are each a single
/// torsion cycle, and rewarding raw magnitude would smuggle "size" back
/// into a metric we ruled must price only structure (flat-unit-cost). So we
/// count cycles, which is bounded by the cell count and stays quantized.
///
/// A dimension whose homology cannot be computed (a non-integer coefficient
/// slipped past admission, say) contributes `0` — a claim is never granted
/// a torsion lift it cannot substantiate. `betti` is exact, so on an
/// admitted complex this branch does not arise.
#[must_use]
pub fn graded_torsion(complex: &DeclaredComplex) -> Vec<u64> {
    (0..complex.cells.len())
        .map(|d| {
            let dim = u32::try_from(d).unwrap_or(u32::MAX);
            betti(complex, dim)
                .map(|b| u64::try_from(b.torsion.len()).unwrap_or(u64::MAX))
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use assay::complex::{DeclaredComplex, Entry};
    use assay::whole;

    #[test]
    fn a_torsion_free_complex_has_a_flat_graded_torsion() {
        // ∂_1 = [[1,0],[0,1],[0,0]]: invariant factors (1,1) — all trivial,
        // no torsion in any grade.
        let op = vec![
            Entry { row: 0, col: 0, coeff: whole(1) },
            Entry { row: 1, col: 1, coeff: whole(1) },
        ];
        let complex = DeclaredComplex {
            cells: vec![3, 2],
            ops: vec![op],
        };
        assert_eq!(graded_torsion(&complex), vec![0, 0], "flat: every grade ×1");
    }

    #[test]
    fn torsion_at_a_grade_counts_its_independent_cycles_not_their_size() {
        // ∂_1 with Smith invariant factors (2, 4): H_0 has torsion
        // ℤ/2 ⊕ ℤ/4 — TWO independent torsion cycles at grade 0.
        let mut op = vec![
            Entry { row: 0, col: 0, coeff: whole(2) },
            Entry { row: 1, col: 0, coeff: whole(6) },
            Entry { row: 0, col: 1, coeff: whole(4) },
            Entry { row: 1, col: 1, coeff: whole(8) },
        ];
        op.sort_by_key(|e| (e.col, e.row));
        let complex = DeclaredComplex {
            cells: vec![2, 2],
            ops: vec![op],
        };
        let graded = graded_torsion(&complex);
        // Grade 0 carries two cycles (2 and 4); the count is 2, NOT 2+4 or
        // 2·4 — magnitude is not rewarded. Grade 1 is torsion-free.
        assert_eq!(graded, vec![2, 0], "H_0 has 2 torsion cycles, H_1 has none");
    }

    #[test]
    fn a_single_torsion_cycle_of_large_order_still_counts_one() {
        // One invariant factor of large order → exactly ONE cycle, so the
        // lift is 1, the same a ℤ/2 would earn. Magnitude never enters.
        let op = vec![
            Entry { row: 0, col: 0, coeff: whole(1_000_000) },
        ];
        let complex = DeclaredComplex {
            cells: vec![1, 1],
            ops: vec![op],
        };
        let graded = graded_torsion(&complex);
        assert_eq!(
            graded.first().copied(),
            Some(1),
            "ℤ/10⁶ is one cycle, weighted the same as ℤ/2"
        );
    }
}
