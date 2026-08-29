//! K2 — the derivation core: find a witness the kernel was not given.
//!
//! A court's fixed evaluator ([`assay::complex::DeclaredComplex::closes_to`])
//! only ever checks a witness someone hands it. This is the other half:
//! given the universe alone and the boundary a conjecture prescribes,
//! walk the complex's own licensed 1-cells until a chain closing onto
//! that boundary turns up, or the budget runs out.
//!
//! This is deliberately not "search" in the generic tree/heuristic
//! sense — there is no open-ended possibility space to explore. The
//! complex already licenses which single steps exist as 1-cells (SQ3):
//! `ops[0]` names, for every 1-cell, the exact pair of 0-cells it
//! connects and which way. What a deriver does is traverse that
//! already-licensed step-graph — never invent, weight, or guess at a
//! step the complex did not already declare.
//!
//! Scoped to the shape every conjecture in this workspace actually
//! poses (SQ1/SQ4): a 1-dimensional witness (`ops[0]` only) closing
//! onto a two-point boundary, `theorem − axiom`. A target that is not
//! exactly one `+1` cell and one `-1` cell refuses by name rather than
//! guessing what a richer boundary would mean.
//!
//! **Breadth-first, not iterative deepening.** A single source/sink
//! reachability-and-shortest-path query over a directed graph is
//! already polynomial (`O(V+E)` via BFS) — nothing about this scope
//! is exponential, so no depth-bounded/iterative-deepening machinery
//! was ever necessary here. An earlier revision reached for iterative
//! deepening anyway; that was solving an already-easy problem with
//! the wrong tool, not a limit of the domain. Multi-term boundaries or
//! non-graph (dim≥2) complexes are where the real escape from search
//! lives — linear algebra over `∂` (Smith Normal Form / min-cost
//! flow), tracked separately, not here.

use std::collections::VecDeque;

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
    /// this prices finding a witness, not checking one.
    BudgetExhausted {
        /// The budget the attempt was given.
        budget: u64,
    },
    /// Every 0-cell reachable within budget was visited and the
    /// destination was never among them — the two named points are
    /// not connected by any licensed path at all.
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

/// Breadth-first from `source` toward `destination`, one visit per
/// 0-cell — a global visited set rules out cycles for free, unlike a
/// depth-bounded walk that has to track and unwind a per-branch path.
/// Returns the 0-cell path (`source..=destination` inclusive) the
/// instant `destination` is discovered — BFS's own guarantee that
/// this is a *shortest* licensed path, not merely *a* path.
fn breadth_first(
    adjacency: &[Vec<(u32, u32)>],
    source: u32,
    destination: u32,
    budget: u64,
) -> Result<Vec<u32>, DerivationRefused> {
    let mut visited = vec![false; adjacency.len()];
    let mut parent: Vec<Option<u32>> = vec![None; adjacency.len()];
    if let Some(slot) = visited.get_mut(source as usize) {
        *slot = true;
    }
    let mut frontier = VecDeque::from([source]);
    let mut spent = 0u64;
    while let Some(at) = frontier.pop_front() {
        let Some(edges) = adjacency.get(at as usize) else {
            continue;
        };
        for (to, _cell) in edges {
            if visited.get(*to as usize).copied().unwrap_or(true) {
                continue;
            }
            spent = spent.saturating_add(1);
            if spent > budget {
                return Err(DerivationRefused::BudgetExhausted { budget });
            }
            if let Some(slot) = visited.get_mut(*to as usize) {
                *slot = true;
            }
            if let Some(slot) = parent.get_mut(*to as usize) {
                *slot = Some(at);
            }
            if *to == destination {
                return Ok(retrace(&parent, source, destination));
            }
            frontier.push_back(*to);
        }
    }
    Err(DerivationRefused::NoDerivation)
}

/// Walk BFS's parent pointers back from `destination` to `source`,
/// then reverse — the path in travel order.
fn retrace(parent: &[Option<u32>], source: u32, destination: u32) -> Vec<u32> {
    let mut path = vec![destination];
    let mut at = destination;
    while at != source {
        let Some(before) = parent.get(at as usize).copied().flatten() else {
            break;
        };
        path.push(before);
        at = before;
    }
    path.reverse();
    path
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
/// Breadth-first: every 0-cell within one step, then two, and so on,
/// stopping the instant the destination is discovered — one pass,
/// `O(V+E)` bounded by `budget`, no re-exploration of shallower depths.
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
    let path = breadth_first(&adjacency, source, destination, budget)?;
    Ok(witness_along(&adjacency, &path))
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

    /// The decoy edge `1 -> 4` is a dead end nowhere near the target
    /// — BFS must never let it show up in the returned witness, and
    /// must still find the direct path through node 1 toward 3.
    #[test]
    fn a_decoy_branch_never_leaks_into_the_witness() {
        let universe = path_graph();
        let target = boundary(0, 3);
        let witness = derive(&universe, &target, 100).expect("0->1->2->3 is licensed");
        let decoy_cell = 3u32; // the 1->4 edge's column, per path_graph()
        assert!(
            !witness.iter().any(|(cell, _)| *cell == decoy_cell),
            "witness must never cite a step that does not lead to the target"
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
