//! K2 — the derivation core: find a witness the kernel was not given.
//!
//! A court's fixed evaluator ([`assay::complex::DeclaredComplex::closes_to`])
//! only ever CHECKS a witness someone hands it. This is the other half:
//! given the universe alone and the boundary a conjecture prescribes,
//! walk the complex's OWN licensed 1-cells until a chain closing onto
//! that boundary turns up, or the budget runs out.
//!
//! This is deliberately not "search" in the generic tree/heuristic
//! sense — there is no open-ended possibility space to explore. The
//! complex already licenses which single steps exist as 1-cells (SQ3):
//! `ops[0]` names, for every 1-cell, the exact pair of 0-cells it
//! connects and which way. What a deriver does is TRAVERSE that
//! already-licensed step-graph — depth-bounded, iterative deepening
//! (one 0-cell memory footprint per branch, not one per frontier node)
//! — never invent, weight, or guess at a step the complex did not
//! already declare.
//!
//! Scoped to the shape every conjecture in this workspace actually
//! poses (SQ1/SQ4): a 1-dimensional witness (`ops[0]` only) closing
//! onto a two-point boundary, `theorem − axiom`. A target that is not
//! exactly one `+1` cell and one `-1` cell refuses by name rather than
//! guessing what a richer boundary would mean.

use std::collections::HashSet;

use assay::complex::DeclaredComplex;
use assay::{whole, Exact};

/// Why a derivation attempt refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivationRefused {
    /// The complex has no dimension-1 cells (no 1-cells to traverse
    /// at all) or no dimension-0 cells to name endpoints in.
    NoStepGraph,
    /// The target is not a plain two-point boundary — exactly one
    /// `+1` cell (the destination) and one `-1` cell (the source).
    /// SQ1's general boundary matching is the court's job; a deriver
    /// only walks a path between two named points.
    NotASimpleBoundary,
    /// One of the target's named cells is outside the complex's
    /// dimension-0 range.
    CellOutOfRange {
        /// The offending cell index.
        cell: u32,
    },
    /// The derivation budget was spent before a chain closing the
    /// target was found — production may be expensive, but it is
    /// never unbounded. Distinct from a court's verification fuel:
    /// this prices FINDING a witness, not checking one.
    BudgetExhausted {
        /// The budget the attempt was given.
        budget: u64,
    },
    /// Every depth up to the complex's own size was exhausted within
    /// budget and no chain closing the target exists — the two named
    /// points are not connected by any licensed path at all.
    NoDerivation,
}

/// One licensed 1-cell, read out of `ops[0]` as a directed edge
/// between two 0-cells: `to − from`, the shape every step this
/// workspace compiles ([`assay::rewrite::Presentation::compile`])
/// produces. A column with any other shape (not exactly one `+1` and
/// one `-1` entry) is not a traversable edge and is simply absent
/// from the step-graph a deriver walks — a richer boundary shape is
/// not this deriver's concern.
fn step_graph(universe: &DeclaredComplex) -> Option<Vec<Vec<(u32, u32)>>> {
    let zero_cells = *universe.cells.first()?;
    let one_cells = *universe.cells.get(1)?;
    let op = universe.ops.first()?;
    let mut by_col: Vec<Vec<(u32, Exact)>> = vec![Vec::new(); one_cells as usize];
    for entry in op {
        by_col.get_mut(entry.col as usize)?.push((entry.row, entry.coeff.clone()));
    }
    let mut adjacency: Vec<Vec<(u32, u32)>> = vec![Vec::new(); zero_cells as usize];
    for (cell, entries) in by_col.iter().enumerate() {
        let cell = u32::try_from(cell).ok()?;
        let [(row_a, coeff_a), (row_b, coeff_b)] = entries.as_slice() else {
            continue;
        };
        let (from, to) = if *coeff_a == whole(-1) && *coeff_b == whole(1) {
            (*row_a, *row_b)
        } else if *coeff_a == whole(1) && *coeff_b == whole(-1) {
            (*row_b, *row_a)
        } else {
            continue;
        };
        adjacency.get_mut(from as usize)?.push((to, cell));
    }
    Some(adjacency)
}

/// The two endpoints a target's boundary names, if it names exactly
/// two: the `-1` cell (source) and the `+1` cell (destination).
fn endpoints(target: &[(u32, Exact)]) -> Result<(u32, u32), DerivationRefused> {
    let [(cell_a, coeff_a), (cell_b, coeff_b)] = target else {
        return Err(DerivationRefused::NotASimpleBoundary);
    };
    if *coeff_a == whole(-1) && *coeff_b == whole(1) {
        Ok((*cell_a, *cell_b))
    } else if *coeff_a == whole(1) && *coeff_b == whole(-1) {
        Ok((*cell_b, *cell_a))
    } else {
        Err(DerivationRefused::NotASimpleBoundary)
    }
}

/// The traversal's own state: the step-graph it walks, where it is
/// headed, and how much of the derivation budget it has spent so far
/// — bundled so a single depth-bounded walk stays a small, ordinary
/// recursive call rather than a wide parameter list.
struct Traversal<'a> {
    adjacency: &'a [Vec<(u32, u32)>],
    destination: u32,
    budget: u64,
    spent: u64,
}

impl Traversal<'_> {
    /// Depth-bounded walk from `at`, extending `path`/`on_path` in
    /// place. Refuses to revisit a 0-cell already on the current
    /// branch — the step-graph a real presentation compiles to can
    /// hold cycles, and a branch that walked back onto itself would
    /// never bottom out.
    fn walk(
        &mut self,
        at: u32,
        depth_left: usize,
        path: &mut Vec<u32>,
        on_path: &mut HashSet<u32>,
    ) -> Result<bool, DerivationRefused> {
        if at == self.destination {
            return Ok(true);
        }
        if depth_left == 0 {
            return Ok(false);
        }
        let Some(edges) = self.adjacency.get(at as usize) else {
            return Ok(false);
        };
        for (to, _cell) in edges {
            if on_path.contains(to) {
                continue;
            }
            self.spent = self.spent.saturating_add(1);
            if self.spent > self.budget {
                return Err(DerivationRefused::BudgetExhausted { budget: self.budget });
            }
            path.push(*to);
            on_path.insert(*to);
            if self.walk(*to, depth_left - 1, path, on_path)? {
                return Ok(true);
            }
            path.pop();
            on_path.remove(to);
        }
        Ok(false)
    }
}

/// The witness 1-chain a word path implies: one `+1` per licensed
/// step traversed, coefficients merged when a step is walked more
/// than once — the same accumulation
/// [`assay::rewrite::Compiled::derive`] does, run here directly
/// against `ops[0]` since a kernel holds only the wire-portable
/// [`DeclaredComplex`], never the poser's word/step tables.
fn witness_along(adjacency: &[Vec<(u32, u32)>], path: &[u32]) -> Vec<(u32, Exact)> {
    let mut acc: std::collections::BTreeMap<u32, Exact> = std::collections::BTreeMap::new();
    for pair in path.windows(2) {
        let (Some(from), Some(to)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let Some(edges) = adjacency.get(*from as usize) else {
            continue;
        };
        let Some((_, cell)) = edges.iter().find(|(t, _)| t == to) else {
            continue;
        };
        let slot = acc.entry(*cell).or_insert_with(|| whole(0));
        *slot += whole(1);
    }
    acc.into_iter().filter(|(_, coeff)| *coeff != whole(0)).collect()
}

/// Find a 1-chain in `universe` closing onto `target`, spending no
/// more than `budget` licensed-step visits.
///
/// Iterative deepening: try every path of length 1, then every path
/// of length 2, and so on, up to the universe's own 0-cell count (no
/// simple path can be longer). Each depth re-walks from empty, so the
/// memory a branch holds is one 0-cell per current depth, never one
/// per node ever seen — the budget bounds work, not memory.
pub fn derive(
    universe: &DeclaredComplex,
    target: &[(u32, Exact)],
    budget: u64,
) -> Result<Vec<(u32, Exact)>, DerivationRefused> {
    let adjacency = step_graph(universe).ok_or(DerivationRefused::NoStepGraph)?;
    let (source, destination) = endpoints(target)?;
    let zero_cells = u32::try_from(adjacency.len()).unwrap_or(u32::MAX);
    if source >= zero_cells {
        return Err(DerivationRefused::CellOutOfRange { cell: source });
    }
    if destination >= zero_cells {
        return Err(DerivationRefused::CellOutOfRange { cell: destination });
    }
    if source == destination {
        return Ok(Vec::new());
    }
    let mut traversal = Traversal {
        adjacency: &adjacency,
        destination,
        budget,
        spent: 0,
    };
    for depth in 1..=adjacency.len() {
        let mut path = vec![source];
        let mut on_path = HashSet::from([source]);
        if traversal.walk(source, depth, &mut path, &mut on_path)? {
            return Ok(witness_along(&adjacency, &path));
        }
    }
    Err(DerivationRefused::NoDerivation)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use assay::complex::Entry;

    /// A hand-built path graph `0 -> 1 -> 2 -> 3`, plus a decoy edge
    /// `1 -> 4` nowhere near the target — proof the traversal only
    /// ever walks declared 1-cells, never invents a shortcut.
    fn path_graph() -> DeclaredComplex {
        let edge = |from: u32, to: u32, col: u32| {
            let mut pair = vec![
                Entry { row: from, col, coeff: whole(-1) },
                Entry { row: to, col, coeff: whole(1) },
            ];
            pair.sort_by_key(|e| (e.col, e.row));
            pair
        };
        let mut op = Vec::new();
        op.extend(edge(0, 1, 0));
        op.extend(edge(1, 2, 1));
        op.extend(edge(2, 3, 2));
        op.extend(edge(1, 4, 3)); // decoy: leads nowhere useful
        DeclaredComplex {
            cells: vec![5, 4],
            ops: vec![op],
        }
    }

    fn boundary(source: u32, destination: u32) -> Vec<(u32, Exact)> {
        let mut target = vec![(source, whole(-1)), (destination, whole(1))];
        target.sort_by_key(|(cell, _)| *cell);
        target
    }

    #[test]
    fn a_three_step_path_derives_within_budget() {
        let universe = path_graph();
        let target = boundary(0, 3);
        let witness = derive(&universe, &target, 100).expect("0->1->2->3 is licensed");
        assert_eq!(witness, vec![(0, whole(1)), (1, whole(1)), (2, whole(1))]);
        universe
            .closes_to(1, &witness, &target, assay::complex::DEFAULT_FUEL)
            .expect("a derived witness closes the boundary it was asked for");
    }

    #[test]
    fn an_exhausted_budget_refuses_by_name_not_by_hanging() {
        let universe = path_graph();
        let target = boundary(0, 3);
        assert_eq!(
            derive(&universe, &target, 1),
            Err(DerivationRefused::BudgetExhausted { budget: 1 })
        );
    }

    #[test]
    fn two_points_with_no_licensed_path_between_them_refuse_by_name() {
        let universe = path_graph();
        let target = boundary(3, 0); // the licensed edges only run forward
        assert_eq!(derive(&universe, &target, 100), Err(DerivationRefused::NoDerivation));
    }

    #[test]
    fn a_target_that_is_not_a_two_point_boundary_refuses() {
        let universe = path_graph();
        let lopsided = vec![(0, whole(-1)), (1, whole(-1)), (2, whole(1))];
        assert_eq!(
            derive(&universe, &lopsided, 100),
            Err(DerivationRefused::NotASimpleBoundary)
        );
    }

    #[test]
    fn the_reference_dihedral_conjecture_derives_from_the_wire_form_alone() {
        // The done-when bar (K2): a calculus this function was never
        // compiled to know. `Compiled::words`/`::steps` are the
        // poser's own build-time scaffolding — thrown away here, so
        // only the wire-portable `DeclaredComplex` + target survive,
        // exactly what an announced conjecture hands a real kernel.
        let compiled = datum::corpus::dihedral_order_6_compiled().expect("confluent, compiles");
        let axiom = compiled.word(b"bab").expect("in the bounded universe");
        let theorem = compiled.word(b"aa").expect("in the bounded universe");
        let target = compiled.target(axiom, theorem).expect("a real dihedral instance");
        let universe = compiled.complex;

        let witness = derive(&universe, &target, 1_000).expect("bab = aa is genuinely derivable");
        universe
            .closes_to(1, &witness, &target, assay::complex::DEFAULT_FUEL)
            .expect("the traversal's own witness closes the prescribed boundary");
    }
}
