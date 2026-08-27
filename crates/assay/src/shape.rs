//! Shape — the shared PoUW domain: orbs, edges, exact charges.
//!
//! Rank-free by construction. Not a grid; not a 2-D field intersection.
//! This is what both kernels state and what the highway can carry without
//! naming a kernel type.
//!
//! ```text
//! body = domain u8=2 ‖ LE64(transport) ‖ LE32(orbs) ‖ LE32(edge_count)
//!      ‖ (LE32(i) ‖ LE32(j) ‖ exact)…
//! ```
//!
//! Endpoints are stored with `i < j`. Charges are exact rationals.
//! [`ShapeClaim::work_id`] zeros transport so structure is the credit key.

use num_traits::Zero;

use crate::exact_codec::{self, ExactBroken};
use crate::work::WorkId;
use crate::{whole, Exact};

/// Domain tag for shape claim bodies.
pub const DOMAIN_SHAPE: u8 = 2;

/// Why a shape claim was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeBroken {
    /// Buffer ended mid-field.
    Truncated,
    /// Wrong domain byte.
    Domain(u8),
    /// Exact field refused.
    Exact(ExactBroken),
    /// Zero orbs.
    NoOrbs,
    /// Edge endpoint out of range or self-loop.
    BadEdge {
        /// From orb.
        i: u32,
        /// To orb.
        j: u32,
        /// Declared orb count.
        orbs: u32,
    },
    /// Zero charge — not useful structure.
    ZeroCharge {
        /// From.
        i: u32,
        /// To.
        j: u32,
    },
    /// Duplicate undirected edge after canonicalisation.
    DuplicateEdge {
        /// From.
        i: u32,
        /// To.
        j: u32,
    },
    /// No edges — empty graph is not useful work.
    Empty,
    /// Bytes left after the claim.
    Trailing,
}

impl From<ExactBroken> for ShapeBroken {
    fn from(e: ExactBroken) -> Self {
        match e {
            ExactBroken::Truncated => ShapeBroken::Truncated,
            other => ShapeBroken::Exact(other),
        }
    }
}

/// One undirected edge with an exact charge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Lower endpoint (canonical `i < j`).
    pub i: u32,
    /// Higher endpoint.
    pub j: u32,
    /// Exact rational charge on the relation.
    pub charge: Exact,
}

impl Edge {
    /// Build a canonical edge (`i < j`). Refuses self-loops via [`Shape::edge`].
    pub fn between(a: u32, b: u32, charge: Exact) -> Option<Self> {
        if a == b {
            return None;
        }
        let (i, j) = if a < b { (a, b) } else { (b, a) };
        Some(Self { i, j, charge })
    }
}

/// A shape: orbs and charged edges. No kernel types.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Shape {
    orbs: u32,
    edges: Vec<Edge>,
}

impl Shape {
    /// Empty shape over `orbs` sites.
    pub fn new(orbs: u32) -> Self {
        Self {
            orbs,
            edges: Vec::new(),
        }
    }

    /// Orb count.
    pub fn orbs(&self) -> u32 {
        self.orbs
    }

    /// Edges, canonical order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Add an undirected charged edge. Refuses self-loop, OOR, zero charge,
    /// duplicate.
    pub fn edge(&mut self, a: u32, b: u32, charge: Exact) -> Result<(), ShapeBroken> {
        if self.orbs == 0 {
            return Err(ShapeBroken::NoOrbs);
        }
        if a == b || a >= self.orbs || b >= self.orbs {
            return Err(ShapeBroken::BadEdge {
                i: a,
                j: b,
                orbs: self.orbs,
            });
        }
        if charge.is_zero() {
            return Err(ShapeBroken::ZeroCharge { i: a, j: b });
        }
        let edge = Edge::between(a, b, charge).ok_or(ShapeBroken::BadEdge {
            i: a,
            j: b,
            orbs: self.orbs,
        })?;
        if self.edges.iter().any(|e| e.i == edge.i && e.j == edge.j) {
            return Err(ShapeBroken::DuplicateEdge {
                i: edge.i,
                j: edge.j,
            });
        }
        self.edges.push(edge);
        self.edges.sort_by_key(|x| (x.i, x.j));
        Ok(())
    }

    /// Well-formed useful shape: orbs, at least one edge, all edges valid.
    pub fn admit(&self) -> Result<(), ShapeBroken> {
        if self.orbs == 0 {
            return Err(ShapeBroken::NoOrbs);
        }
        if self.edges.is_empty() {
            return Err(ShapeBroken::Empty);
        }
        for e in &self.edges {
            if e.i >= self.orbs || e.j >= self.orbs || e.i >= e.j {
                return Err(ShapeBroken::BadEdge {
                    i: e.i,
                    j: e.j,
                    orbs: self.orbs,
                });
            }
            if e.charge.is_zero() {
                return Err(ShapeBroken::ZeroCharge { i: e.i, j: e.j });
            }
        }
        // duplicates (edges are sorted by (i,j))
        let mut prev: Option<&Edge> = None;
        for e in &self.edges {
            if let Some(p) = prev {
                if p.i == e.i && p.j == e.j {
                    return Err(ShapeBroken::DuplicateEdge { i: e.i, j: e.j });
                }
            }
            prev = Some(e);
        }
        Ok(())
    }

    /// Per-orb credit units if admitted: one unit per orb (unfolded span).
    ///
    /// Not a product of edges. Empty/refused → empty vec.
    pub fn credit_axes(&self) -> Vec<u128> {
        if self.admit().is_err() {
            return Vec::new();
        }
        let n = self.orbs as usize;
        if n == 0 {
            Vec::new()
        } else {
            vec![1u128; n]
        }
    }
}

/// Portable shape work claim for the highway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeClaim {
    /// Transport only — not part of [`ShapeClaim::work_id`].
    pub transport: u64,
    /// The useful structure.
    pub shape: Shape,
}

impl ShapeClaim {
    /// New claim.
    pub fn new(transport: u64, shape: Shape) -> Self {
        Self { transport, shape }
    }

    /// Verify by re-checking well-formedness (no handed token).
    pub fn verify(&self) -> Result<(), ShapeBroken> {
        self.shape.admit()
    }

    /// Structure identity for credit.
    pub fn work_id(&self) -> WorkId {
        WorkId::from_bytes(self.encode_with_transport(0))
    }

    /// Wire body (domain-tagged).
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_transport(self.transport)
    }

    fn encode_with_transport(&self, transport: u64) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(DOMAIN_SHAPE);
        out.extend_from_slice(&transport.to_le_bytes());
        out.extend_from_slice(&self.shape.orbs.to_le_bytes());
        let n = u32::try_from(self.shape.edges.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for e in &self.shape.edges {
            out.extend_from_slice(&e.i.to_le_bytes());
            out.extend_from_slice(&e.j.to_le_bytes());
            exact_codec::put_exact(&e.charge, &mut out);
        }
        out
    }

    /// Decode a domain-tagged shape body.
    pub fn decode(bytes: &[u8]) -> Result<Self, ShapeBroken> {
        let mut at = 0usize;
        let domain = exact_codec::take_u8(bytes, &mut at)?;
        if domain != DOMAIN_SHAPE {
            return Err(ShapeBroken::Domain(domain));
        }
        let transport = exact_codec::take_u64(bytes, &mut at)?;
        let orbs = exact_codec::take_u32(bytes, &mut at)?;
        let n = exact_codec::take_u32(bytes, &mut at)? as usize;
        let mut shape = Shape::new(orbs);
        for _ in 0..n {
            let i = exact_codec::take_u32(bytes, &mut at)?;
            let j = exact_codec::take_u32(bytes, &mut at)?;
            let charge = exact_codec::take_exact(bytes, &mut at)?;
            shape.edge(i, j, charge)?;
        }
        if at != bytes.len() {
            return Err(ShapeBroken::Trailing);
        }
        // Re-admit after build (edge() already checks).
        shape.admit()?;
        Ok(Self { transport, shape })
    }
}

/// Triangle on 3 orbs with unit charges — minimal useful shape fixture.
pub fn triangle_claim(transport: u64) -> ShapeClaim {
    let mut s = Shape::new(3);
    let c = whole(1);
    // Fresh triangle: each edge lands or the fixture is wrong (tests catch).
    let _ = s.edge(0, 1, c.clone());
    let _ = s.edge(1, 2, c.clone());
    let _ = s.edge(0, 2, c);
    ShapeClaim::new(transport, s)
}
