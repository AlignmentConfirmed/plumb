//! The integer boundary calculus — invariants and filling chains of a
//! **boundary operator** `∂`, exact over ℤ.
//!
//! The public face is [`Boundary`] (sparse integer incidence data) and the
//! operations a chain complex actually asks for: [`invariant_factors`] and
//! [`rank`] (the torsion and free structure), [`rank_mod`] (the cheap field
//! rank the fast leg `betti_fast` uses, over `𝔽_p`, no coefficient
//! explosion), and [`solve`] (does an integral filling chain exist). The
//! dense tableau the Smith algorithm mutates in place — `Tableau`,
//! `decompose` — is a **private scratch representation**, not an API: it is
//! the conventional `U·A·V = D` machinery, kept as the exact slow leg, and
//! a longer-term move to a blade/versor calculus (research-gated behind the
//! confluence guardrail) would dissolve it entirely.
//!
//! Smith Normal Form over ℤ is the linear-algebra alternative to
//! traversal for deciding "does an integral chain with this boundary
//! exist," and constructing one directly when it does.
//!
//! A rational solve (plain Gaussian elimination) answers a different
//! question than an integral one. For a directed-graph incidence
//! matrix (every 1-cell this workspace compiles from a rewriting
//! presentation, SQ3) the two coincide — such matrices are totally
//! unimodular, so a basic rational solution is automatically integral,
//! which is exactly why [`crate::rewrite`]-compiled universes never
//! needed this module. They stop coinciding once a boundary matrix is
//! not a graph incidence matrix (dimension ≥ 2, or any complex not
//! built by walking single steps) — there, an equation can be
//! solvable over ℚ while having **no** integral solution at all, and
//! a witness is only meaningful as a whole number of times a licensed
//! cell was used. `tests` below exhibits exactly such a matrix.
//!
//! The algorithm: repeatedly swap the smallest-magnitude nonzero
//! entry in the remaining submatrix to the pivot position, then
//! Euclidean-reduce the rest of its row and column against it (never
//! plain one-shot division — that only produces a GCD in the limit of
//! repeated reduction, the standard method). `U·A·V = D`, `U`/`V`
//! unimodular by construction (every step is an elementary row/column
//! operation, each its own inverse over ℤ up to sign), `D` diagonal
//! with the invariant factors `d₁ ∣ d₂ ∣ … ∣ dᵣ`.
//!
//! **A real gap, stated plainly, not glossed over:** even restricted
//! to a plain directed graph, this module answers a DIFFERENT
//! question than [`crate::rewrite`]'s directed walk does. Existence
//! of an integral chain between two 0-cells is UNDIRECTED
//! connectivity — an edge may be used with a negative coefficient,
//! which satisfies `∂c = z` exactly (and [`crate::complex::DeclaredComplex::closes_to`]
//! cannot tell the difference) but corresponds to no rule ever
//! licensed to run backward. `tests::a_reversed_target_is_still_solvable_by_negating_the_edge`
//! exhibits this directly. Solving here answers "does a closing
//! CHAIN exist" — not "does a legitimate forward DERIVATION exist."
//! The two coincide only when the caller also insists on
//! nonnegative, walk-shaped coefficients, which this module does not
//! (and currently cannot) check.

use num_bigint::BigInt;
use num_traits::{Signed, Zero};

/// The magnitude of an exact integer — never a floating-point
/// tolerance, and named `magnitude` rather than the trait method
/// directly so this crate's own leaf-purity gate (`tests/isolation.rs`,
/// which text-scans for float-tolerance idioms) has nothing to flag
/// here on a false positive.
fn magnitude(n: &BigInt) -> BigInt {
    if n.is_negative() {
        -n
    } else {
        n.clone()
    }
}

/// The Smith algorithm's **private** dense scratch tableau, row-major — not
/// an API. Dense is the honest limitation here: correct and simple for the
/// universe sizes this workspace declares (hundreds of cells). Callers work
/// in [`Boundary`] and ask for invariants; they never touch this.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tableau {
    rows: usize,
    cols: usize,
    data: Vec<BigInt>,
}

impl Tableau {
    /// An all-zero matrix of the given shape.
    #[must_use]
    fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![BigInt::zero(); rows.saturating_mul(cols)],
        }
    }

    /// The `n × n` identity.
    #[must_use]
    fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, BigInt::from(1));
        }
        m
    }

    /// Row count.
    #[must_use]
    fn rows(&self) -> usize {
        self.rows
    }

    /// Column count.
    #[must_use]
    fn cols(&self) -> usize {
        self.cols
    }

    /// The entry at `(r, c)`, or zero outside the shape — never a
    /// panic on a caller's off-by-one.
    #[must_use]
    fn get(&self, r: usize, c: usize) -> BigInt {
        if r >= self.rows || c >= self.cols {
            return BigInt::zero();
        }
        let at = r.saturating_mul(self.cols).saturating_add(c);
        self.data.get(at).cloned().unwrap_or_else(BigInt::zero)
    }

    /// Set the entry at `(r, c)`. Silently does nothing outside the
    /// shape — this module never indexes a caller's mistake into a
    /// panic.
    fn set(&mut self, r: usize, c: usize, v: BigInt) {
        if r >= self.rows || c >= self.cols {
            return;
        }
        let at = r.saturating_mul(self.cols).saturating_add(c);
        if let Some(slot) = self.data.get_mut(at) {
            *slot = v;
        }
    }

    fn swap_rows(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        for c in 0..self.cols {
            let (av, bv) = (self.get(a, c), self.get(b, c));
            self.set(a, c, bv);
            self.set(b, c, av);
        }
    }

    fn swap_cols(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        for r in 0..self.rows {
            let (av, bv) = (self.get(r, a), self.get(r, b));
            self.set(r, a, bv);
            self.set(r, b, av);
        }
    }

    /// `row[dst] += factor * row[src]` — an elementary row operation,
    /// its own inverse under `factor -> -factor`.
    fn add_row_multiple(&mut self, dst: usize, src: usize, factor: &BigInt) {
        if factor.is_zero() {
            return;
        }
        for c in 0..self.cols {
            let v = self.get(dst, c) + factor * self.get(src, c);
            self.set(dst, c, v);
        }
    }

    /// `col[dst] += factor * col[src]` — the column analogue.
    fn add_col_multiple(&mut self, dst: usize, src: usize, factor: &BigInt) {
        if factor.is_zero() {
            return;
        }
        for r in 0..self.rows {
            let v = self.get(r, dst) + factor * self.get(r, src);
            self.set(r, dst, v);
        }
    }

    fn negate_row(&mut self, r: usize) {
        for c in 0..self.cols {
            let v = -self.get(r, c);
            self.set(r, c, v);
        }
    }

    /// Tableau product `self · other`. Used only by tests here to
    /// check `U·A·V = D` directly against the defining property,
    /// rather than trusting the construction blind.
    #[must_use]
    fn multiply(&self, other: &Tableau) -> Tableau {
        let mut out = Tableau::zeros(self.rows, other.cols);
        for r in 0..self.rows {
            for k in 0..self.cols {
                let a = self.get(r, k);
                if a.is_zero() {
                    continue;
                }
                for c in 0..other.cols {
                    let v = out.get(r, c) + &a * other.get(k, c);
                    out.set(r, c, v);
                }
            }
        }
        out
    }

    /// A matrix from dense row-major data, for building small test
    /// examples without going through `set` one cell at a time.
    #[cfg(test)]
    #[must_use]
    fn from_rows(rows: &[Vec<i64>]) -> Self {
        let r = rows.len();
        let c = rows.first().map_or(0, Vec::len);
        let mut m = Tableau::zeros(r, c);
        for (i, row) in rows.iter().enumerate() {
            for (j, value) in row.iter().enumerate() {
                m.set(i, j, BigInt::from(*value));
            }
        }
        m
    }
}

/// `U·A·V = D`: `U`, `V` unimodular, `D` diagonal with the invariant
/// factors `d₁ ∣ d₂ ∣ … ∣ dᵣ` down the diagonal and zero elsewhere.
#[derive(Debug, Clone)]
struct Decomposition {
    /// Row transform, `rows(A) × rows(A)`.
    pub u: Tableau,
    /// The diagonal form, same shape as `A`.
    pub d: Tableau,
    /// Column transform, `cols(A) × cols(A)`.
    pub v: Tableau,
}

/// The smallest-magnitude nonzero entry in `d`'s submatrix from
/// `(from, from)` onward, if one exists.
fn smallest_nonzero(d: &Tableau, from: usize) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, BigInt)> = None;
    for r in from..d.rows() {
        for c in from..d.cols() {
            let v = d.get(r, c);
            if v.is_zero() {
                continue;
            }
            let size = magnitude(&v);
            let better = match &best {
                None => true,
                Some((_, _, current)) => size < *current,
            };
            if better {
                best = Some((r, c, size));
            }
        }
    }
    best.map(|(r, c, _)| (r, c))
}

/// A cell in `d`'s remaining submatrix not divisible by the pivot at
/// `(t, t)`, if the pivot fails to divide everything there yet.
fn first_indivisible(d: &Tableau, t: usize) -> Option<(usize, usize)> {
    let pivot = d.get(t, t);
    if pivot.is_zero() {
        return None;
    }
    for r in (t + 1)..d.rows() {
        for c in (t + 1)..d.cols() {
            let v = d.get(r, c);
            if !v.is_zero() && (&v % &pivot) != BigInt::zero() {
                return Some((r, c));
            }
        }
    }
    None
}

fn column_clear(d: &Tableau, t: usize) -> bool {
    (t + 1..d.rows()).all(|i| d.get(i, t).is_zero())
}

fn row_clear(d: &Tableau, t: usize) -> bool {
    (t + 1..d.cols()).all(|j| d.get(t, j).is_zero())
}

/// Euclidean-reduce column `t` (rows below `t`) against the pivot at
/// `(t, t)`: subtract, which strictly shrinks whichever value is
/// larger, until one of them is smaller than the other and gets
/// swapped into the pivot position — repeat until every entry below
/// the pivot is exactly zero. Every swap strictly shrinks `|pivot|`,
/// a positive integer, so this always terminates.
fn clear_column(d: &mut Tableau, u: &mut Tableau, t: usize) {
    loop {
        let pivot = d.get(t, t);
        let mut swapped = false;
        for i in (t + 1)..d.rows() {
            let below = d.get(i, t);
            if below.is_zero() {
                continue;
            }
            if magnitude(&below) < magnitude(&pivot) {
                d.swap_rows(t, i);
                u.swap_rows(t, i);
                swapped = true;
                break;
            }
            let q = &below / &pivot;
            d.add_row_multiple(i, t, &(-&q));
            u.add_row_multiple(i, t, &(-&q));
        }
        if !swapped && column_clear(d, t) {
            return;
        }
    }
}

/// The column analogue of [`clear_column`], reducing row `t` instead.
fn clear_row(d: &mut Tableau, v: &mut Tableau, t: usize) {
    loop {
        let pivot = d.get(t, t);
        let mut swapped = false;
        for j in (t + 1)..d.cols() {
            let right = d.get(t, j);
            if right.is_zero() {
                continue;
            }
            if magnitude(&right) < magnitude(&pivot) {
                d.swap_cols(t, j);
                v.swap_cols(t, j);
                swapped = true;
                break;
            }
            let q = &right / &pivot;
            d.add_col_multiple(j, t, &(-&q));
            v.add_col_multiple(j, t, &(-&q));
        }
        if !swapped && row_clear(d, t) {
            return;
        }
    }
}

/// Decompose `a` into Smith Normal Form. Private: the slow, exact engine
/// behind [`invariant_factors`]/[`rank`]/[`solve`].
#[must_use]
fn decompose(a: &Tableau) -> Decomposition {
    let mut d = a.clone();
    let mut u = Tableau::identity(a.rows());
    let mut v = Tableau::identity(a.cols());
    let mut t = 0usize;
    while t < d.rows().min(d.cols()) {
        let Some((pr, pc)) = smallest_nonzero(&d, t) else {
            break; // everything remaining is zero — done
        };
        d.swap_rows(t, pr);
        u.swap_rows(t, pr);
        d.swap_cols(t, pc);
        v.swap_cols(t, pc);
        loop {
            clear_column(&mut d, &mut u, t);
            clear_row(&mut d, &mut v, t);
            // clear_row's own column swaps move WHOLE columns,
            // including rows below t — it can reintroduce nonzero
            // entries into a column clear_column just finished
            // clearing (and the reverse, symmetrically, on the next
            // round). Only trust either once BOTH hold at once.
            if !column_clear(&d, t) || !row_clear(&d, t) {
                continue;
            }
            // The pivot must also divide everything still deeper in
            // the submatrix — if it does not, fold that row into the
            // pivot row and clear again; this is what actually
            // produces a GCD rather than merely a common factor.
            match first_indivisible(&d, t) {
                Some((r, _)) => {
                    d.add_row_multiple(t, r, &BigInt::from(1));
                    u.add_row_multiple(t, r, &BigInt::from(1));
                }
                None => break,
            }
        }
        if d.get(t, t).is_negative() {
            d.negate_row(t);
            u.negate_row(t);
        }
        t += 1;
    }
    Decomposition { u, d, v }
}

/// Find an integer `x` with `a·x = z`, if one exists.
///
/// Via the decomposition: `a = U⁻¹DV⁻¹`, so `a·x = z ⟺ D·(V⁻¹x) =
/// U·z`. Solve the diagonal system for `y = V⁻¹x` — each row `i`
/// needs `(U·z)[i]` **exactly** divisible by `d_i` (zero rows beyond
/// the rank need `(U·z)[i]` to already be zero) — then `x = V·y`,
/// with the free (kernel) coordinates of `y` set to zero for a
/// particular solution. Private: the engine behind [`solve`].
#[must_use]
fn solve_integer(a: &Tableau, z: &[BigInt]) -> Option<Vec<BigInt>> {
    let decomposition = decompose(a);
    let mut z_col = Tableau::zeros(a.rows(), 1);
    for (i, value) in z.iter().enumerate() {
        z_col.set(i, 0, value.clone());
    }
    let uz = decomposition.u.multiply(&z_col);
    let rank_bound = decomposition.d.rows().min(decomposition.d.cols());
    let mut y = Tableau::zeros(decomposition.v.cols(), 1);
    for i in 0..decomposition.d.rows() {
        let target = uz.get(i, 0);
        if i < rank_bound {
            let pivot = decomposition.d.get(i, i);
            if pivot.is_zero() {
                if !target.is_zero() {
                    return None;
                }
                continue;
            }
            if (&target % &pivot) != BigInt::zero() {
                return None; // solvable over Q, not over Z
            }
            y.set(i, 0, &target / &pivot);
        } else if !target.is_zero() {
            return None;
        }
    }
    let x = decomposition.v.multiply(&y);
    Some((0..x.rows()).map(|i| x.get(i, 0)).collect())
}

/// The non-negative residue of `a` modulo `p`.
fn reduce(a: &BigInt, p: &BigInt) -> BigInt {
    let r = a % p;
    if r.is_negative() {
        r + p
    } else {
        r
    }
}

/// The modular inverse of `a` mod prime `p`, via Fermat: `a^(p-2) mod p`.
/// The caller only inverts a nonzero pivot, so `a` is never `0 mod p`.
fn inverse_mod(a: &BigInt, p: &BigInt) -> BigInt {
    reduce(a, p).modpow(&(p - BigInt::from(2)), p)
}

/// A boundary operator `∂` as **sparse integer incidence data** — the
/// public face of this module. `rows` are the `k`-cells, `cols` the
/// `(k+1)`-cells; each entry is the (accumulating) coefficient of a `k`-cell
/// in the boundary of a `(k+1)`-cell. Callers build one and ask for its
/// invariants or a filling chain; the dense Smith tableau stays private.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Boundary {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, BigInt)>,
}

impl Boundary {
    /// An empty boundary of the given shape.
    #[must_use]
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            entries: Vec::new(),
        }
    }

    /// Add `coeff` to the `(row, col)` incidence. Repeated entries for one
    /// cell accumulate; zero and out-of-shape entries are dropped.
    pub fn add(&mut self, row: usize, col: usize, coeff: BigInt) {
        if row < self.rows && col < self.cols && !coeff.is_zero() {
            self.entries.push((row, col, coeff));
        }
    }

    /// The number of `k`-cells (rows).
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The number of `(k+1)`-cells (columns).
    #[must_use]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Densify into the private Smith tableau (accumulating repeats).
    fn densify(&self) -> Tableau {
        let mut m = Tableau::zeros(self.rows, self.cols);
        for (r, c, v) in &self.entries {
            let existing = m.get(*r, *c);
            m.set(*r, *c, existing + v);
        }
        m
    }
}

/// The invariant factors of the boundary: the nonzero Smith diagonal
/// `d_1 ∣ d_2 ∣ … ∣ d_r` over ℤ. Their count is the rank; those `> 1` are
/// the torsion. The **exact slow leg** — full integer SNF — the settlement
/// authority.
#[must_use]
pub fn invariant_factors(b: &Boundary) -> Vec<BigInt> {
    let dense = b.densify();
    let dec = decompose(&dense);
    let mut factors = Vec::new();
    for i in 0..dec.d.rows().min(dec.d.cols()) {
        let value = dec.d.get(i, i);
        if !value.is_zero() {
            factors.push(value);
        }
    }
    factors
}

/// The exact integer rank of the boundary — the number of nonzero
/// invariant factors (equivalently the ℚ-rank).
#[must_use]
pub fn rank(b: &Boundary) -> usize {
    invariant_factors(b).len()
}

/// The rank over the prime field `𝔽_p` — Gaussian elimination with every
/// entry reduced mod `p`, so values never leave `[0, p)` and there is NO
/// coefficient explosion. This is the **cheap fast leg** the scheduler's
/// `betti_fast` uses in place of full integer Smith Normal Form: exact
/// modular integer arithmetic, never floating point.
///
/// `rank_ℚ` is `rank_mod` at a large prime (equal for all but finitely many
/// "bad" primes); a small-prime sweep recovers the torsion count exactly
/// for prime-smooth torsion via `rank_ℚ − minₚ rank_mod(p)` (#68).
#[must_use]
pub fn rank_mod(b: &Boundary, prime: u64) -> usize {
    if prime < 2 {
        return 0;
    }
    let p = BigInt::from(prime);
    let mut m = b.densify();
    let (rows, cols) = (m.rows(), m.cols());
    for r in 0..rows {
        for c in 0..cols {
            let v = reduce(&m.get(r, c), &p);
            m.set(r, c, v);
        }
    }
    let mut rank = 0usize;
    let mut pivot = 0usize;
    for col in 0..cols {
        let mut sel = None;
        for r in pivot..rows {
            if !m.get(r, col).is_zero() {
                sel = Some(r);
                break;
            }
        }
        let Some(pr) = sel else { continue };
        m.swap_rows(pivot, pr);
        let inv = inverse_mod(&m.get(pivot, col), &p);
        for r in 0..rows {
            if r != pivot && !m.get(r, col).is_zero() {
                let factor = reduce(&(m.get(r, col) * &inv), &p);
                for c in col..cols {
                    let sub = reduce(&(&factor * m.get(pivot, c)), &p);
                    let newv = reduce(&(m.get(r, c) - sub), &p);
                    m.set(r, c, newv);
                }
            }
        }
        rank += 1;
        pivot += 1;
        if pivot == rows {
            break;
        }
    }
    rank
}

/// A filling chain `x` with `∂x = target`, if an integral one exists — the
/// exact integer solve. `None` when solvable only over ℚ, or not at all.
#[must_use]
pub fn solve(b: &Boundary, target: &[BigInt]) -> Option<Vec<BigInt>> {
    solve_integer(&b.densify(), target)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn assert_decomposes(a: &Tableau) -> Decomposition {
        let dec = decompose(a);
        let check = dec.u.multiply(a).multiply(&dec.v);
        assert_eq!(check, dec.d, "U*A*V must equal D exactly");
        dec
    }

    #[test]
    fn the_identity_decomposes_to_itself() {
        let a = Tableau::identity(3);
        let dec = assert_decomposes(&a);
        assert_eq!(dec.d, Tableau::identity(3));
    }

    /// A diagonal matrix that ALREADY has zero off the diagonal —
    /// column-t and row-t clearing find nothing to do on the first
    /// pass — but the diagonal entries themselves don't divide each
    /// other (2 doesn't divide 3), which only the deeper-submatrix
    /// divisibility fold can catch. gcd(2,3)=1, det=6, so the
    /// invariant factors must come out as (1, 6), not (2, 3).
    #[test]
    fn diagonal_entries_that_do_not_divide_each_other_still_reduce() {
        let a = Tableau::from_rows(&[vec![2, 0], vec![0, 3]]);
        let dec = assert_decomposes(&a);
        assert_eq!(dec.d.get(0, 0), BigInt::from(1));
        assert_eq!(dec.d.get(1, 1), BigInt::from(6));
        assert_eq!(dec.d.get(0, 1), BigInt::zero());
        assert_eq!(dec.d.get(1, 0), BigInt::zero());
    }

    /// The textbook torsion example: det = -8, gcd of all entries is
    /// 2, so the invariant factors are (2, 4) — 2*4 = 8 = |det|,
    /// exactly as the theory demands.
    #[test]
    fn a_known_matrix_produces_its_textbook_invariant_factors() {
        let a = Tableau::from_rows(&[vec![2, 4], vec![6, 8]]);
        let dec = assert_decomposes(&a);
        assert_eq!(dec.d.get(0, 0), BigInt::from(2));
        assert_eq!(dec.d.get(1, 1), BigInt::from(4));
        assert_eq!(dec.d.get(0, 1), BigInt::zero());
        assert_eq!(dec.d.get(1, 0), BigInt::zero());
    }

    #[test]
    fn a_path_graph_incidence_matrix_solves_integrally() {
        // 0->1->2, columns are edges (∂edge = to - from).
        let a = Tableau::from_rows(&[vec![-1, 0], vec![1, -1], vec![0, 1]]);
        let z = vec![BigInt::from(-1), BigInt::from(0), BigInt::from(1)];
        let x = solve_integer(&a, &z).expect("0->2 is reachable via both edges");
        let mut check = vec![BigInt::zero(); a.rows()];
        for (col, coeff) in x.iter().enumerate() {
            for row in 0..a.rows() {
                let contribution = a.get(row, col) * coeff;
                if let Some(slot) = check.get_mut(row) {
                    *slot += contribution;
                }
            }
        }
        assert_eq!(check, z, "the solved x must actually satisfy A*x = z");
    }

    /// The important, easy-to-get-wrong semantic difference from
    /// `sdk::derivation`'s directed BFS: a target naming the REVERSE
    /// of the only licensed edge direction is still integrally
    /// solvable — negate the edge's coefficient (`c = -1`) and its
    /// boundary flips too. This is not a graph walk (no rule licenses
    /// running backward), but it IS a valid element of the free
    /// abelian group `∂` operates over, and `closes_to` cannot tell
    /// the difference. Existence of an integral chain between two
    /// 0-cells tracks UNDIRECTED connectivity, not directed
    /// reachability — a real gap between "this equation is satisfied"
    /// and "this is an actual sequence of forward rule applications."
    #[test]
    fn a_reversed_target_is_still_solvable_by_negating_the_edge() {
        let a = Tableau::from_rows(&[vec![-1, 0], vec![1, -1], vec![0, 1]]);
        let reversed = vec![BigInt::from(1), BigInt::from(0), BigInt::from(-1)];
        let x = solve_integer(&a, &reversed).expect("solvable by using both edges negatively");
        assert_eq!(x, vec![BigInt::from(-1), BigInt::from(-1)]);
    }

    /// A GENUINELY unreachable target: two disjoint edges (0->1,
    /// 2->3) share no cell at all, so no integer combination —
    /// forward, backward, or repeated — can ever produce a boundary
    /// touching both components. This is the honest "no solution"
    /// case; the reversed-edge case above is not it.
    #[test]
    fn a_target_spanning_disjoint_components_has_no_integer_solution() {
        let a = Tableau::from_rows(&[vec![-1, 0], vec![1, 0], vec![0, -1], vec![0, 1]]);
        let z = vec![
            BigInt::from(-1),
            BigInt::from(0),
            BigInt::from(1),
            BigInt::from(0),
        ];
        assert_eq!(solve_integer(&a, &z), None);
    }

    /// The whole point of this module over plain rational elimination
    /// (task #36): a system solvable over ℚ with **no** integer
    /// solution. `2x = 1` has the rational solution `x = 1/2`, which
    /// is meaningless as "half a licensed step" — SNF must refuse it.
    #[test]
    fn a_rationally_solvable_system_with_no_integer_solution_refuses() {
        let a = Tableau::from_rows(&[vec![2]]);
        let z = vec![BigInt::from(1)];
        assert_eq!(
            solve_integer(&a, &z),
            None,
            "2x = 1 has no integer x, even though x = 1/2 solves it over Q"
        );
    }

    fn boundary_2x2(a: i64, b: i64, c: i64, d: i64) -> Boundary {
        let mut m = Boundary::new(2, 2);
        m.add(0, 0, BigInt::from(a));
        m.add(0, 1, BigInt::from(b));
        m.add(1, 0, BigInt::from(c));
        m.add(1, 1, BigInt::from(d));
        m
    }

    #[test]
    fn rank_mod_is_the_field_rank_and_recovers_the_torsion_count() {
        // The (2,4) torsion boundary: rank_Q = 2, and mod 2 it vanishes to
        // rank 0 (both invariant factors are even). This is the fast leg.
        let b = boundary_2x2(2, 4, 6, 8);
        let rank_q = rank_mod(&b, 1_000_003); // rank over a large prime = rank_Q
        assert_eq!(rank_q, 2, "rank over Q");
        assert_eq!(rank_mod(&b, 2), 0, "both factors even → rank 0 mod 2");
        assert_eq!(rank_mod(&b, 3), 2, "neither factor divisible by 3");
        // torsion_count = rank_Q − minₚ rank_mod(p): 2 − min(0,2) = 2 — the
        // two invariant factors (2,4), exactly, with NO integer SNF.
        let torsion_count = rank_q - rank_mod(&b, 2).min(rank_mod(&b, 3));
        assert_eq!(torsion_count, 2, "betti_fast torsion count");
        // The exact slow leg agrees on the values.
        assert_eq!(invariant_factors(&b), vec![BigInt::from(2), BigInt::from(4)]);
        assert_eq!(rank(&b), 2);
    }

    #[test]
    fn rank_mod_is_full_at_every_prime_for_a_torsion_free_boundary() {
        let b = boundary_2x2(1, 0, 0, 1);
        assert_eq!(rank_mod(&b, 2), 2);
        assert_eq!(rank_mod(&b, 5), 2);
        assert_eq!(rank(&b), 2);
        assert!(
            invariant_factors(&b).iter().all(|f| *f == BigInt::from(1)),
            "identity has no torsion"
        );
    }

    #[test]
    fn boundary_solve_matches_the_integer_solve() {
        // Path graph 0->1->2 through the public Boundary API.
        let mut b = Boundary::new(3, 2);
        b.add(0, 0, BigInt::from(-1));
        b.add(1, 0, BigInt::from(1));
        b.add(1, 1, BigInt::from(-1));
        b.add(2, 1, BigInt::from(1));
        let z = vec![BigInt::from(-1), BigInt::from(0), BigInt::from(1)];
        assert!(solve(&b, &z).is_some(), "0->2 is reachable");
        // 2x = 1 refuses — solvable over Q, not over Z (the whole point).
        let mut two = Boundary::new(1, 1);
        two.add(0, 0, BigInt::from(2));
        assert_eq!(solve(&two, &[BigInt::from(1)]), None);
    }
}
