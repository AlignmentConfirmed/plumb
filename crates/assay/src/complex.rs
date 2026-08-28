//! UC1–UC2 — the declared complex, and the fixed evaluator.
//!
//! **The engine is the invariant; the geometry is data.** A complex
//! arrives as cell counts and sparse boundary operators with exact
//! rational coefficients — no `struct Hexagon` exists anywhere. The
//! evaluator checks two things and knows nothing else:
//!
//! 1. the declared operators compose to zero (`∂∘∂ = 0`) — the
//!    declaration *is* a chain complex, not just a matrix pile;
//! 2. a submitted witness chain **closes**: `∂c = 0`, exactly.
//!
//! Every evaluation is **fuel-bounded**: an axiom pack is code by
//! another name, and this is where that name is priced. Exhaustion is
//! a named refusal, never a hang — the evaluator is total.
//!
//! Canonical form is enforced at decode: entries sorted, coefficients
//! nonzero, no duplicates. One structure, one byte string — which is
//! what lets `work_id` be content-addressed and lets two parties agree
//! a hexagon is not a five-simplex by comparing bytes.

use crate::exact_codec::{self, ExactBroken};
use crate::Exact;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

/// Domain byte for declared-complex claims. Boundary is 1, Shape is 2.
pub const DOMAIN_DECLARED: u8 = 3;

/// Domain byte for proof claims (SQ): prescribed boundary + cited
/// lemmas. Deduction as boundary annihilation.
pub const DOMAIN_PROOF: u8 = 4;

/// Fuel the reward path evaluates under when no deployment says
/// otherwise. A *default*, not a constant of nature — UC6 prices
/// fuel on the board per-space.
pub const DEFAULT_FUEL: u64 = 1_000_000;

/// One sparse entry of a boundary operator: the coefficient of
/// `row` (a k-cell) in the boundary of `col` (a (k+1)-cell).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Target cell in dimension k.
    pub row: u32,
    /// Source cell in dimension k+1.
    pub col: u32,
    /// Exact coefficient, never zero in canonical form.
    pub coeff: Exact,
}

/// A chain complex, declared as data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredComplex {
    /// Cells per dimension: `cells[k]` many k-cells, k = 0..=K.
    pub cells: Vec<u32>,
    /// `ops[k]` is `∂_{k+1}`: entries mapping (k+1)-cells to k-cells.
    /// `ops.len() == cells.len() - 1`.
    pub ops: Vec<Vec<Entry>>,
}

/// Why a declared complex or a witness refused. Named, total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComplexBroken {
    /// Bytes ended before the structure did.
    Truncated,
    /// Bytes continued after the structure ended.
    Trailing,
    /// The domain byte was not [`DOMAIN_DECLARED`].
    Domain(u8),
    /// No cells at all — an empty universe declares nothing.
    Empty,
    /// Operator count disagrees with dimension count.
    OperatorShape,
    /// An entry names a cell outside its dimension's count.
    CellOutOfRange {
        /// Which operator (maps dimension k+1 to k).
        op: u32,
    },
    /// Entries out of order, duplicated, or zero-coefficient: the
    /// same structure must be the same bytes.
    NotCanonical,
    /// The declared operators do not compose to zero: this is not a
    /// chain complex, and nothing over it means anything.
    NotAComplex {
        /// Dimension where `∂∘∂ ≠ 0` was found.
        dim: u32,
    },
    /// The witness names a dimension the complex does not have.
    NoSuchDimension,
    /// The witness chain's boundary does not vanish: the claim does
    /// not close.
    OpenBoundary {
        /// A cell with nonzero net boundary flux.
        cell: u32,
    },
    /// The witness chain's boundary does not equal the PRESCRIBED
    /// target (SQ1): a missing premise or a dangling conclusion, and
    /// the refusal names where.
    BoundaryMismatch {
        /// The cell where computed flux and target disagree.
        cell: u32,
    },
    /// The evaluation budget ran out. A refusal, never a hang — and
    /// it names the budget, because an over-budget evaluation is
    /// refused *at a price* (UC6), not at a mystery.
    FuelExhausted {
        /// The budget the evaluation was given.
        budget: u64,
    },
    /// An exact coefficient failed to decode.
    Exact(ExactBroken),
}

impl From<ExactBroken> for ComplexBroken {
    fn from(e: ExactBroken) -> Self {
        match e {
            ExactBroken::Truncated => ComplexBroken::Truncated,
            other => ComplexBroken::Exact(other),
        }
    }
}

/// The evaluation budget: one unit per multiply-accumulate or
/// canonical-order comparison. Charging is infallible; spending past
/// the budget refuses.
struct Fuel {
    budget: u64,
    left: u64,
}

impl Fuel {
    fn new(budget: u64) -> Self {
        Self { budget, left: budget }
    }

    fn spend(&mut self, units: u64) -> Result<(), ComplexBroken> {
        match self.left.checked_sub(units) {
            Some(left) => {
                self.left = left;
                Ok(())
            }
            None => Err(ComplexBroken::FuelExhausted {
                budget: self.budget,
            }),
        }
    }

    fn spent(&self) -> u64 {
        self.budget - self.left
    }
}

impl DeclaredComplex {
    /// Admit the declaration: shape, canonicality, and `∂∘∂ = 0`,
    /// all under `fuel`. Returns the fuel **spent** — the price of
    /// having checked (UC6).
    pub fn admit(&self, fuel: u64) -> Result<u64, ComplexBroken> {
        let mut fuel = Fuel::new(fuel);
        if self.cells.is_empty() || self.cells.iter().all(|c| *c == 0) {
            return Err(ComplexBroken::Empty);
        }
        if self.ops.len() + 1 != self.cells.len() {
            return Err(ComplexBroken::OperatorShape);
        }
        for (k, op) in self.ops.iter().enumerate() {
            let rows = self.cells.get(k).copied().unwrap_or(0);
            let cols = self.cells.get(k + 1).copied().unwrap_or(0);
            let mut previous: Option<(u32, u32)> = None;
            for entry in op {
                fuel.spend(1)?;
                if entry.row >= rows || entry.col >= cols {
                    return Err(ComplexBroken::CellOutOfRange { op: k as u32 });
                }
                if entry.coeff.is_zero() {
                    return Err(ComplexBroken::NotCanonical);
                }
                let key = (entry.col, entry.row);
                if previous.is_some_and(|p| p >= key) {
                    return Err(ComplexBroken::NotCanonical);
                }
                previous = Some(key);
            }
        }
        // ∂∘∂ = 0: for consecutive operators, every (k+2)-cell's
        // double boundary must vanish on every k-cell.
        for k in 0..self.ops.len().saturating_sub(1) {
            let lower = self.ops.get(k).map(Vec::as_slice).unwrap_or(&[]);
            let upper = self.ops.get(k + 1).map(Vec::as_slice).unwrap_or(&[]);
            // Accumulate coefficient of (row_low, col_up) products.
            let mut acc: std::collections::BTreeMap<(u32, u32), Exact> =
                std::collections::BTreeMap::new();
            for up in upper {
                for low in lower {
                    if low.col == up.row {
                        fuel.spend(1)?;
                        let slot = acc
                            .entry((low.row, up.col))
                            .or_insert_with(|| Exact::from_integer(0.into()));
                        *slot += low.coeff.clone() * up.coeff.clone();
                    }
                }
            }
            if acc.values().any(|v| !v.is_zero()) {
                return Err(ComplexBroken::NotAComplex { dim: k as u32 });
            }
        }
        Ok(fuel.spent())
    }

    /// The definition bytes a chain publishes (UC4): the complex
    /// alone, canonically — no transport, no witness. What
    /// `Act::Declare` carries and a resolver compares against.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let dims = u32::try_from(self.cells.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&dims.to_le_bytes());
        for count in &self.cells {
            out.extend_from_slice(&count.to_le_bytes());
        }
        for op in &self.ops {
            let n = u32::try_from(op.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&n.to_le_bytes());
            for entry in op {
                out.extend_from_slice(&entry.row.to_le_bytes());
                out.extend_from_slice(&entry.col.to_le_bytes());
                exact_codec::put_exact(&entry.coeff, &mut out);
            }
        }
        out
    }

    /// Read definition bytes back. Canonicality and complex-hood are
    /// [`DeclaredComplex::admit`]'s answer, not this codec's.
    pub fn decode(bytes: &[u8]) -> Result<Self, ComplexBroken> {
        let mut at = 0usize;
        let dims = exact_codec::take_u32(bytes, &mut at)? as usize;
        if dims == 0 || dims > 4096 {
            return Err(ComplexBroken::Empty);
        }
        let mut cells = Vec::with_capacity(dims);
        for _ in 0..dims {
            cells.push(exact_codec::take_u32(bytes, &mut at)?);
        }
        let mut ops = Vec::with_capacity(dims.saturating_sub(1));
        for _ in 0..dims.saturating_sub(1) {
            let n = exact_codec::take_u32(bytes, &mut at)? as usize;
            let mut op = Vec::with_capacity(n.min(1 << 16));
            for _ in 0..n {
                let row = exact_codec::take_u32(bytes, &mut at)?;
                let col = exact_codec::take_u32(bytes, &mut at)?;
                let coeff = exact_codec::take_exact(bytes, &mut at)?;
                op.push(Entry { row, col, coeff });
            }
            ops.push(op);
        }
        if at != bytes.len() {
            return Err(ComplexBroken::Trailing);
        }
        Ok(Self { cells, ops })
    }

    /// SQ1 — does a witness chain have exactly the **prescribed**
    /// boundary: `∂c = z`?
    ///
    /// This is the proof shape: `z = target − axioms`, and a chain
    /// closing onto it is watertight — no missing premises (flux the
    /// target demands but the witness lacks) and no dangling
    /// conclusions (flux the witness produces but the target does not
    /// name). `z = ∅` recovers plain closure. The mismatch refusal
    /// names the offending cell.
    pub fn closes_to(
        &self,
        dim: u32,
        witness: &[(u32, Exact)],
        target: &[(u32, Exact)],
        fuel: u64,
    ) -> Result<u64, ComplexBroken> {
        let mut fuel = Fuel::new(fuel);
        let dim = dim as usize;
        let count = self
            .cells
            .get(dim)
            .copied()
            .ok_or(ComplexBroken::NoSuchDimension)?;
        if witness.is_empty() {
            return Err(ComplexBroken::Empty);
        }
        let mut previous: Option<u32> = None;
        for (cell, coeff) in witness {
            fuel.spend(1)?;
            if *cell >= count {
                return Err(ComplexBroken::CellOutOfRange { op: dim as u32 });
            }
            if coeff.is_zero() || previous.is_some_and(|p| p >= *cell) {
                return Err(ComplexBroken::NotCanonical);
            }
            previous = Some(*cell);
        }
        // The target lives one dimension down, canonical like the
        // witness (a target that is not one byte string per meaning
        // would break content addressing).
        let below = if dim == 0 {
            0
        } else {
            self.cells.get(dim - 1).copied().unwrap_or(0)
        };
        let mut previous: Option<u32> = None;
        for (cell, coeff) in target {
            fuel.spend(1)?;
            if dim == 0 || *cell >= below {
                return Err(ComplexBroken::CellOutOfRange { op: dim as u32 });
            }
            if coeff.is_zero() || previous.is_some_and(|p| p >= *cell) {
                return Err(ComplexBroken::NotCanonical);
            }
            previous = Some(*cell);
        }
        if dim == 0 {
            // A 0-chain has no boundary; only the empty target matches.
            return Ok(fuel.spent());
        }
        let op = self
            .ops
            .get(dim - 1)
            .map(Vec::as_slice)
            .ok_or(ComplexBroken::NoSuchDimension)?;
        let mut flux: std::collections::BTreeMap<u32, Exact> =
            std::collections::BTreeMap::new();
        for (cell, coeff) in witness {
            for entry in op {
                if entry.col == *cell {
                    fuel.spend(1)?;
                    let slot = flux
                        .entry(entry.row)
                        .or_insert_with(|| Exact::from_integer(0.into()));
                    *slot += entry.coeff.clone() * coeff.clone();
                }
            }
        }
        for (cell, coeff) in target {
            fuel.spend(1)?;
            let slot = flux
                .entry(*cell)
                .or_insert_with(|| Exact::from_integer(0.into()));
            *slot -= coeff.clone();
        }
        if let Some((cell, _)) = flux.iter().find(|(_, v)| !v.is_zero()) {
            return Err(if target.is_empty() {
                ComplexBroken::OpenBoundary { cell: *cell }
            } else {
                ComplexBroken::BoundaryMismatch { cell: *cell }
            });
        }
        Ok(fuel.spent())
    }

    /// Does a witness chain in dimension `dim` **close**: `∂c = 0`?
    ///
    /// The witness is sparse `(cell, coeff)` pairs, canonical (sorted
    /// by cell, nonzero coefficients). A 0-chain closes trivially —
    /// its boundary lands in nothing.
    pub fn closes(
        &self,
        dim: u32,
        witness: &[(u32, Exact)],
        fuel: u64,
    ) -> Result<u64, ComplexBroken> {
        let mut fuel = Fuel::new(fuel);
        let dim = dim as usize;
        let count = self
            .cells
            .get(dim)
            .copied()
            .ok_or(ComplexBroken::NoSuchDimension)?;
        if witness.is_empty() {
            return Err(ComplexBroken::Empty);
        }
        let mut previous: Option<u32> = None;
        for (cell, coeff) in witness {
            fuel.spend(1)?;
            if *cell >= count {
                return Err(ComplexBroken::CellOutOfRange { op: dim as u32 });
            }
            if coeff.is_zero() || previous.is_some_and(|p| p >= *cell) {
                return Err(ComplexBroken::NotCanonical);
            }
            previous = Some(*cell);
        }
        if dim == 0 {
            return Ok(fuel.spent()); // a 0-chain has no boundary to open
        }
        let op = self
            .ops
            .get(dim - 1)
            .map(Vec::as_slice)
            .ok_or(ComplexBroken::NoSuchDimension)?;
        let mut flux: std::collections::BTreeMap<u32, Exact> =
            std::collections::BTreeMap::new();
        for (cell, coeff) in witness {
            for entry in op {
                if entry.col == *cell {
                    fuel.spend(1)?;
                    let slot = flux
                        .entry(entry.row)
                        .or_insert_with(|| Exact::from_integer(0.into()));
                    *slot += entry.coeff.clone() * coeff.clone();
                }
            }
        }
        if let Some((cell, _)) = flux.iter().find(|(_, v)| !v.is_zero()) {
            return Err(ComplexBroken::OpenBoundary { cell: *cell });
        }
        Ok(fuel.spent())
    }
}

/// Why [`DeclaredComplex::solve`] could not construct a witness.
///
/// Distinct from [`ComplexBroken`] on purpose: that enum names why a
/// GIVEN witness failed to verify; this one names why no witness was
/// found in the first place — a different operation with different
/// failure modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveRefused {
    /// `dim` names a dimension this complex does not have a boundary
    /// operator for.
    NoSuchDimension,
    /// No integral chain closes onto this target — solvable over ℚ
    /// is not the same question, and [`crate::snf`]'s whole point is
    /// that gap.
    NoIntegralSolution,
}

impl DeclaredComplex {
    /// Find a `dim`-chain closing onto `target` directly, via linear
    /// algebra over `∂_dim` — never by walking anything.
    ///
    /// Scales `∂_dim` and `target` by the LCM of every denominator
    /// present (an equation `Ax = z` has the identical solution set
    /// as `(kA)x = (kz)` for any nonzero `k`, so this changes nothing
    /// about which `x` exist — only makes them integer-solvable via
    /// [`crate::snf`]), then decides integral solvability directly.
    ///
    /// This is producer-side, not verifier-side: whatever it returns
    /// still has to pass [`DeclaredComplex::closes_to`] like any other
    /// witness — a bug here produces a rejected claim, never a wrongly
    /// accepted one.
    ///
    /// See [`crate::snf`]'s module docs for the real gap this doesn't
    /// close: an integral solution may use a licensed cell with a
    /// NEGATIVE coefficient, which is not the same thing as a
    /// legitimate forward derivation using that cell.
    pub fn solve(&self, dim: u32, target: &[(u32, Exact)]) -> Result<Vec<(u32, Exact)>, SolveRefused> {
        let dim = dim as usize;
        if dim == 0 {
            return Err(SolveRefused::NoSuchDimension);
        }
        let rows = self.cells.get(dim - 1).copied().ok_or(SolveRefused::NoSuchDimension)?;
        let cols = self.cells.get(dim).copied().ok_or(SolveRefused::NoSuchDimension)?;
        let op = self.ops.get(dim - 1).ok_or(SolveRefused::NoSuchDimension)?;

        let one = BigInt::from(1);
        let mut scale = one.clone();
        for entry in op {
            scale = lcm(&scale, entry.coeff.denom());
        }
        for (_, coeff) in target {
            scale = lcm(&scale, coeff.denom());
        }

        let mut a = crate::snf::Matrix::zeros(rows as usize, cols as usize);
        for entry in op {
            let scaled = entry.coeff.numer() * (&scale / entry.coeff.denom());
            let existing = a.get(entry.row as usize, entry.col as usize);
            a.set(entry.row as usize, entry.col as usize, existing + scaled);
        }
        let mut z = vec![BigInt::from(0); rows as usize];
        for (cell, coeff) in target {
            let scaled = coeff.numer() * (&scale / coeff.denom());
            if let Some(slot) = z.get_mut(*cell as usize) {
                *slot += scaled;
            }
        }

        let x = crate::snf::solve_integer(&a, &z).ok_or(SolveRefused::NoIntegralSolution)?;
        Ok(x
            .into_iter()
            .enumerate()
            .filter(|(_, coeff)| !coeff.is_zero())
            .map(|(cell, coeff)| (cell as u32, Exact::from_integer(coeff)))
            .collect())
    }
}

fn lcm(a: &BigInt, b: &BigInt) -> BigInt {
    if a.is_zero() || b.is_zero() {
        return BigInt::from(0);
    }
    let g = gcd(a, b);
    (a / g) * b
}

fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    let (mut a, mut b) = (a.clone(), b.clone());
    while !b.is_zero() {
        let r = &a % &b;
        a = b;
        b = r;
    }
    if a.is_negative() {
        -a
    } else {
        a
    }
}

/// A declared-domain claim: the complex, the witness, the closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredClaim {
    /// **Transport only** — not the credit key.
    pub transport: u64,
    /// The universe this claim closes in, declared as data.
    pub complex: DeclaredComplex,
    /// The dimension the witness lives in.
    pub dim: u32,
    /// The chain claimed to close: sparse, canonical.
    pub witness: Vec<(u32, Exact)>,
}

impl DeclaredClaim {
    /// Verify by re-derivation under `fuel`: the declaration is a
    /// complex, and the witness closes in it. Returns total fuel
    /// spent — what a board prices (UC6).
    pub fn verify(&self, fuel: u64) -> Result<u64, ComplexBroken> {
        let spent = self.complex.admit(fuel)?;
        let remaining = fuel.saturating_sub(spent);
        let more = self.complex.closes(self.dim, &self.witness, remaining)?;
        Ok(spent.saturating_add(more))
    }

    /// **Primary identity for credit:** structure only, transport
    /// zeroed. Canonical encoding makes this content-addressed.
    pub fn work_id(&self) -> crate::work::WorkId {
        crate::work::WorkId::from_bytes(self.encode_with_transport(0))
    }

    /// Multi-axial credit: one component per dimension of the
    /// declared complex — the breadth of the space the closure was
    /// verified in. Empty (no credit) unless the claim verifies.
    pub fn credit_axes(&self) -> Vec<u128> {
        if self.verify(DEFAULT_FUEL).is_err() {
            return Vec::new();
        }
        self.cells_as_credit()
    }

    fn cells_as_credit(&self) -> Vec<u128> {
        self.complex
            .cells
            .iter()
            .map(|c| u128::from(*c))
            .collect()
    }

    /// Wire body, domain-tagged.
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_transport(self.transport)
    }

    fn encode_with_transport(&self, transport: u64) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(DOMAIN_DECLARED);
        out.extend_from_slice(&transport.to_le_bytes());
        let dims = u32::try_from(self.complex.cells.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&dims.to_le_bytes());
        for count in &self.complex.cells {
            out.extend_from_slice(&count.to_le_bytes());
        }
        for op in &self.complex.ops {
            let n = u32::try_from(op.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&n.to_le_bytes());
            for entry in op {
                out.extend_from_slice(&entry.row.to_le_bytes());
                out.extend_from_slice(&entry.col.to_le_bytes());
                exact_codec::put_exact(&entry.coeff, &mut out);
            }
        }
        out.extend_from_slice(&self.dim.to_le_bytes());
        let n = u32::try_from(self.witness.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for (cell, coeff) in &self.witness {
            out.extend_from_slice(&cell.to_le_bytes());
            exact_codec::put_exact(coeff, &mut out);
        }
        out
    }

    /// Decode a domain-tagged declared body. Canonicality is enforced
    /// by [`DeclaredClaim::verify`], not re-checked here — decode
    /// answers "is this the format", verify answers "is this true".
    pub fn decode(bytes: &[u8]) -> Result<Self, ComplexBroken> {
        let mut at = 0usize;
        let domain = exact_codec::take_u8(bytes, &mut at)?;
        if domain != DOMAIN_DECLARED {
            return Err(ComplexBroken::Domain(domain));
        }
        let transport = exact_codec::take_u64(bytes, &mut at)?;
        let dims = exact_codec::take_u32(bytes, &mut at)? as usize;
        if dims == 0 || dims > 4096 {
            return Err(ComplexBroken::Empty);
        }
        let mut cells = Vec::with_capacity(dims);
        for _ in 0..dims {
            cells.push(exact_codec::take_u32(bytes, &mut at)?);
        }
        let mut ops = Vec::with_capacity(dims.saturating_sub(1));
        for _ in 0..dims.saturating_sub(1) {
            let n = exact_codec::take_u32(bytes, &mut at)? as usize;
            let mut op = Vec::with_capacity(n.min(1 << 16));
            for _ in 0..n {
                let row = exact_codec::take_u32(bytes, &mut at)?;
                let col = exact_codec::take_u32(bytes, &mut at)?;
                let coeff = exact_codec::take_exact(bytes, &mut at)?;
                op.push(Entry { row, col, coeff });
            }
            ops.push(op);
        }
        let dim = exact_codec::take_u32(bytes, &mut at)?;
        let n = exact_codec::take_u32(bytes, &mut at)? as usize;
        let mut witness = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            let cell = exact_codec::take_u32(bytes, &mut at)?;
            let coeff = exact_codec::take_exact(bytes, &mut at)?;
            witness.push((cell, coeff));
        }
        if at != bytes.len() {
            return Err(ComplexBroken::Trailing);
        }
        Ok(Self {
            transport,
            complex: DeclaredComplex { cells, ops },
            dim,
            witness,
        })
    }
}

/// UC5 — Shape, re-expressed as a declared complex.
///
/// The shape vocabulary (an undirected simple charged graph) is a
/// constrained sub-language of declared complexes; its constraints
/// live in this translation, and everything else is checked by the
/// same fixed evaluator that checks every domain. The witness is the
/// all-orbs 0-chain — Shape's law is structural admission, not
/// closure, and a 0-chain closes trivially, so the verdict is
/// exactly the declaration's admissibility.
///
/// Refusals reuse the complex's own names: a zero charge is a
/// non-canonical coefficient, a duplicate edge is a non-canonical
/// column, an out-of-range orb is an out-of-range cell.
pub fn from_shape(shape: &crate::shape::Shape) -> Result<DeclaredClaim, ComplexBroken> {
    let orbs = shape.orbs();
    let edges = shape.edges();
    if orbs == 0 || edges.is_empty() {
        return Err(ComplexBroken::Empty);
    }
    let mut op = Vec::with_capacity(edges.len() * 2);
    let mut previous: Option<(u32, u32)> = None;
    for (k, edge) in edges.iter().enumerate() {
        let col = u32::try_from(k).map_err(|_| ComplexBroken::OperatorShape)?;
        if edge.i >= orbs || edge.j >= orbs || edge.i >= edge.j {
            return Err(ComplexBroken::CellOutOfRange { op: 0 });
        }
        if edge.charge.is_zero() {
            return Err(ComplexBroken::NotCanonical);
        }
        // A duplicate (i, j) pair is the same column signature twice —
        // one structure, one byte string, so it refuses here.
        if previous.is_some_and(|p| p == (edge.i, edge.j)) {
            return Err(ComplexBroken::NotCanonical);
        }
        previous = Some((edge.i, edge.j));
        // ∂(edge) = j − i, weighted by the charge. Canonical order
        // within the column: row i < row j.
        op.push(Entry {
            row: edge.i,
            col,
            coeff: -edge.charge.clone(),
        });
        op.push(Entry {
            row: edge.j,
            col,
            coeff: edge.charge.clone(),
        });
    }
    let count = u32::try_from(edges.len()).map_err(|_| ComplexBroken::OperatorShape)?;
    let witness = (0..orbs)
        .map(|orb| (orb, Exact::from_integer(1.into())))
        .collect();
    Ok(DeclaredClaim {
        transport: 0,
        complex: DeclaredComplex {
            cells: vec![orbs, count],
            ops: vec![op],
        },
        dim: 0,
        witness,
    })
}

/// SQ — a proof claim: a chain with a **prescribed** boundary, plus
/// cited lemmas.
///
/// The proof shape: `∂(witness) = target`, where the target encodes
/// `theorem − axioms` in the registered calculus. Dependencies are
/// `work_id`s of previously settled claims, cited as trusted
/// infrastructure — **this leaf does not check them**, because the
/// ledger is the court's; `RewardBook` refuses a proof citing any
/// work its book has not settled, and never re-pays cited compute
/// (the T2 memoization cache, spent as a cache).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofClaim {
    /// **Transport only** — not the credit key.
    pub transport: u64,
    /// The universe the proof lives in.
    pub complex: DeclaredComplex,
    /// The dimension the witness lives in.
    pub dim: u32,
    /// The prescribed boundary: `theorem − axioms`, canonical.
    pub target: Vec<(u32, Exact)>,
    /// The derivation chain claimed to close onto the target.
    pub witness: Vec<(u32, Exact)>,
    /// Settled lemmas this proof stands on, by content address.
    pub deps: Vec<Vec<u8>>,
}

impl ProofClaim {
    /// Verify by re-derivation under `fuel`: the universe admits and
    /// the witness closes **onto the target** (SQ1). Cited lemmas
    /// cost nothing here — the court's book answers for them.
    pub fn verify(&self, fuel: u64) -> Result<u64, ComplexBroken> {
        let spent = self.complex.admit(fuel)?;
        let remaining = fuel.saturating_sub(spent);
        let more = self
            .complex
            .closes_to(self.dim, &self.witness, &self.target, remaining)?;
        Ok(spent.saturating_add(more))
    }

    /// **Primary identity:** structure only, transport zeroed. The
    /// citations are part of the structure — the same derivation
    /// standing on different lemmas is a different proof.
    pub fn work_id(&self) -> crate::work::WorkId {
        crate::work::WorkId::from_bytes(self.encode_with_transport(0))
    }

    /// Multi-axial credit: the breadth of the universe the proof was
    /// verified in, one component per dimension. Empty unless it
    /// verifies.
    pub fn credit_axes(&self) -> Vec<u128> {
        if self.verify(DEFAULT_FUEL).is_err() {
            return Vec::new();
        }
        self.complex.cells.iter().map(|c| u128::from(*c)).collect()
    }

    /// Wire body, domain-tagged.
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_transport(self.transport)
    }

    fn encode_with_transport(&self, transport: u64) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(DOMAIN_PROOF);
        out.extend_from_slice(&transport.to_le_bytes());
        let definition = self.complex.encode();
        let dl = u32::try_from(definition.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&dl.to_le_bytes());
        out.extend_from_slice(&definition);
        out.extend_from_slice(&self.dim.to_le_bytes());
        for chain in [&self.target, &self.witness] {
            let n = u32::try_from(chain.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&n.to_le_bytes());
            for (cell, coeff) in chain {
                out.extend_from_slice(&cell.to_le_bytes());
                exact_codec::put_exact(coeff, &mut out);
            }
        }
        let n = u32::try_from(self.deps.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for dep in &self.deps {
            let l = u32::try_from(dep.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&l.to_le_bytes());
            out.extend_from_slice(dep);
        }
        out
    }

    /// Decode a domain-tagged proof body.
    pub fn decode(bytes: &[u8]) -> Result<Self, ComplexBroken> {
        let mut at = 0usize;
        let domain = exact_codec::take_u8(bytes, &mut at)?;
        if domain != DOMAIN_PROOF {
            return Err(ComplexBroken::Domain(domain));
        }
        let transport = exact_codec::take_u64(bytes, &mut at)?;
        let dl = exact_codec::take_u32(bytes, &mut at)? as usize;
        let definition = exact_codec::take_bytes(bytes, &mut at, dl)?;
        let complex = DeclaredComplex::decode(definition)?;
        let dim = exact_codec::take_u32(bytes, &mut at)?;
        let mut chains = Vec::new();
        for _ in 0..2 {
            let n = exact_codec::take_u32(bytes, &mut at)? as usize;
            let mut chain = Vec::with_capacity(n.min(1 << 16));
            for _ in 0..n {
                let cell = exact_codec::take_u32(bytes, &mut at)?;
                let coeff = exact_codec::take_exact(bytes, &mut at)?;
                chain.push((cell, coeff));
            }
            chains.push(chain);
        }
        let witness = chains.pop().unwrap_or_default();
        let target = chains.pop().unwrap_or_default();
        let n = exact_codec::take_u32(bytes, &mut at)? as usize;
        let mut deps = Vec::with_capacity(n.min(1 << 12));
        for _ in 0..n {
            let l = exact_codec::take_u32(bytes, &mut at)? as usize;
            deps.push(exact_codec::take_bytes(bytes, &mut at, l)?.to_vec());
        }
        if at != bytes.len() {
            return Err(ComplexBroken::Trailing);
        }
        Ok(Self {
            transport,
            complex,
            dim,
            target,
            witness,
            deps,
        })
    }
}
