//! H5 — independent node appends well-formed acts; empty and bad blocks refuse.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use datum::block::{self, BlockRefused};
use isthmus::deed::{Act, Ledger};
use isthmus::layout::Layout;

#[test]
fn empty_block_refuses() {
    let court = Ledger::new(Layout::founding());
    assert!(matches!(
        block::produce(&court, vec![]),
        Err(BlockRefused::Empty)
    ));
}

#[test]
fn observation_encumber_appends() {
    let court = Ledger::new(Layout::founding());
    let acts = vec![Act::Encumber {
        low: 10,
        high: 12,
        by: "foreign-mesh".into(),
        witnessed: "test".into(),
    }];
    let block = block::produce(&court, acts).expect("produce");
    assert_eq!(block.prior_height, 0);
    assert_eq!(block.height, 1);
    assert_eq!(block.batch_len(), 1);
    block.court.well_formed().expect("well formed");
    assert_eq!(block.landed_slice().len(), 1);
}

#[test]
fn second_block_extends_height() {
    let court = Ledger::new(Layout::founding());
    let b1 = block::produce(
        &court,
        vec![Act::Encumber {
            low: 1,
            high: 1,
            by: "a".into(),
            witnessed: "t".into(),
        }],
    )
    .expect("b1");
    let b2 = block::produce(
        &b1.court,
        vec![Act::Encumber {
            low: 2,
            high: 2,
            by: "b".into(),
            witnessed: "t".into(),
        }],
    )
    .expect("b2");
    assert_eq!(b1.height, 1);
    assert_eq!(b2.prior_height, 1);
    assert_eq!(b2.height, 2);
}

#[test]
fn settlement_acts_from_funded_join_are_a_block() {
    // H5 integration: settle::land produces a block whose acts match the proposal.
    use datum::negotiation::Position;
    use datum::reward::RewardBook;
    use datum::settle;
    use datum::board::Application;
    use isthmus::ratio::Exact;
    use num_bigint::BigInt;

    let court = datum::ledger::authority().expect("authority");
    let prior = court.acts().len();
    let mut position = Position::new();
    let amount = Exact::from(BigInt::from(64u32));
    position.offer(isthmus::layout::TAG, amount.clone());
    position.offer("block-d1", amount.clone());
    position.offer("nova-d1", amount);
    let application = Application {
        applicant: "block-holder".into(),
        shape: vec![3],
        position,
        witness: "work".into(),
    };
    let mut book = RewardBook::new();
    let mut bodies = Vec::new();
    for flux in 1..=3 {
        let mut b = assay::Boundary::new(1);
        let f = assay::whole(flux);
        assert!(b.face(assay::Facet::new(0, assay::Orientation::Low, f.clone())));
        assert!(b.face(assay::Facet::new(0, assay::Orientation::High, f)));
        bodies.push(assay::Claim::new(0, b).encode());
    }
    let refs: Vec<&[u8]> = bodies.iter().map(|x| x.as_slice()).collect();
    let (proposal, block, _) =
        settle::land(&mut book, &court, &application, &refs).expect("land");
    assert_eq!(block.acts, proposal.acts);
    assert!(block.height > prior);
    assert!(block
        .court
        .deeds()
        .iter()
        .any(|d| d.live && d.holder == "block-holder"));
}

#[test]
fn onramp_foreign_tag_is_forwarded_by_carrier() {
    let mut wire = Vec::new();
    isthmus::frame::put_frame(
        &Layout::founding(),
        200,
        b"strand-dialect-bytes",
        &mut wire,
    )
    .expect("put");
    match isthmus::node::carrier_step(&wire).expect("step") {
        isthmus::node::CarrierOut::Forward { whole } => {
            assert_eq!(whole, wire.as_slice());
        }
        isthmus::node::CarrierOut::Deliver { .. } => {
            panic!("tollway tag must forward on the superhighway carrier")
        }
    }
}
