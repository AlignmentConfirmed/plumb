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

use crate::sched::Axis;
use assay::complex::DeclaredComplex;
use assay::homology::betti;

/// A generator's **global** identity: a deterministic 64-bit hash of
/// `(registered-domain tag, homological dimension, cell index)`.
///
/// A `DeclaredComplex`'s cell indices are local (`0..cells[k]`), so cell 3
/// of one universe is not cell 3 of another. Keying by the chain-registered
/// domain `tag` (an `Act::Declare` fact every node resolves identically)
/// gives cells in the *same* universe a shared axis and cells in *different*
/// universes disjoint ones — so curvature is real interference within a
/// universe and exact orthogonality across universes (the frontier
/// incentive: a freshly-declared domain is uncontested ground).
///
/// The hash is BLAKE3 (via [`sig::envelope_hash`]) over the little-endian
/// bytes, so it is **deterministic on every node** — convergence needs
/// that. A 64-bit space is not injective (a `Tag` alone is 64 bits), but
/// curvature is turn-order-only, so a collision is a harmless scheduling
/// hint, never a settlement fault.
#[must_use]
pub fn global_gen(tag: u64, dim: u32, cell: u32) -> u64 {
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&tag.to_le_bytes());
    bytes[8..12].copy_from_slice(&dim.to_le_bytes());
    bytes[12..16].copy_from_slice(&cell.to_le_bytes());
    let digest = sig::envelope_hash(&bytes);
    let head = digest.get(0..8).and_then(|s| s.try_into().ok());
    u64::from_le_bytes(head.unwrap_or([0u8; 8]))
}

/// The **support** of a claim — the generator axes it actively touches, in
/// global identity — read from its witness and target cells (not the whole
/// universe: two claims in one domain interfere only where their chains
/// overlap). `witness_cells` are the `witness_dim`-cells the derivation
/// uses; `target_cells` are the `(witness_dim − 1)`-cells it closes onto
/// (empty for a cycle-shape claim with no prescribed target). The result is
/// deduplicated and sorted, so it is a canonical set of [`Axis`] ready for
/// `sched::Claim.support`.
#[must_use]
pub fn support_axes(
    tag: u64,
    witness_dim: u32,
    witness_cells: &[u32],
    target_cells: &[u32],
) -> Vec<Axis> {
    let target_dim = witness_dim.saturating_sub(1);
    let mut axes: Vec<Axis> = witness_cells
        .iter()
        .map(|&cell| Axis::new(global_gen(tag, witness_dim, cell), witness_dim))
        .chain(
            target_cells
                .iter()
                .map(|&cell| Axis::new(global_gen(tag, target_dim, cell), target_dim)),
        )
        .collect();
    axes.sort_unstable();
    axes.dedup();
    axes
}

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

    #[test]
    fn a_generators_global_id_is_deterministic_and_separates_coordinates() {
        // Convergence needs determinism: same inputs → same id, every node.
        assert_eq!(global_gen(7, 1, 3), global_gen(7, 1, 3));
        // A different tag, dimension, OR cell is a different generator.
        assert_ne!(global_gen(7, 1, 3), global_gen(8, 1, 3), "tag separates");
        assert_ne!(global_gen(7, 1, 3), global_gen(7, 2, 3), "dim separates");
        assert_ne!(global_gen(7, 1, 3), global_gen(7, 1, 4), "cell separates");
    }

    #[test]
    fn claims_in_different_universes_are_orthogonal() {
        // The SAME local cell (3) under different domain tags maps to
        // different global generators → curvature between them is zero. A
        // freshly-declared domain is uncontested ground (frontier incentive).
        let a = support_axes(100, 1, &[3], &[]);
        let b = support_axes(200, 1, &[3], &[]);
        assert_ne!(
            a.first().map(|x| x.gen),
            b.first().map(|x| x.gen),
            "different universes share no generator"
        );
    }

    #[test]
    fn claims_in_the_same_universe_share_a_touched_generator() {
        // Two claims under the SAME tag that both touch cell 3 at dim 1 land
        // on the SAME axis → genuine interference on exactly that cell.
        let a = support_axes(100, 1, &[3, 5], &[]);
        let b = support_axes(100, 1, &[3, 9], &[]);
        let shared = a.iter().filter(|x| b.contains(x)).count();
        assert_eq!(shared, 1, "exactly the shared cell 3 collides");
    }

    #[test]
    fn support_spans_witness_and_target_deduped_and_graded() {
        // Witness cells at dim 2, target (boundary) cells at dim 1; cell 10
        // repeated in the witness collapses to one axis.
        let axes = support_axes(1, 2, &[10, 11, 10], &[4]);
        assert_eq!(axes.len(), 3, "{{10,11}}@2 + {{4}}@1, deduped");
        assert!(axes.iter().any(|x| x.dim == 2), "witness axes at its dim");
        assert!(axes.iter().any(|x| x.dim == 1), "target axes at dim − 1");
    }
}
