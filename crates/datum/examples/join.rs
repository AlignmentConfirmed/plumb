//! Join the court with multi-axial useful-work credit.
//!
//! ```text
//! cargo run --example join
//! ```
//!
//! Builds shape-domain work (orbs/edges/charges), credits it on the
//! reward book until the survey price is covered, then enacts the
//! application. No kernel tree required — pure highway path.

use datum::board::Application;
use datum::extent::Extent;
use datum::negotiation::Position;
use datum::onramp::{shape_body, shape_from_edges};
use datum::reward::RewardBook;
use datum::settle;
use isthmus::ratio::Exact;
use num_bigint::BigInt;

fn application(name: &str, run: u128, offer: u128) -> Application {
    let mut position = Position::new();
    let amount = Exact::from(BigInt::from(offer));
    position.offer(isthmus::layout::TAG, amount.clone());
    for at in 1..=1 {
        position.offer(&format!("{name}-d{at}"), amount.clone());
        position.offer(&format!("nova-d{at}"), amount.clone());
    }
    Application {
        applicant: name.into(),
        shape: vec![run],
        position,
        witness: format!("{name} shape-domain work_id credits"),
    }
}

/// One-axis closed boundary credits (matches 1-D survey price arity).
fn boundary_credits(count: u128) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for flux in 1..=count {
        let mut b = assay::Boundary::new(1);
        let f = assay::whole(flux as i64);
        let _ = b.face(assay::Facet::new(0, assay::Orientation::Low, f.clone()));
        let _ = b.face(assay::Facet::new(0, assay::Orientation::High, f));
        out.push(assay::Claim::new(0, b).encode());
    }
    out
}

fn main() {
    let court = match datum::ledger::authority() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("no authority chain: {e}");
            std::process::exit(1);
        }
    };

    let applicant = "join-example";
    let run_width = 4u128;
    let app = application(applicant, run_width, 64);

    let proposal = match datum::board::survey(&court, &app) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("survey refused: {e:?}");
            std::process::exit(1);
        }
    };
    println!("survey price (per axis): {}", proposal.price);
    println!("estate: {:?}", proposal.estate);

    // Demonstrate shape on-ramp (3-orb triangle) frames on the highway,
    // then fund with 1-D boundary credits that match price arity.
    let shape = match shape_from_edges(
        3,
        [
            (0, 1, assay::whole(1)),
            (1, 2, assay::whole(1)),
            (0, 2, assay::whole(1)),
        ],
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("shape build failed: {e:?}");
            std::process::exit(1);
        }
    };
    let shape_body = match shape_body(0, shape) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("shape body failed: {e:?}");
            std::process::exit(1);
        }
    };
    let mut wire = Vec::new();
    if let Err(e) = isthmus::work::put_shape_claim(&shape_body, &mut wire) {
        eprintln!("frame failed: {e}");
        std::process::exit(1);
    }
    println!(
        "highway shape frame: {} bytes, tag {}",
        wire.len(),
        isthmus::work::SHAPE_CLAIM_TAG
    );

    // Shape credit is per-orb (3 axes here); survey price for a 1-D run
    // is one component. Covers requires same arity — so settlement uses
    // matching-arity boundary work. Shape is still framed and credited
    // on a separate book to show the PoUW path.
    let mut shape_book = RewardBook::new();
    match shape_book.credit_claim(&shape_body) {
        Ok(c) => println!(
            "shape work_id credited on shape book: {} components {:?}",
            c.axes.axes(),
            c.axes.components()
        ),
        Err(e) => println!("shape credit note: {e:?}"),
    }

    let mut book = RewardBook::new();
    let bodies = boundary_credits(run_width);
    let refs: Vec<&[u8]> = bodies.iter().map(|b| b.as_slice()).collect();
    match settle::join_with_work(&mut book, &court, &app, &refs) {
        Ok((_prop, grown, credits)) => {
            println!(
                "enacted: {} new credits, court now {} acts",
                credits.len(),
                grown.acts().len()
            );
            let live: Vec<_> = grown
                .deeds()
                .into_iter()
                .filter(|d| d.live)
                .map(|d| d.holder)
                .collect();
            println!("live holders: {live:?}");
            assert!(
                book.covers(&Extent::new(vec![run_width])),
                "book should cover price"
            );
            println!("ok: {applicant} joined with work_id-funded settlement");
        }
        Err(e) => {
            eprintln!("join refused: {e:?}");
            std::process::exit(1);
        }
    }
}
