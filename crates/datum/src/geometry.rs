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
use assay::homology::{betti, betti_fast};

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
/// The support of a **work value on the wire** — decode the claim body
/// (either shape) and read the cells its witness (and, for a proof, its
/// target) touch, in global identity under `tag`. Cheap: decoding parses
/// structure only, running **no** SNF — so the scheduler can order a
/// claim's expensive verification without first doing it. A body that does
/// not decode yields an empty support (it schedules at the base lane; the
/// settler will refuse it on its own terms).
#[must_use]
pub fn claim_support(value: &[u8], tag: u64) -> Vec<Axis> {
    use assay::complex::{DeclaredClaim, ProofClaim};
    if let Ok(proof) = ProofClaim::decode(value) {
        let witness: Vec<u32> = proof.witness.iter().map(|&(cell, _)| cell).collect();
        let target: Vec<u32> = proof.target.iter().map(|&(cell, _)| cell).collect();
        return support_axes(tag, proof.dim, &witness, &target);
    }
    if let Ok(claim) = DeclaredClaim::decode(value) {
        let witness: Vec<u32> = claim.witness.iter().map(|&(cell, _)| cell).collect();
        return support_axes(tag, claim.dim, &witness, &[]);
    }
    Vec::new()
}

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
/// Computed by the **fast leg** `assay::homology::betti_fast` — field ranks
/// over `𝔽_p`, no integer Smith Normal Form on the scheduler's hot path
/// (#68). Exact for prime-smooth torsion (the crystallographic range); the
/// book's exact `betti` remains the settlement authority. A dimension whose
/// homology cannot be computed contributes `0` — a claim is never granted a
/// torsion lift it cannot substantiate.
#[must_use]
pub fn graded_torsion(complex: &DeclaredComplex) -> Vec<u64> {
    (0..complex.cells.len())
        .map(|d| {
            let dim = u32::try_from(d).unwrap_or(u32::MAX);
            // FAST leg (#68): the scheduler's torsion count via field ranks,
            // no integer SNF. grade_shapes (the book) keeps the exact betti.
            betti_fast(complex, dim)
                .map(|b| u64::try_from(b.torsion_count).unwrap_or(u64::MAX))
                .unwrap_or(0)
        })
        .collect()
}

/// The homological **shape** of a grade `k`: the number of FREE (ℤ) axes
/// and the invariant factors `m_i` of its TORSION (ℤ/m_iℤ) axes. Together
/// they type the credit a generator of this grade can carry — the free
/// axes count in ℤ, the torsion axes count in ℤ/m_iℤ.
///
/// Same SNF (`betti`) as [`graded_torsion`], but carrying the factor
/// **values** the book's modular arithmetic needs, where the scheduler
/// wanted only the count. This is the second, book-side reading of the one
/// torsion we defined: the scheduler lifts by how *many* cycles a grade
/// has; the section counts *within* each cyclic group `ℤ/m_iℤ`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeShape {
    /// Free rank `b_k`: the number of ℤ-valued axes.
    pub free_rank: usize,
    /// Torsion invariant factors `m_i` (each `> 1`): one `ℤ/m_iℤ` axis each.
    pub torsion: Vec<u64>,
}

/// The graded homological shapes of a complex: `grade_shapes(C)[k]` is the
/// free rank and torsion invariant factors of `H_k(C)`. The multi-axial
/// section is valued, grade by grade, in these groups.
#[must_use]
pub fn grade_shapes(complex: &DeclaredComplex) -> Vec<GradeShape> {
    (0..complex.cells.len())
        .map(|d| {
            let dim = u32::try_from(d).unwrap_or(u32::MAX);
            betti(complex, dim)
                .map(|b| GradeShape {
                    free_rank: b.free_rank,
                    torsion: b
                        .torsion
                        .iter()
                        .map(|m| u64::try_from(m).unwrap_or(u64::MAX))
                        .collect(),
                })
                .unwrap_or(GradeShape {
                    free_rank: 0,
                    torsion: Vec::new(),
                })
        })
        .collect()
}

/// The grades a settled claim engages, each with its homology shape — the
/// input to a convergence-section deposit (§6h). Decodes the claim's complex
/// once (`grade_shapes` = the exact SNF book leg) and pairs each grade the
/// witness (and, for a proof, its target) dimension touches with that
/// grade's shape. Empty if the value does not decode as a claim.
#[must_use]
pub fn claim_grades(value: &[u8], tag: u64) -> Vec<((u64, u32), GradeShape)> {
    use assay::complex::{DeclaredClaim, ProofClaim};
    let (complex, dims): (DeclaredComplex, Vec<u32>) = if let Ok(proof) = ProofClaim::decode(value) {
        let dim = proof.dim;
        let dims = if dim > 0 { vec![dim, dim - 1] } else { vec![dim] };
        (proof.complex, dims)
    } else if let Ok(claim) = DeclaredClaim::decode(value) {
        (claim.complex, vec![claim.dim])
    } else {
        return Vec::new();
    };
    let shapes = grade_shapes(&complex);
    let mut grades = Vec::new();
    for dim in dims {
        if let Some(shape) = shapes.get(dim as usize) {
            grades.push(((tag, dim), shape.clone()));
        }
    }
    grades
}

/// The crystallographic class of a universe's torsion (#70).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradeClass {
    /// Every torsion order lies in the crystallographic set `{2,3,4,6}` (or
    /// there is no torsion at all). `betti_fast` is **exact** for such a
    /// universe — the fast field-rank leg cannot mis-count `{2,3}`-smooth
    /// torsion, and these are the only lattice-compatible orders.
    Crystallographic,
    /// A torsion factor of an order **outside** `{2,3,4,6}`. `betti_fast`
    /// may undercount it (a bounded, turn-order-only scheduling-hint error);
    /// the book's exact SNF remains the settlement authority regardless. In
    /// a crystallographic domain this cannot occur, so its appearance is an
    /// **anomaly signal worth a verdict** — flagged, never rejected.
    Exotic {
        /// The offending invariant-factor order.
        order: u64,
    },
}

/// Classify a universe's torsion (exactly, via the book's SNF leg
/// [`grade_shapes`], not the fast leg — a fixed prime sweep cannot reliably
/// *detect* large-prime torsion). Crystallographic iff every invariant
/// factor has order in `{2,3,4,6}`. This is verification/telemetry, never an
/// admission gate here: settlement is exact for any torsion (#70). A court
/// MAY choose to flag or refuse `Exotic` universes; nothing here forces it.
#[must_use]
pub fn classify(complex: &DeclaredComplex) -> GradeClass {
    for shape in grade_shapes(complex) {
        for order in shape.torsion {
            if !matches!(order, 2 | 3 | 4 | 6) {
                return GradeClass::Exotic { order };
            }
        }
    }
    GradeClass::Crystallographic
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
    fn grade_shapes_carry_the_invariant_factors_the_book_counts_in() {
        // Same (2,4) complex: H_0 = ℤ/2 ⊕ ℤ/4, free rank 0. Where the
        // scheduler saw "2 cycles", the book sees the moduli [2, 4] — the
        // groups its torsion axes count in.
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
        let shapes = grade_shapes(&complex);
        let h0 = shapes.first().expect("grade 0");
        assert_eq!(h0.free_rank, 0, "both cells consumed into torsion");
        assert_eq!(h0.torsion, vec![2, 4], "the moduli, not their count");
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

    fn one_cell_boundary(m: i64) -> DeclaredComplex {
        // cells [1,1], ∂_1 = [[m]]: H_0 = coker([m]) = ℤ/mℤ (trivial at m=±1).
        DeclaredComplex {
            cells: vec![1, 1],
            ops: vec![vec![Entry { row: 0, col: 0, coeff: whole(m) }]],
        }
    }

    #[test]
    fn classify_accepts_crystallographic_orders_and_flags_exotic() {
        // {2,3,4,6} are the lattice-compatible orders — crystallographic.
        assert_eq!(classify(&one_cell_boundary(2)), GradeClass::Crystallographic);
        assert_eq!(classify(&one_cell_boundary(6)), GradeClass::Crystallographic);
        assert_eq!(classify(&one_cell_boundary(1)), GradeClass::Crystallographic, "torsion-free");
        // A (2,4) universe: both factors crystallographic.
        let mut op = vec![
            Entry { row: 0, col: 0, coeff: whole(2) },
            Entry { row: 1, col: 0, coeff: whole(6) },
            Entry { row: 0, col: 1, coeff: whole(4) },
            Entry { row: 1, col: 1, coeff: whole(8) },
        ];
        op.sort_by_key(|e| (e.col, e.row));
        let torsion_2_4 = DeclaredComplex { cells: vec![2, 2], ops: vec![op] };
        assert_eq!(classify(&torsion_2_4), GradeClass::Crystallographic);
        // Exotic: an order outside {2,3,4,6} is flagged WITH its order.
        assert_eq!(classify(&one_cell_boundary(5)), GradeClass::Exotic { order: 5 });
        assert_eq!(classify(&one_cell_boundary(8)), GradeClass::Exotic { order: 8 });
    }

    #[test]
    fn the_shipped_universes_are_crystallographic() {
        // #70 survey: every universe plumb actually ships classifies
        // Crystallographic. Torsion-free graphs, plus the theta-filled
        // complex whose ∂f = 2e1 − 2e2 yields ℤ/2 (order 2 ∈ {2,3,4,6}).
        use crate::domains::{demo_cycle_universe, demo_theta_filled_universe, demo_theta_universe};
        assert_eq!(classify(&demo_cycle_universe(5)), GradeClass::Crystallographic);
        assert_eq!(classify(&demo_theta_universe()), GradeClass::Crystallographic);

        let filled = demo_theta_filled_universe();
        assert_eq!(classify(&filled), GradeClass::Crystallographic);
        // It genuinely carries torsion (ℤ/2 in H_1) — proving the classifier
        // ACCEPTS crystallographic torsion, not merely torsion-freeness.
        assert!(
            grade_shapes(&filled).iter().any(|s| s.torsion.contains(&2)),
            "theta-filled has ℤ/2 torsion"
        );

        // The dihedral market's registered universe is a rewriting
        // presentation (dim 1) → totally unimodular → torsion-free.
        let dihedral = crate::corpus::dihedral_order_6_compiled().expect("dihedral compiles");
        assert_eq!(classify(&dihedral.complex), GradeClass::Crystallographic);
    }
}
