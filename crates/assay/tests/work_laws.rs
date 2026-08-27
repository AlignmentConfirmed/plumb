//! Portable multi-axial work claims — produce, verify, refuse open work.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use assay::work::{credit_axes, Claim};
use assay::{whole, Boundary, Facet, Orientation};

fn closed_box(nonce: u64) -> Claim {
    let mut b = Boundary::new(2);
    let f = whole(3);
    assert!(b.face(Facet::new(0, Orientation::Low, f.clone())));
    assert!(b.face(Facet::new(0, Orientation::High, f.clone())));
    assert!(b.face(Facet::new(1, Orientation::Low, f.clone())));
    assert!(b.face(Facet::new(1, Orientation::High, f)));
    Claim::new(nonce, b)
}

#[test]
fn a_closed_claim_mints_and_round_trips() {
    let claim = closed_box(7);
    assert!(claim.produce().is_some());
    let bytes = claim.encode();
    let back = Claim::decode(&bytes).expect("decode");
    assert_eq!(back.nonce, 7);
    assert!(back.verify().is_some());
    assert_eq!(credit_axes(&back), vec![1, 1]);
}

#[test]
fn an_open_claim_earns_nothing() {
    let mut b = Boundary::new(1);
    // both faces, unequal flux → open
    assert!(b.face(Facet::new(0, Orientation::Low, whole(1))));
    assert!(b.face(Facet::new(0, Orientation::High, whole(2))));
    let claim = Claim::new(1, b);
    assert!(claim.produce().is_none());
    assert!(credit_axes(&claim).is_empty());
}

#[test]
fn unmeasured_earns_nothing() {
    let claim = Claim::new(2, Boundary::new(2));
    assert!(claim.produce().is_none());
    assert!(credit_axes(&claim).is_empty());
}

#[test]
fn hostile_trailing_bytes_refuse() {
    let mut bytes = closed_box(3).encode();
    bytes.push(0xff);
    assert!(Claim::decode(&bytes).is_err());
}

#[test]
fn work_id_is_structure_not_transport() {
    let a = closed_box(1);
    let b = closed_box(999);
    assert_eq!(a.work_id(), b.work_id());
    // different flux → different structure
    let mut boundary = Boundary::new(2);
    let f = whole(9);
    assert!(boundary.face(Facet::new(0, Orientation::Low, f.clone())));
    assert!(boundary.face(Facet::new(0, Orientation::High, f.clone())));
    assert!(boundary.face(Facet::new(1, Orientation::Low, f.clone())));
    assert!(boundary.face(Facet::new(1, Orientation::High, f)));
    let other = Claim::new(1, boundary);
    assert_ne!(a.work_id(), other.work_id());
}
