//! Shape-domain PoUW: orbs, edges, charges, work_id, admit.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use assay::shape::{triangle_claim, Shape, ShapeBroken, ShapeClaim};
use assay::whole;
use assay::work::{WorkBody, DOMAIN_BOUNDARY};

#[test]
fn triangle_admits_and_round_trips() {
    let claim = triangle_claim(7);
    claim.verify().expect("admit");
    let bytes = claim.encode();
    let back = ShapeClaim::decode(&bytes).expect("decode");
    assert_eq!(back.transport, 7);
    assert_eq!(back.shape.orbs(), 3);
    assert_eq!(back.shape.edges().len(), 3);
    assert_eq!(back.shape.credit_axes(), vec![1, 1, 1]);
}

#[test]
fn work_id_ignores_transport() {
    let a = triangle_claim(1).work_id();
    let b = triangle_claim(99).work_id();
    assert_eq!(a, b);
}

#[test]
fn empty_shape_is_not_useful() {
    let s = Shape::new(3);
    assert!(matches!(s.admit(), Err(ShapeBroken::Empty)));
    assert!(s.credit_axes().is_empty());
}

#[test]
fn self_loop_and_zero_charge_refuse() {
    let mut s = Shape::new(2);
    assert!(matches!(
        s.edge(0, 0, whole(1)),
        Err(ShapeBroken::BadEdge { .. })
    ));
    assert!(matches!(
        s.edge(0, 1, whole(0)),
        Err(ShapeBroken::ZeroCharge { .. })
    ));
}

#[test]
fn work_body_dispatches_domains() {
    let shape = triangle_claim(0).encode();
    match WorkBody::parse(&shape).expect("shape") {
        WorkBody::Shape(s) => assert!(s.verify().is_ok()),
        WorkBody::Boundary(_) => panic!("expected shape"),
    }
    // boundary domain still works
    let mut b = assay::Boundary::new(1);
    assert!(b.face(assay::Facet::new(
        0,
        assay::Orientation::Low,
        whole(1)
    )));
    assert!(b.face(assay::Facet::new(
        0,
        assay::Orientation::High,
        whole(1)
    )));
    let bound = assay::Claim::new(0, b).encode();
    assert_eq!(bound[0], DOMAIN_BOUNDARY);
    match WorkBody::parse(&bound).expect("boundary") {
        WorkBody::Boundary(c) => assert!(c.verify().is_some()),
        WorkBody::Shape(_) => panic!("expected boundary"),
    }
}

#[test]
fn edge_order_normalised_for_work_id() {
    let mut s1 = Shape::new(2);
    s1.edge(0, 1, whole(3)).unwrap();
    let mut s2 = Shape::new(2);
    s2.edge(1, 0, whole(3)).unwrap(); // swapped endpoints
    assert_eq!(
        ShapeClaim::new(0, s1).work_id(),
        ShapeClaim::new(0, s2).work_id()
    );
}
