//! Tollway → superhighway on-ramp for useful work.
//!
//! Converts **public** structure (orbs, edges, exact charges) into an
//! [`assay::Shape`] and frames it for the highway. Does **not** import
//! private kernel witness types — that monopoly is the measured
//! gap; the on-ramp is the path that does not need it.
//!
//! ```text
//! a kernel's Support     ──►  assay::Shape  ──►  body (domain=2)
//!                                              ──►  isthmus tag 82 frame
//! ```
//!
//! Edge-free helpers build shapes from raw edges so the court can be
//! exercised without a kernel tree.

use assay::shape::{Shape, ShapeBroken, ShapeClaim};
use assay::Exact;
use isthmus::frame::Malformed;
use isthmus::layout::Tag;
use isthmus::work::{self, SHAPE_CLAIM_TAG};

/// Why an on-ramp conversion failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnrampBroken {
    /// Shape admit/encode refused.
    Shape(ShapeBroken),
    /// Highway framing refused.
    Frame(Malformed),
    /// Orb or endpoint count does not fit `u32`.
    TooWide,
}

impl From<ShapeBroken> for OnrampBroken {
    fn from(e: ShapeBroken) -> Self {
        OnrampBroken::Shape(e)
    }
}

/// Build an admitted shape from orbs and charged edges.
///
/// Endpoints may be in either order; charges must be non-zero exacts.
pub fn shape_from_edges(
    orbs: u32,
    edges: impl IntoIterator<Item = (u32, u32, Exact)>,
) -> Result<Shape, OnrampBroken> {
    let mut shape = Shape::new(orbs);
    for (a, b, charge) in edges {
        shape.edge(a, b, charge)?;
    }
    shape.admit()?;
    Ok(shape)
}

/// Portable claim body (domain=2) ready for credit or framing.
pub fn shape_body(transport: u64, shape: Shape) -> Result<Vec<u8>, OnrampBroken> {
    shape.admit()?;
    Ok(ShapeClaim::new(transport, shape).encode())
}

/// Full highway frame: tag 82 ‖ length ‖ shape body.
pub fn shape_frame(transport: u64, shape: Shape) -> Result<Vec<u8>, OnrampBroken> {
    let body = shape_body(transport, shape)?;
    let mut wire = Vec::new();
    work::put_shape_claim(&body, &mut wire).map_err(OnrampBroken::Frame)?;
    Ok(wire)
}

/// Peel a shape-claim frame to its body (no verification).
pub fn peel_shape_frame(wire: &[u8]) -> Result<(Tag, &[u8]), OnrampBroken> {
    let (tag, value) = work::take_frame(wire).map_err(OnrampBroken::Frame)?;
    if tag != SHAPE_CLAIM_TAG {
        return Err(OnrampBroken::Frame(Malformed::UnexpectedTag {
            expected: SHAPE_CLAIM_TAG,
            found: tag,
        }));
    }
    Ok((tag, value))
}


