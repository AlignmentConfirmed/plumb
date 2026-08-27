//! IS-4/1 §5 laws: the frame round-trips; the arm refuses rather than
//! guesses; the revision is required, never defaulted.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use isthmus::witness::{Arm, Observer, Witness};

fn sample() -> Witness {
    Witness {
        arm: Arm::Succinct,
        observer: Observer {
            kind: 2,
            identity: [7u8; 32],
            revision: "corpus/v1".into(),
            depth: 3,
        },
        subject: [9u8; 32],
        derivation: vec![1, 2, 3],
    }
}

#[test]
fn the_frame_round_trips_including_an_empty_derivation() {
    let full = sample();
    assert_eq!(Witness::decode(&full.encode()).expect("its own bytes"), full);

    let mut bare = sample();
    bare.derivation.clear();
    assert_eq!(Witness::decode(&bare.encode()).expect("bare"), bare);
}

#[test]
fn an_unknown_arm_refuses_because_the_budget_is_not_guessable() {
    let mut bytes = sample().encode();
    bytes[0] = 2;
    assert!(
        Witness::decode(&bytes).is_err(),
        "a watcher must know succinct vs replay BEFORE it starts"
    );
}

#[test]
fn the_revision_is_required_never_defaulted() {
    let mut unnamed = sample();
    unnamed.observer.revision = String::new();
    assert!(
        Witness::decode(&unnamed.encode()).is_err(),
        "a corpus without a revision names a moving target"
    );
}

#[test]
fn truncation_refuses_rather_than_repairs() {
    let bytes = sample().encode();
    // Cut inside the observer identity: bytes were promised.
    assert!(Witness::decode(&bytes[..20]).is_err());
}
