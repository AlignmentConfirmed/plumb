//! Land the deed proper: mature an observed claim into its claimant's
//! deed on the stored chain.
//!
//! Usage: `cargo run --example mature -- <holder> <low> <high>`
//!
//! Deliberate, like the mint and the observation: validates that every
//! tag in the run is encumbered by exactly this holder, that the
//! holder holds nothing (H1), and that the future chain is well-formed
//! — then appends one Issue act. The prior chain stays a byte prefix.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (holder, low, high) = match (args.get(1), args.get(2), args.get(3)) {
        (Some(h), Some(l), Some(hi)) => {
            match (l.parse::<u64>(), hi.parse::<u64>()) {
                (Ok(l), Ok(hi)) => (h.clone(), l, hi),
                _ => {
                    eprintln!("low/high must be tags");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("usage: mature <holder> <low> <high>");
            std::process::exit(1);
        }
    };

    let stored = match datum::ledger::stored() {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("no chain ({e})");
            std::process::exit(1);
        }
    };
    let acts = match isthmus::deed::chain::decode(&stored) {
        Ok(acts) => acts,
        Err(e) => {
            eprintln!("chain malformed: {e}");
            std::process::exit(1);
        }
    };
    let mut court = isthmus::deed::Ledger::replay(isthmus::layout::Layout::founding(), acts);

    let deed = match court.mature(&holder, low, high) {
        Ok(deed) => deed,
        Err(refusal) => {
            eprintln!("REFUSED: {refusal:?}");
            std::process::exit(1);
        }
    };
    if let Err(flaw) = court.well_formed() {
        eprintln!("REFUSED: the matured chain is ill-formed: {flaw:?}");
        std::process::exit(1);
    }

    let grown = isthmus::deed::chain::encode(court.acts());
    // The maturation appended exactly one act, so the prior bytes are a
    // prefix; refuse to write anything that is not.
    if grown.get(..stored.len()) != Some(stored.as_slice()) {
        eprintln!("REFUSED: the prior chain would not be a byte prefix");
        std::process::exit(1);
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(datum::ledger::FOUNDING);
    match std::fs::write(&path, &grown) {
        Ok(()) => println!(
            "matured: {holder} holds {}-{} as a deed; {} -> {} bytes",
            deed.low(),
            deed.high(),
            stored.len(),
            grown.len()
        ),
        Err(e) => {
            eprintln!("cannot write: {e}");
            std::process::exit(1);
        }
    }
}
