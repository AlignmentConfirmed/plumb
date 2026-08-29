//! Court settlement event — neutral leaf type for dual-circuit claims.
//!
//! ```text
//! datum RewardBook  ──emits──►  CreditEvent
//!                                 │
//!                    ┌────────────┴────────────┐
//!                    ▼                         ▼
//!              CapacityDelta (G)         PayoutClaim (E)
//! ```
//!
//! **Diamond ruling:** one verified `work_id` may lawfully project into
//! **both** game capacity and edge payout (dual claim classes). Circuits
//! do not cross; dual claiming is not inflation.
//!
//! Schema is **tollway-agnostic**: the court verifies multi-axial closure
//! and issues credit; it does not encode which domain mesh produced the
//! geometry (any portable Shape path).
//!
//! This module lives in assay so court, edge, and game share one type
//! without cyclic crate deps. No mesh, no venue, no game imports.

use crate::work::WorkId;

/// Which isolated circuits this credit may lawfully project into.
///
/// Dual claim is the diamond default: both flags true. A single class
/// may be disabled for specialized pools without changing the schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimClasses {
    /// Authorize a game [`CapacityDelta`](crate::credit_event) mint path.
    pub game_capacity: bool,
    /// Authorize an external bounty [`PayoutClaim`] path on the edge.
    pub edge_payout: bool,
}

impl ClaimClasses {
    /// Dual claim — game capacity **and** edge payout (diamond locked).
    pub const fn dual() -> Self {
        Self {
            game_capacity: true,
            edge_payout: true,
        }
    }

    /// Game capacity only (no external payout projection).
    pub const fn game_only() -> Self {
        Self {
            game_capacity: true,
            edge_payout: false,
        }
    }

    /// Edge payout only (no capacity mint projection).
    pub const fn edge_only() -> Self {
        Self {
            game_capacity: false,
            edge_payout: true,
        }
    }
}

impl Default for ClaimClasses {
    fn default() -> Self {
        Self::dual()
    }
}

/// One court-settled useful work, portable across edge and game sinks.
///
/// Emitted **only** after the court has verified structure and recorded
/// `work_id` once. Sinks must not invent events; replaying the same
/// `work_id` at the court yields no second event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditEvent {
    /// Structure identity (primary anti-double-pay key).
    pub work_id: WorkId,
    /// Transport field from the submission (informational; not identity).
    pub transport: u64,
    /// Multi-axial court credit units — one `u128` per axis, never folded.
    pub axes: Vec<u128>,
    /// Isolated circuit projections authorized for this event.
    pub classes: ClaimClasses,
}

impl CreditEvent {
    /// Build a dual-claim event (default diamond posture).
    pub fn dual(work_id: WorkId, transport: u64, axes: Vec<u128>) -> Self {
        Self {
            work_id,
            transport,
            axes,
            classes: ClaimClasses::dual(),
        }
    }

    /// Build with explicit claim classes.
    pub fn with_classes(
        work_id: WorkId,
        transport: u64,
        axes: Vec<u128>,
        classes: ClaimClasses,
    ) -> Self {
        Self {
            work_id,
            transport,
            axes,
            classes,
        }
    }

    /// Whether this event may mint game capacity.
    pub fn projects_game(&self) -> bool {
        self.classes.game_capacity
    }

    /// Whether this event may authorize an edge payout.
    pub fn projects_edge(&self) -> bool {
        self.classes.edge_payout
    }

    /// Axis count (never a product score).
    pub fn axis_count(&self) -> usize {
        self.axes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_is_default_and_both_project() {
        let id = WorkId::from_bytes(b"w".to_vec());
        let e = CreditEvent::dual(id, 1, vec![2, 3]);
        assert!(e.projects_game());
        assert!(e.projects_edge());
        assert_eq!(e.axis_count(), 2);
        assert_eq!(ClaimClasses::default(), ClaimClasses::dual());
    }

    #[test]
    fn single_class_flags() {
        assert!(ClaimClasses::game_only().game_capacity);
        assert!(!ClaimClasses::game_only().edge_payout);
        assert!(!ClaimClasses::edge_only().game_capacity);
        assert!(ClaimClasses::edge_only().edge_payout);
    }
}
