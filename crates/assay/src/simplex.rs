//! Exact-rational two-phase simplex — task #37's engine.
//!
//! `min c^T x` subject to `A x = b`, `x ≥ 0`. Every arithmetic
//! operation is over [`Exact`] — no floating point, no tolerance, no
//! "close enough" pivot: a reduced cost is either exactly negative or
//! it is not, an artificial variable is either exactly zero or the
//! system was infeasible.
//!
//! [`minimize_l1`] is the only thing this module exists for: the
//! sparsest witness (fewest, smallest licensed cells used) is
//! `L₀`-minimization, which is NP-hard exactly; `L₁` relaxation
//! (Basis Pursuit — split each variable into its positive and
//! negative part and minimize their sum) is the tractable convex
//! stand-in, biased toward sparse solutions without being one. It
//! answers a DIFFERENT question than [`crate::snf`]: that module asks
//! "does an INTEGER chain exist"; this one asks "what is the
//! RATIONAL-least-total-magnitude chain" — the two need not agree,
//! and this module never rounds one into the other.

use crate::{whole, zero, Exact};
use num_traits::{Signed, Zero};

/// A dense simplex tableau: `rows` constraint rows plus one objective
/// row, `cols` variable columns plus one RHS column.
struct Tableau {
    rows: usize,
    cols: usize,
    data: Vec<Exact>,
    basis: Vec<usize>,
}

impl Tableau {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![zero(); (rows.saturating_add(1)).saturating_mul(cols.saturating_add(1))],
            basis: vec![0; rows],
        }
    }

    fn width(&self) -> usize {
        self.cols.saturating_add(1)
    }

    fn get(&self, r: usize, c: usize) -> Exact {
        let at = r.saturating_mul(self.width()).saturating_add(c);
        self.data.get(at).cloned().unwrap_or_else(zero)
    }

    fn set(&mut self, r: usize, c: usize, v: Exact) {
        let at = r.saturating_mul(self.width()).saturating_add(c);
        if let Some(slot) = self.data.get_mut(at) {
            *slot = v;
        }
    }

    fn rhs_col(&self) -> usize {
        self.cols
    }

    fn objective_row(&self) -> usize {
        self.rows
    }

    fn basis_at(&self, row: usize) -> usize {
        self.basis.get(row).copied().unwrap_or(usize::MAX)
    }

    fn set_basis(&mut self, row: usize, col: usize) {
        if let Some(slot) = self.basis.get_mut(row) {
            *slot = col;
        }
    }
}

/// Pivot on `(row, col)`: normalize `row` so `col` becomes `1` there,
/// then eliminate `col` from every other row, including the
/// objective row.
fn pivot(t: &mut Tableau, row: usize, col: usize) {
    let pivot_value = t.get(row, col);
    for c in 0..=t.rhs_col() {
        let v = t.get(row, c) / &pivot_value;
        t.set(row, c, v);
    }
    for r in 0..=t.objective_row() {
        if r == row {
            continue;
        }
        let factor = t.get(r, col);
        if factor.is_zero() {
            continue;
        }
        for c in 0..=t.rhs_col() {
            let v = t.get(r, c) - &factor * t.get(row, c);
            t.set(r, c, v);
        }
    }
    t.set_basis(row, col);
}

/// Drive the objective row's negative reduced costs to none, via
/// Bland's rule (smallest-index entering column, and smallest basic
/// variable index breaking ratio-test ties) — the standard
/// anti-cycling rule, needed because this tableau can be genuinely
/// degenerate (an L1 formulation's artificial-variable rows often
/// land on exactly zero). Returns `false` if unbounded.
///
/// `eligible_cols` bounds which columns may ENTER — phase 1 passes
/// `t.cols` (every column, artificials included, is fair game while
/// hunting for feasibility); phase 2 must pass the count of real
/// variables only. An artificial variable is retired once phase 1
/// zeroes it out; letting it re-enter in phase 2 (its reduced cost
/// can absolutely go negative there) would silently reintroduce
/// infeasibility that phase 1 already paid to eliminate.
fn run(t: &mut Tableau, eligible_cols: usize) -> bool {
    loop {
        let entering = (0..eligible_cols).find(|&c| t.get(t.objective_row(), c).is_negative());
        let Some(entering) = entering else {
            return true;
        };
        let mut leaving: Option<(usize, Exact, usize)> = None;
        for r in 0..t.rows {
            let coeff = t.get(r, entering);
            if !coeff.is_positive() {
                continue;
            }
            let ratio = t.get(r, t.rhs_col()) / &coeff;
            let basis_var = t.basis_at(r);
            let better = match &leaving {
                None => true,
                Some((_, best_ratio, best_var)) => {
                    ratio < *best_ratio || (ratio == *best_ratio && basis_var < *best_var)
                }
            };
            if better {
                leaving = Some((r, ratio, basis_var));
            }
        }
        let Some((row, _, _)) = leaving else {
            return false; // unbounded
        };
        pivot(t, row, entering);
    }
}

/// `min c^T x` subject to `A x = b`, `x ≥ 0`. `None` if infeasible.
/// Every caller in this module passes `c ≥ 0` componentwise (an L1
/// cost), so phase 2 is always bounded below by zero — unboundedness
/// would mean a bug upstream, not a real answer, and is reported as
/// `None` rather than pretended away.
fn minimize(a: &[Vec<Exact>], b: &[Exact], c: &[Exact]) -> Option<Vec<Exact>> {
    let m = a.len();
    let n = c.len();

    let mut a = a.to_vec();
    let mut b = b.to_vec();
    for i in 0..m {
        let negative = b.get(i).is_some_and(Signed::is_negative);
        if negative {
            if let Some(row) = a.get_mut(i) {
                for v in row.iter_mut() {
                    *v = -v.clone();
                }
            }
            if let Some(v) = b.get_mut(i) {
                *v = -v.clone();
            }
        }
    }

    // Phase 1: minimize the sum of one artificial variable per row,
    // to find any feasible basic solution at all.
    let total_cols = n.saturating_add(m);
    let mut t = Tableau::new(m, total_cols);
    for i in 0..m {
        for j in 0..n {
            let value = a.get(i).and_then(|row| row.get(j)).cloned().unwrap_or_else(zero);
            t.set(i, j, value);
        }
        t.set(i, n.saturating_add(i), whole(1));
        t.set(i, total_cols, b.get(i).cloned().unwrap_or_else(zero));
        t.set_basis(i, n.saturating_add(i));
    }
    // The phase-1 objective row, in reduced-cost form: `c_j - Σᵢ
    // t[i][j]` (every artificial starts basic with phase-1 cost 1, so
    // `c_j` is 1 for an artificial column and 0 for a real one — this
    // is NOT optional bookkeeping: without it, an artificial's own
    // column reads as having reduced cost -1 instead of the 0 a basic
    // column's reduced cost must always be, and phase 1 can pivot an
    // already-basic artificial "in" over itself, corrupting the run).
    for j in 0..total_cols {
        let mut cost = if j >= n { whole(1) } else { zero() };
        for i in 0..m {
            cost -= t.get(i, j);
        }
        t.set(t.objective_row(), j, cost);
    }
    let mut phase1_value = zero();
    for i in 0..m {
        phase1_value -= t.get(i, total_cols);
    }
    t.set(t.objective_row(), total_cols, phase1_value);

    run(&mut t, total_cols);
    if !t.get(t.objective_row(), total_cols).is_zero() {
        return None; // infeasible: artificials could not be zeroed
    }

    // Any artificial still basic must be sitting at exactly zero
    // (the phase-1 optimum is zero and every artificial is ≥ 0) —
    // drive it out via any nonzero real column in its row, a
    // degenerate pivot that changes no RHS value anywhere.
    for i in 0..m {
        if t.basis_at(i) < n {
            continue;
        }
        if let Some(col) = (0..n).find(|&c| !t.get(i, c).is_zero()) {
            pivot(&mut t, i, col);
        }
    }

    // Phase 2: the real objective, reduced against the basis phase 1
    // left behind.
    for j in 0..total_cols {
        let value = if j < n { c.get(j).cloned().unwrap_or_else(zero) } else { zero() };
        t.set(t.objective_row(), j, value);
    }
    t.set(t.objective_row(), total_cols, zero());
    for i in 0..m {
        let basic = t.basis_at(i);
        let cost = if basic < n { c.get(basic).cloned().unwrap_or_else(zero) } else { zero() };
        if cost.is_zero() {
            continue;
        }
        for j in 0..=t.rhs_col() {
            let v = t.get(t.objective_row(), j) - &cost * t.get(i, j);
            t.set(t.objective_row(), j, v);
        }
    }

    if !run(&mut t, n) {
        return None; // unbounded — should not happen for an L1 cost
    }

    let mut x = vec![zero(); n];
    for i in 0..m {
        let basic = t.basis_at(i);
        if basic < n {
            if let Some(slot) = x.get_mut(basic) {
                *slot = t.get(i, t.rhs_col());
            }
        }
    }
    Some(x)
}

/// The `L₁`-minimal (rational, not necessarily integral) `x` with
/// `A x = z` — Basis Pursuit via variable splitting (`x = u − v`,
/// `u, v ≥ 0`, minimize `Σu + Σv`).
pub fn minimize_l1(a: &[Vec<Exact>], z: &[Exact]) -> Option<Vec<Exact>> {
    let n = a.first().map_or(0, Vec::len);
    let mut augmented: Vec<Vec<Exact>> = Vec::with_capacity(a.len());
    for row in a {
        let mut widened = row.clone();
        widened.extend(row.iter().map(|v| -v.clone()));
        augmented.push(widened);
    }
    let cost = vec![whole(1); n.saturating_mul(2)];
    let uv = minimize(&augmented, z, &cost)?;
    let mut x = Vec::with_capacity(n);
    for i in 0..n {
        let u = uv.get(i).cloned().unwrap_or_else(zero);
        let v = uv.get(n.saturating_add(i)).cloned().unwrap_or_else(zero);
        x.push(u - v);
    }
    Some(x)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn row(values: &[i64]) -> Vec<Exact> {
        values.iter().map(|v| whole(*v)).collect()
    }

    #[test]
    fn a_single_equation_finds_its_l1_minimal_point() {
        // u - v = 5, minimize |x| where x = u - v: the answer is x=5.
        let a = vec![row(&[1])];
        let z = vec![whole(5)];
        let x = minimize_l1(&a, &z).expect("feasible");
        assert_eq!(x, vec![whole(5)]);
    }

    #[test]
    fn an_infeasible_system_refuses() {
        // 0*x = 1 has no solution at all.
        let a = vec![row(&[0])];
        let z = vec![whole(1)];
        assert_eq!(minimize_l1(&a, &z), None);
    }

    fn magnitude(v: &Exact) -> Exact {
        if v.is_negative() {
            -v.clone()
        } else {
            v.clone()
        }
    }

    fn l1_norm(x: &[Exact]) -> Exact {
        let mut total = zero();
        for v in x {
            total += magnitude(v);
        }
        total
    }

    /// The real point of this module: a genuinely underdetermined
    /// system where the SPARSEST solution is not the first one a
    /// naive elimination would hand back. `x2 = 1` alone satisfies
    /// both equations (L1 norm 1); `(1, 0, 1)` also satisfies them
    /// (L1 norm 2) and is exactly what plain row-reduction tends to
    /// produce. Basis Pursuit must find the former.
    #[test]
    fn basis_pursuit_finds_the_genuinely_sparser_solution() {
        let a = vec![row(&[1, 1, 0]), row(&[0, 1, 1])];
        let z = vec![whole(1), whole(1)];
        let x = minimize_l1(&a, &z).expect("feasible");

        let x0 = x.first().cloned().unwrap_or_else(zero);
        let x1 = x.get(1).cloned().unwrap_or_else(zero);
        let x2 = x.get(2).cloned().unwrap_or_else(zero);
        assert_eq!(&x0 + &x1, whole(1), "A*x = z must hold: row 1");
        assert_eq!(&x1 + &x2, whole(1), "A*x = z must hold: row 2");
        assert_eq!(l1_norm(&x), whole(1), "the sparsest solution has L1 norm 1, not 2");
    }
}
