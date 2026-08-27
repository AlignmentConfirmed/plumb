//! Portable multi-axial work claims — boundary domain (PoWC flux).
//!
//! Shape-domain PoUW lives in [`crate::shape`]. Bodies are domain-tagged:
//!
//! ```text
//! domain u8=1 ‖ LE64(transport) ‖ LE32(axes) ‖ LE32(facet_count)
//!            ‖ (LE32(axis) ‖ orientation u8 ‖ exact)…
//! ```
//!
//! **`work_id` is the structure, not the transport field.**

use crate::exact_codec::{self, ExactBroken};
use crate::flux::{Boundary, Facet, Orientation};
use crate::{assess, Convergence, Upsilon};

/// Domain tag for multi-axial boundary claims.
pub const DOMAIN_BOUNDARY: u8 = 1;

/// Why a claim body was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimBroken {
    /// Buffer ended mid-field.
    Truncated,
    /// Wrong or missing domain byte.
    Domain(u8),
    /// Exact field refused.
    Exact(ExactBroken),
    /// Orientation byte not 0 or 1.
    Orientation(u8),
    /// Facet axis out of range for declared axes.
    AxisOutOfRange {
        /// Facet axis.
        axis: usize,
        /// Declared axis count.
        axes: usize,
    },
    /// Bytes left after the claim.
    Trailing,
}

impl From<ExactBroken> for ClaimBroken {
    fn from(e: ExactBroken) -> Self {
        match e {
            ExactBroken::Truncated => ClaimBroken::Truncated,
            other => ClaimBroken::Exact(other),
        }
    }
}

/// Content address of useful work: structure bytes only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkId(Vec<u8>);

impl WorkId {
    /// Build from raw structure bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Raw structure bytes (stable encoding, transport zeroed).
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A portable multi-axial boundary claim: flux closure on every axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// **Transport only** — not the credit key.
    pub nonce: u64,
    /// Oriented boundary the producer claims closes on every axis.
    pub boundary: Boundary,
}

impl Claim {
    /// Build a claim from a boundary and optional transport tag.
    pub fn new(nonce: u64, boundary: Boundary) -> Self {
        Self { nonce, boundary }
    }

    /// Assess this claim. Mint [`Upsilon`] only if every axis cancels.
    pub fn assess(&self) -> Convergence {
        assess(&self.boundary)
    }

    /// Produce: only succeeds when the boundary closes on every axis.
    pub fn produce(&self) -> Option<Upsilon> {
        self.assess().witness()
    }

    /// Verify by re-deriving.
    pub fn verify(&self) -> Option<Upsilon> {
        self.produce()
    }

    /// **Primary identity for PoWC credit:** structure only.
    pub fn work_id(&self) -> WorkId {
        WorkId(self.encode_with_transport(0))
    }

    /// Encode the portable body (domain-tagged).
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_transport(self.nonce)
    }

    fn encode_with_transport(&self, transport: u64) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(DOMAIN_BOUNDARY);
        out.extend_from_slice(&transport.to_le_bytes());
        let axes = u32::try_from(self.boundary.axes()).unwrap_or(u32::MAX);
        out.extend_from_slice(&axes.to_le_bytes());
        let facets = self.boundary.facets();
        let n = u32::try_from(facets.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&n.to_le_bytes());
        for facet in facets {
            let axis = u32::try_from(facet.axis).unwrap_or(u32::MAX);
            out.extend_from_slice(&axis.to_le_bytes());
            out.push(match facet.orientation {
                Orientation::Low => 0,
                Orientation::High => 1,
            });
            exact_codec::put_exact(&facet.flux, &mut out);
        }
        out
    }

    /// Decode a domain-tagged boundary body.
    pub fn decode(bytes: &[u8]) -> Result<Self, ClaimBroken> {
        let mut at = 0usize;
        let domain = exact_codec::take_u8(bytes, &mut at)?;
        if domain != DOMAIN_BOUNDARY {
            return Err(ClaimBroken::Domain(domain));
        }
        let nonce = exact_codec::take_u64(bytes, &mut at)?;
        let axes = exact_codec::take_u32(bytes, &mut at)? as usize;
        let n = exact_codec::take_u32(bytes, &mut at)? as usize;
        let mut boundary = Boundary::new(axes);
        for _ in 0..n {
            let axis = exact_codec::take_u32(bytes, &mut at)? as usize;
            if axis >= axes {
                return Err(ClaimBroken::AxisOutOfRange { axis, axes });
            }
            let o = exact_codec::take_u8(bytes, &mut at)?;
            let orientation = match o {
                0 => Orientation::Low,
                1 => Orientation::High,
                other => return Err(ClaimBroken::Orientation(other)),
            };
            let flux = exact_codec::take_exact(bytes, &mut at)?;
            let _ = boundary.face(Facet::new(axis, orientation, flux));
        }
        if at != bytes.len() {
            return Err(ClaimBroken::Trailing);
        }
        Ok(Self { nonce, boundary })
    }
}

/// Credit weight of a closed boundary claim: one unit per closed axis.
pub fn credit_axes(claim: &Claim) -> Vec<u128> {
    match claim.assess() {
        Convergence::Closed(_) => {
            let n = claim.boundary.axes();
            if n == 0 {
                Vec::new()
            } else {
                vec![1u128; n]
            }
        }
        _ => Vec::new(),
    }
}

/// Decode either domain: boundary or shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkBody {
    /// Multi-axial flux claim.
    Boundary(Claim),
    /// Shape-domain PoUW claim.
    Shape(crate::shape::ShapeClaim),
}

impl WorkBody {
    /// Domain-dispatch decode.
    ///
    /// Returns `None` when the domain is unknown or the body is empty.
    /// Shape structural refusals are `Some(Err)` via [`ShapeClaim::decode`];
    /// use [`WorkBody::parse`] for a unified result.
    pub fn parse(bytes: &[u8]) -> Result<Self, ParseBroken> {
        match bytes.first().copied() {
            Some(DOMAIN_BOUNDARY) => Claim::decode(bytes)
                .map(WorkBody::Boundary)
                .map_err(ParseBroken::Boundary),
            Some(crate::shape::DOMAIN_SHAPE) => crate::shape::ShapeClaim::decode(bytes)
                .map(WorkBody::Shape)
                .map_err(ParseBroken::Shape),
            Some(d) => Err(ParseBroken::Domain(d)),
            None => Err(ParseBroken::Empty),
        }
    }

    /// Structure work_id.
    pub fn work_id(&self) -> WorkId {
        match self {
            WorkBody::Boundary(c) => c.work_id(),
            WorkBody::Shape(s) => s.work_id(),
        }
    }

    /// Transport field.
    pub fn transport(&self) -> u64 {
        match self {
            WorkBody::Boundary(c) => c.nonce,
            WorkBody::Shape(s) => s.transport,
        }
    }

    /// Per-axis credit if the work verifies; empty if not.
    pub fn credit_axes(&self) -> Vec<u128> {
        match self {
            WorkBody::Boundary(c) => credit_axes(c),
            WorkBody::Shape(s) => {
                if s.verify().is_ok() {
                    s.shape.credit_axes()
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Boundary-only in-process witness, if any.
    ///
    /// Shape-domain credits never mint [`Upsilon`]; structure admits without a flux witness.
    pub fn witness(&self) -> Option<Upsilon> {
        match self {
            WorkBody::Boundary(c) => c.verify(),
            WorkBody::Shape(_) => None,
        }
    }

    /// Whether this body verifies as useful work.
    pub fn verifies(&self) -> bool {
        match self {
            WorkBody::Boundary(c) => c.verify().is_some(),
            WorkBody::Shape(s) => s.verify().is_ok(),
        }
    }
}

/// Unified parse refusal for [`WorkBody::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseBroken {
    /// Empty buffer.
    Empty,
    /// Unknown domain byte.
    Domain(u8),
    /// Boundary domain refused.
    Boundary(ClaimBroken),
    /// Shape domain refused.
    Shape(crate::shape::ShapeBroken),
}
