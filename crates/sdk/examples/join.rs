//! Join the court through the SDK — both sides of the wire, one process.
//!
//! ```text
//! cargo run -p plumb-sdk --example join
//! ```
//!
//! The kernel side uses only the SDK: declare, check the grant, wrap
//! claims in opaque envelopes. The court side is `datum`, played
//! in-process because transport is not built yet. The seam between the
//! two halves of this file is the seam the network will be.

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
    position.offer(&format!("{name}-d1"), amount.clone());
    position.offer("nova-d1", amount);
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
    // ── court side: the authority chain, replayed off disk ──────────
    let court = match datum::ledger::authority() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("no authority chain: {e}");
            std::process::exit(1);
        }
    };

    let applicant = "join-example";

    // ── kernel side: SDK only from here to the seam ─────────────────

    // attach: declare what we speak; agree with the court's declaration.
    let ours = sdk::attach::declare(&court, applicant, 1 << 16);
    let court_hello = sdk::attach::declare(&court, "isthmus", 1 << 16);
    let pact = match sdk::attach::agree(&ours, &court_hello) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("attach refused: {e:?}");
            std::process::exit(1);
        }
    };
    println!(
        "attached: {} shared revisions, bound {}",
        pact.revisions.len(),
        pact.bound
    );

    // grant: authorization is a ledger fact. The founding chain grants
    // isthmus 64–79; our applicant holds nothing yet — both answers
    // are read off the chain, not off a config.
    println!(
        "grant check: isthmus@64 = {}, {applicant}@64 = {}",
        sdk::grant::authorizes(&court, "isthmus", 64),
        sdk::grant::authorizes(&court, applicant, 64),
    );

    // submit: portable shape work (a 3-orb triangle), enveloped. The
    // SDK frames the body without reading it; a carrier forwards it
    // without being able to.
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
    let body = match shape_body(0, shape) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("shape body failed: {e:?}");
            std::process::exit(1);
        }
    };
    let envelope = match sdk::submit::shape(&body) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("frame failed: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "highway shape frame: {} bytes, tag {}",
        envelope.len(),
        sdk::submit::SHAPE_CLAIM_TAG
    );

    // ── the seam: everything below is the court's side ──────────────

    // The court opens the envelope and credits the work by identity.
    let (tag, received) = match sdk::submit::open(&envelope) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("court could not open envelope: {e}");
            std::process::exit(1);
        }
    };
    assert_eq!(tag, sdk::submit::SHAPE_CLAIM_TAG);
    let mut shape_book = RewardBook::new();
    match shape_book.credit_claim(received) {
        Ok(c) => println!(
            "shape work_id credited on shape book: {} components {:?}",
            c.axes.axes(),
            c.axes.components()
        ),
        Err(e) => println!("shape credit note: {e:?}"),
    }

    // Survey, fund with matching-arity boundary work, enact.
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
            assert!(
                book.covers(&Extent::new(vec![run_width])),
                "book should cover price"
            );
            println!("ok: {applicant} joined through the SDK");
        }
        Err(e) => {
            eprintln!("join refused: {e:?}");
            std::process::exit(1);
        }
    }
}
