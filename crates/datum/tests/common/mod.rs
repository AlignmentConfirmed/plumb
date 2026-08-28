//! The shared test kit — one source of truth for the fixtures every
//! suite was hand-rolling. The audit found `edge_with` defined five
//! times in one binary; a fixture defined twice is two chances to
//! drift apart silently.

#![allow(dead_code, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use isthmus::deed::{Act, Ledger};
use isthmus::layout::Layout;

/// The measured bound every suite runs under.
pub const BOUND: usize = 1 << 16;

/// An edge with the founding encumbrances and one issued deed — the
/// same construction the substrate's own tests use.
pub fn edge_with(holder: &str) -> Ledger {
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room on a fresh edge");
    ledger
}

/// Bind a holder's presenting key on the chain (IS-6/4).
pub fn bind(ledger: &mut Ledger, holder: &str, key: &sig::Keypair, from: u64, until: u64) {
    ledger.record(Act::Bind {
        holder: holder.into(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public().to_vec(),
        from_epoch: from,
        until_epoch: until,
    });
}

/// The shape-domain triangle, enveloped for the highway (tag 82).
pub fn shape_triangle_envelope() -> Vec<u8> {
    let shape = datum::onramp::shape_from_edges(
        3,
        [
            (0, 1, assay::whole(1)),
            (1, 2, assay::whole(1)),
            (0, 2, assay::whole(1)),
        ],
    )
    .expect("triangle builds");
    let body = datum::onramp::shape_body(0, shape).expect("body encodes");
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
    wire
}

/// A declared-domain n-cycle, enveloped for the highway.
pub fn cycle_envelope(n: u32) -> Vec<u8> {
    let body = datum::domains::demo_cycle_claim(n, 0).encode();
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).expect("frames");
    wire
}

/// Poll a condition to true within five seconds, or fail with the
/// message. The audit found this loop hand-rolled seven times.
pub fn await_true(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out awaiting: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
