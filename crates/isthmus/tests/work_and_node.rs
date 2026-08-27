//! Opaque work frames and carrier role — no payload verification.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use isthmus::node::{self, CarrierOut, Role};
use isthmus::work::{self, CLAIM_TAG, RECEIPT_TAG, SHAPE_CLAIM_TAG};

#[test]
fn roles_are_capabilities_not_ranks() {
    assert!(Role::Producer.produces());
    assert!(!Role::Producer.verifies());
    assert!(Role::Verifier.verifies());
    assert!(Role::Carrier.carries());
    assert!(!Role::Carrier.produces());
}

#[test]
fn claim_frame_round_trips_opaque_body() {
    let body = b"not-a-proof-just-bytes";
    let mut wire = Vec::new();
    work::put_claim(body, &mut wire).expect("frame");
    let (tag, value) = work::take_frame(&wire).expect("take");
    assert_eq!(tag, CLAIM_TAG);
    assert_eq!(value, body);
}

#[test]
fn carrier_forwards_foreign_tags_whole() {
    // Tag 1 relation-shaped foreign load.
    let mut wire = Vec::new();
    isthmus::frame::put_frame(
        &isthmus::layout::Layout::founding(),
        1,
        b"foreign-payload",
        &mut wire,
    )
    .expect("put");
    match node::carrier_step(&wire).expect("step") {
        CarrierOut::Forward { whole } => assert_eq!(whole, wire.as_slice()),
        CarrierOut::Deliver { .. } => panic!("foreign tag must forward"),
    }
}

#[test]
fn carrier_delivers_work_tags_without_verifying() {
    let mut wire = Vec::new();
    work::put_receipt(b"opaque-receipt", &mut wire).expect("put");
    match node::carrier_step(&wire).expect("step") {
        CarrierOut::Deliver { tag, body } => {
            assert_eq!(tag, RECEIPT_TAG);
            assert_eq!(body, b"opaque-receipt");
        }
        CarrierOut::Forward { .. } => panic!("work tag should deliver"),
    }
}

#[test]
fn shape_claim_frame_is_work_and_opaque() {
    let body = b"\x02shape-bytes-not-verified-here";
    let mut wire = Vec::new();
    work::put_shape_claim(body, &mut wire).expect("put");
    let (tag, value) = work::take_frame(&wire).expect("take");
    assert_eq!(tag, SHAPE_CLAIM_TAG);
    assert_eq!(value, body);
    match node::carrier_step(&wire).expect("step") {
        CarrierOut::Deliver { tag, body: b } => {
            assert_eq!(tag, SHAPE_CLAIM_TAG);
            assert_eq!(b, body);
        }
        CarrierOut::Forward { .. } => panic!("shape claim is a work tag"),
    }
}
