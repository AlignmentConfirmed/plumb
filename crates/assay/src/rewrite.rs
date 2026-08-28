//! SQ3 — a rewriting calculus, compiled soundly into a polygraph.
//!
//! ```text
//! 0-cells   the words of the universe (bounded length, length-lex)
//! 1-cells   the LEGAL single-step rewrites — and nothing else
//! ```
//!
//! The soundness bar, met by construction: the compiler emits a
//! 1-cell for every position at which a rule's left side occurs, and
//! for nothing else — so an illegal inference does not get refused,
//! it **fails to exist as a cell**. A derivation from axiom to
//! theorem is then a 1-chain with prescribed boundary
//! `theorem − axiom`, verified by the same fixed evaluator as every
//! other domain (SQ1). Register the compiled universe on the chain
//! (`Act::Declare`) and a court judges derivations it was never
//! compiled to know about.
//!
//! Completeness is bounded and stated: the universe holds every word
//! up to `max_len`. A derivation needing longer intermediate words
//! needs a larger registered universe — a bigger telescope, not a
//! different sky.

use crate::complex::{DeclaredComplex, Entry};
use crate::Exact;

/// A string-rewriting presentation: an alphabet and directed rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    /// The letters, as bytes. Order fixes word enumeration.
    pub alphabet: Vec<u8>,
    /// Directed rules `lhs → rhs`. Direction is the calculus: the
    /// reverse application is not licensed unless stated as its own
    /// rule.
    pub rules: Vec<(Vec<u8>, Vec<u8>)>,
}

/// One licensed rewrite step: which word, by which rule, at which
/// position, to which word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Source word's 0-cell index.
    pub from: usize,
    /// Target word's 0-cell index.
    pub to: usize,
    /// Which rule licensed it.
    pub rule: usize,
    /// The position it applied at.
    pub at: usize,
}

/// A compiled universe: the complex the chain registers, plus the
/// word and step tables a derivation builder navigates by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// What `Act::Declare` publishes and the evaluator judges in.
    pub complex: DeclaredComplex,
    /// 0-cell index → word bytes, length-lex order.
    pub words: Vec<Vec<u8>>,
    /// 1-cell index → the licensed step it is.
    pub steps: Vec<Step>,
}

/// Why a compilation or derivation refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteBroken {
    /// Empty alphabet or a rule with an empty left side — a rule that
    /// matches everywhere licenses nothing meaningful.
    DegeneratePresentation,
    /// The named word is not in the bounded universe.
    NoSuchWord,
    /// Two consecutive words in a derivation path have no licensed
    /// step between them — the inference does not exist as a cell,
    /// which is the soundness bar doing its job.
    NoLicensedStep {
        /// Source 0-cell.
        from: usize,
        /// Claimed target 0-cell.
        to: usize,
    },
}

impl Presentation {
    /// Compile the bounded universe: every word up to `max_len`,
    /// every licensed single-step rewrite between them.
    pub fn compile(&self, max_len: usize) -> Result<Compiled, RewriteBroken> {
        if self.alphabet.is_empty() || self.rules.iter().any(|(lhs, _)| lhs.is_empty()) {
            return Err(RewriteBroken::DegeneratePresentation);
        }
        // Words in length-lex order: deterministic, canonical.
        let mut words: Vec<Vec<u8>> = vec![Vec::new()];
        let mut frontier: Vec<Vec<u8>> = vec![Vec::new()];
        for _ in 0..max_len {
            let mut next = Vec::new();
            for word in &frontier {
                for letter in &self.alphabet {
                    let mut grown = word.clone();
                    grown.push(*letter);
                    next.push(grown);
                }
            }
            words.extend(next.iter().cloned());
            frontier = next;
        }
        let index = |w: &[u8]| words.iter().position(|x| x == w);

        // Steps: one 1-cell per (word, rule, position) at which the
        // rule's left side occurs and the result stays in bounds.
        let mut steps = Vec::new();
        for (from, word) in words.iter().enumerate() {
            for (rule, (lhs, rhs)) in self.rules.iter().enumerate() {
                if lhs.len() > word.len() {
                    continue;
                }
                for at in 0..=word.len().saturating_sub(lhs.len()) {
                    if word.get(at..at.saturating_add(lhs.len())) != Some(lhs.as_slice()) {
                        continue;
                    }
                    let mut rewritten = Vec::with_capacity(
                        word.len().saturating_sub(lhs.len()).saturating_add(rhs.len()),
                    );
                    rewritten.extend_from_slice(word.get(..at).unwrap_or(&[]));
                    rewritten.extend_from_slice(rhs);
                    rewritten
                        .extend_from_slice(word.get(at.saturating_add(lhs.len())..).unwrap_or(&[]));
                    if rewritten.len() > max_len {
                        continue;
                    }
                    let Some(to) = index(&rewritten) else {
                        continue;
                    };
                    steps.push(Step { from, to, rule, at });
                }
            }
        }

        // The boundary operator: ∂(step) = to − from, entries in the
        // canonical (col, row) order the evaluator demands.
        let mut op = Vec::with_capacity(steps.len().saturating_mul(2));
        for (col, step) in steps.iter().enumerate() {
            let col = u32::try_from(col).map_err(|_| RewriteBroken::DegeneratePresentation)?;
            let from = u32::try_from(step.from).map_err(|_| RewriteBroken::DegeneratePresentation)?;
            let to = u32::try_from(step.to).map_err(|_| RewriteBroken::DegeneratePresentation)?;
            let mut pair = vec![
                Entry { row: from, col, coeff: -whole_one() },
                Entry { row: to, col, coeff: whole_one() },
            ];
            pair.sort_by_key(|e| (e.col, e.row));
            op.extend(pair);
        }
        let complex = DeclaredComplex {
            cells: vec![
                u32::try_from(words.len()).map_err(|_| RewriteBroken::DegeneratePresentation)?,
                u32::try_from(steps.len()).map_err(|_| RewriteBroken::DegeneratePresentation)?,
            ],
            ops: vec![op],
        };
        Ok(Compiled {
            complex,
            words,
            steps,
        })
    }
}

fn whole_one() -> Exact {
    crate::whole(1)
}

impl Compiled {
    /// The 0-cell index of a word, if the bounded universe holds it.
    pub fn word(&self, bytes: &[u8]) -> Result<usize, RewriteBroken> {
        self.words
            .iter()
            .position(|w| w == bytes)
            .ok_or(RewriteBroken::NoSuchWord)
    }

    /// A licensed step between two words, if the calculus contains
    /// one. `None` is the soundness bar: the inference is not in the
    /// universe.
    #[must_use]
    pub fn step_between(&self, from: usize, to: usize) -> Option<usize> {
        self.steps.iter().position(|s| s.from == from && s.to == to)
    }

    /// Build the derivation witness along a word path: one licensed
    /// step per consecutive pair, coefficients merged canonically.
    /// A pair with no licensed step refuses by naming it.
    pub fn derive(&self, path: &[usize]) -> Result<Vec<(u32, Exact)>, RewriteBroken> {
        let mut acc: std::collections::BTreeMap<u32, Exact> = std::collections::BTreeMap::new();
        for pair in path.windows(2) {
            let (from, to) = match (pair.first(), pair.get(1)) {
                (Some(f), Some(t)) => (*f, *t),
                _ => continue,
            };
            let step = self
                .step_between(from, to)
                .ok_or(RewriteBroken::NoLicensedStep { from, to })?;
            let cell = u32::try_from(step).map_err(|_| RewriteBroken::NoSuchWord)?;
            let slot = acc.entry(cell).or_insert_with(|| crate::whole(0));
            *slot += whole_one();
        }
        Ok(acc
            .into_iter()
            .filter(|(_, coeff)| !num_traits::Zero::is_zero(coeff))
            .collect())
    }

    /// The prescribed boundary of a derivation: `theorem − axiom`,
    /// canonical.
    pub fn target(&self, axiom: usize, theorem: usize) -> Result<Vec<(u32, Exact)>, RewriteBroken> {
        let axiom = u32::try_from(axiom).map_err(|_| RewriteBroken::NoSuchWord)?;
        let theorem = u32::try_from(theorem).map_err(|_| RewriteBroken::NoSuchWord)?;
        let mut target = vec![(axiom, -whole_one()), (theorem, whole_one())];
        target.sort_by_key(|(cell, _)| *cell);
        Ok(target)
    }
}

impl Compiled {
    /// SQ6 — grow the confluence dimension: one 2-cell per **diamond**
    /// (a word with two distinct one-step successors that rejoin in
    /// one step), with boundary `left path − right path`. Squier's
    /// observation, made byte-checkable: the critical branchings ARE
    /// cells, and "two derivations of one lemma commute" is exhibited
    /// by the diamond that fills them — verified as ∂∂ = 0 one
    /// dimension up, by the same evaluator as everything else.
    pub fn with_confluences(mut self) -> Result<Compiled, RewriteBroken> {
        let mut squares: Vec<[usize; 4]> = Vec::new();
        for (left1, s_left) in self.steps.iter().enumerate() {
            for (right1, s_right) in self.steps.iter().enumerate() {
                if left1 >= right1 || s_left.from != s_right.from || s_left.to == s_right.to {
                    continue;
                }
                // A one-step rejoin from both branch tips?
                let join = self.steps.iter().enumerate().find_map(|(left2, a)| {
                    if a.from != s_left.to {
                        return None;
                    }
                    self.steps
                        .iter()
                        .position(|b| b.from == s_right.to && b.to == a.to)
                        .map(|right2| (left2, right2))
                });
                if let Some((left2, right2)) = join {
                    squares.push([left1, left2, right1, right2]);
                }
            }
        }
        let mut op2 = Vec::with_capacity(squares.len().saturating_mul(4));
        for (col, [l1, l2, r1, r2]) in squares.iter().enumerate() {
            let col = u32::try_from(col).map_err(|_| RewriteBroken::DegeneratePresentation)?;
            // ∂(square) = (left1 + left2) − (right1 + right2); a step
            // shared by both paths cancels.
            let mut acc: std::collections::BTreeMap<u32, Exact> = std::collections::BTreeMap::new();
            for (step, sign) in [(l1, 1i64), (l2, 1), (r1, -1), (r2, -1)] {
                let row = u32::try_from(*step).map_err(|_| RewriteBroken::DegeneratePresentation)?;
                let slot = acc.entry(row).or_insert_with(|| crate::whole(0));
                *slot += crate::whole(sign);
            }
            for (row, coeff) in acc {
                if !num_traits::Zero::is_zero(&coeff) {
                    op2.push(Entry { row, col, coeff });
                }
            }
        }
        let count = u32::try_from(squares.len()).map_err(|_| RewriteBroken::DegeneratePresentation)?;
        self.complex.cells.push(count);
        self.complex.ops.push(op2);
        Ok(self)
    }
}
