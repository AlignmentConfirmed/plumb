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
use num_traits::Zero;

/// Domain byte for declared-complex claims. Boundary is 1, Shape is 2.
pub const DOMAIN_DECLARED: u8 = 3;

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
    /// The evaluation budget ran out. A refusal, never a hang.
    FuelExhausted,
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
struct Fuel(u64);

impl Fuel {
    fn spend(&mut self, units: u64) -> Result<(), ComplexBroken> {
        match self.0.checked_sub(units) {
            Some(left) => {
                self.0 = left;
                Ok(())
            }
            None => Err(ComplexBroken::FuelExhausted),
        }
    }
}

impl DeclaredComplex {
    /// Admit the declaration: shape, canonicality, and `∂∘∂ = 0`,
    /// all under `fuel`.
    pub fn admit(&self, fuel: u64) -> Result<(), ComplexBroken> {
        let mut fuel = Fuel(fuel);
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
        Ok(())
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
    ) -> Result<(), ComplexBroken> {
        let mut fuel = Fuel(fuel);
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
            return Ok(()); // a 0-chain has no boundary to open
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
        Ok(())
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
    /// complex, and the witness closes in it.
    pub fn verify(&self, fuel: u64) -> Result<(), ComplexBroken> {
        self.complex.admit(fuel)?;
        self.complex.closes(self.dim, &self.witness, fuel)
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
