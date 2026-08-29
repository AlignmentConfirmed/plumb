//! H_k = ker(∂_k) / im(∂_{k+1}) — task #38, built directly on
//! [`crate::snf`] (task #36).
//!
//! Two things this buys, kept as two functions rather than one,
//! because they answer different questions:
//!
//! - [`homologous`] — are two SPECIFIC witnesses equivalent up to a
//!   filling `(k+1)`-chain (`c' = c + ∂ₖ₊₁h`)? A direct consumer of
//!   [`crate::complex::DeclaredComplex::solve`]: `c` and `c'` are
//!   homologous exactly when `solve(k+1, c − c')` finds an `h`. This
//!   is what `with_confluences`' diamonds are FOR — two derivations
//!   of one lemma are homologous when a compiled diamond fills their
//!   difference, checked here by linear algebra instead of the
//!   brute-force "ask every diamond" scan a caller would otherwise
//!   need to write.
//!
//! - [`betti`] — the GLOBAL structure of `H_k` itself: its free rank
//!   (the Betti number) and torsion coefficients, independent of any
//!   particular witness. The standard Smith-Normal-Form algorithm for
//!   chain-complex homology: because `∂_k ∘ ∂_{k+1} = 0` is already
//!   guaranteed by [`crate::complex::DeclaredComplex::admit`],
//!   `∂_{k+1}`'s own invariant factors (computed independently of
//!   `∂_k`, no basis restriction needed) are exactly `H_k`'s torsion,
//!   and `dim(C_k) − rank(∂_k) − rank(∂_{k+1})` is its free rank.

use num_bigint::BigInt;
use num_traits::Zero;

use crate::complex::{DeclaredComplex, Entry, SolveRefused};
use crate::Exact;

/// Why a homology computation refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomologyRefused {
    /// `dim` names a dimension this complex has no cells for.
    NoSuchDimension,
    /// A boundary entry carries a non-integer coefficient. Torsion is
    /// only meaningful for the TRUE integer matrix — unlike
    /// [`crate::complex::DeclaredComplex::solve`], this cannot rescale
    /// its way past a fraction, since rescaling would corrupt the
    /// actual invariant-factor values homology reports.
    NonIntegerCoefficient,
}

/// `H_k`'s structure: a free part of this rank, plus these torsion
/// coefficients (each `> 1`, one summand `ℤ/dℤ` per entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Betti {
    /// The Betti number: `H_k`'s free rank.
    pub free_rank: usize,
    /// Torsion coefficients, each `> 1`.
    pub torsion: Vec<BigInt>,
}

fn to_boundary(rows: u32, cols: u32, op: &[Entry]) -> Result<crate::snf::Boundary, HomologyRefused> {
    let mut b = crate::snf::Boundary::new(rows as usize, cols as usize);
    for entry in op {
        if entry.coeff.denom() != &BigInt::from(1) {
            return Err(HomologyRefused::NonIntegerCoefficient);
        }
        b.add(entry.row as usize, entry.col as usize, entry.coeff.numer().clone());
    }
    Ok(b)
}

/// `H_dim`'s free rank and torsion.
pub fn betti(complex: &DeclaredComplex, dim: u32) -> Result<Betti, HomologyRefused> {
    let dim = dim as usize;
    let n_k = *complex.cells.get(dim).ok_or(HomologyRefused::NoSuchDimension)?;

    let rank_k = if dim == 0 {
        0 // C_0 has no boundary out of it; ker(∂_0) is all of C_0.
    } else {
        let rows = *complex.cells.get(dim - 1).ok_or(HomologyRefused::NoSuchDimension)?;
        match complex.ops.get(dim - 1) {
            Some(op) => crate::snf::rank(&to_boundary(rows, n_k, op)?),
            None => 0,
        }
    };

    let (rank_k1, torsion) = match complex.ops.get(dim) {
        Some(op) => {
            let cols = *complex.cells.get(dim + 1).unwrap_or(&0);
            // The nonzero invariant factors of ∂_{dim+1}: their count is the
            // rank, those > 1 are H_dim's torsion.
            let factors = crate::snf::invariant_factors(&to_boundary(n_k, cols, op)?);
            let rank = factors.len();
            let torsion: Vec<BigInt> = factors
                .into_iter()
                .filter(|value| *value != BigInt::from(1))
                .collect();
            (rank, torsion)
        }
        None => (0, Vec::new()),
    };

    Ok(Betti {
        free_rank: (n_k as usize).saturating_sub(rank_k).saturating_sub(rank_k1),
        torsion,
    })
}

/// Are `a` and `b` — two `dim`-chains — equivalent up to a filling
/// `(dim+1)`-chain? `true` exactly when `solve(dim+1, a − b)` finds a
/// witness; `a == b` is always `true` here (the zero chain trivially
/// fills its own zero difference).
pub fn homologous(
    complex: &DeclaredComplex,
    dim: u32,
    a: &[(u32, Exact)],
    b: &[(u32, Exact)],
) -> Result<bool, SolveRefused> {
    let target = difference(a, b);
    match complex.solve(dim.saturating_add(1), &target) {
        Ok(_) => Ok(true),
        Err(SolveRefused::NoIntegralSolution) => Ok(false),
        Err(other) => Err(other),
    }
}

fn difference(a: &[(u32, Exact)], b: &[(u32, Exact)]) -> Vec<(u32, Exact)> {
    let mut acc: std::collections::BTreeMap<u32, Exact> = std::collections::BTreeMap::new();
    for (cell, coeff) in a {
        *acc.entry(*cell).or_insert_with(crate::zero) += coeff.clone();
    }
    for (cell, coeff) in b {
        *acc.entry(*cell).or_insert_with(crate::zero) -= coeff.clone();
    }
    acc.into_iter().filter(|(_, c)| !c.is_zero()).collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::whole;

    /// The n-cycle: n vertices, n edges, no 2-cells. A classic
    /// topological fact, not a made-up expectation: `H_1` of a circle
    /// is `ℤ` — free rank 1, zero torsion.
    fn cycle(n: u32) -> DeclaredComplex {
        let mut op = Vec::new();
        for i in 0..n {
            let (source, target) = (i, (i + 1) % n);
            let mut pair = vec![
                Entry { row: target, col: i, coeff: whole(1) },
                Entry { row: source, col: i, coeff: whole(-1) },
            ];
            pair.sort_by_key(|e| (e.col, e.row));
            op.extend(pair);
        }
        DeclaredComplex {
            cells: vec![n, n],
            ops: vec![op],
        }
    }

    #[test]
    fn a_circles_first_betti_number_is_one_with_no_torsion() {
        let complex = cycle(5);
        let h1 = betti(&complex, 1).expect("H_1 exists");
        assert_eq!(h1.free_rank, 1, "a circle has one independent cycle");
        assert!(h1.torsion.is_empty(), "a circle's H_1 has no torsion");
    }

    /// A synthetic ∂_1 with a KNOWN invariant-factor pair (the same
    /// textbook matrix `snf` itself validates against) — this complex
    /// corresponds to no real geometric object; it exists only to
    /// prove `betti` actually reports torsion when it is genuinely
    /// there; H_0 = C_0 / im(∂_1), which for invariant factors (2, 4)
    /// is torsion (2, 4) with zero free rank (2 cells, both consumed).
    #[test]
    fn betti_reports_real_torsion_when_present() {
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
        let h0 = betti(&complex, 0).expect("H_0 exists");
        assert_eq!(h0.free_rank, 0);
        assert_eq!(h0.torsion, vec![BigInt::from(2), BigInt::from(4)]);
    }

    /// An invariant factor of exactly `1` means that direction is
    /// fully consumed, not torsion — `ℤ/1ℤ` is the trivial group.
    /// `∂_1` here (3 zero-cells, 2 one-cells, `[[1,0],[0,1],[0,0]]`)
    /// has invariant factors `(1, 1)`: both trivial. `betti_reports_real_torsion_when_present`
    /// alone cannot catch a broken filter, because every invariant
    /// factor in it is already genuine torsion (2, 4) — this is the
    /// case that actually needs the `!= 1` filter to matter.
    #[test]
    fn a_trivial_invariant_factor_of_one_is_not_torsion() {
        let op = vec![
            Entry { row: 0, col: 0, coeff: whole(1) },
            Entry { row: 1, col: 1, coeff: whole(1) },
        ];
        let complex = DeclaredComplex {
            cells: vec![3, 2],
            ops: vec![op],
        };
        let h0 = betti(&complex, 0).expect("H_0 exists");
        assert_eq!(h0.free_rank, 1, "3 cells, rank-2 image consumes 2 of them");
        assert!(h0.torsion.is_empty(), "invariant factors of 1 are not torsion");
    }

    #[test]
    fn a_nonexistent_dimension_refuses_by_name() {
        let complex = DeclaredComplex {
            cells: vec![1],
            ops: vec![],
        };
        assert_eq!(betti(&complex, 3), Err(HomologyRefused::NoSuchDimension));
    }

    /// The stated use case: two genuinely different derivations of one
    /// lemma (`with_confluences`' whole reason to exist) are
    /// homologous — equivalent up to the filling diamond — while
    /// remaining unequal as raw witnesses.
    #[test]
    fn two_derivations_of_one_lemma_are_homologous_not_equal() {
        let compiled = crate::rewrite::Presentation {
            alphabet: vec![b'a', b'b'],
            rules: vec![(vec![b'b', b'a'], vec![b'a', b'b'])],
        }
        .compile(4)
        .expect("compiles")
        .with_confluences()
        .expect("confluences");

        let (baba, abba, baab, abab) = (
            compiled.word(b"baba").expect("w"),
            compiled.word(b"abba").expect("w"),
            compiled.word(b"baab").expect("w"),
            compiled.word(b"abab").expect("w"),
        );
        let left = compiled.derive(&[baba, abba, abab]).expect("left path");
        let right = compiled.derive(&[baba, baab, abab]).expect("right path");
        assert_ne!(left, right, "genuinely different derivations");

        assert_eq!(
            homologous(&compiled.complex, 1, &left, &right),
            Ok(true),
            "a compiled diamond fills their difference"
        );
    }
}
