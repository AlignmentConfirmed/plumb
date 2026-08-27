//! Strike a deed from the authority's chain.
//!
//! **Nothing is deleted.** [`Act::Retire`] is an entry, not an erasure
//! — the chain is append-only and a struck deed is struck *by adding a
//! record that says so*. Every earlier act stays exactly where it was,
//! and the stored bytes grow at the end and nowhere else.
//!
//! ## What striking costs, stated before it is run
//!
//! `IS-6` §8 and `IS-3` §5.2: **retired ground is never reissued.** A
//! grantee may retire a tag; it may not reissue one, and neither may
//! this court. So striking a deed does not return its ground to the
//! open pool — it spends it.
//!
//! That is the rule that stops space being laundered through retire
//! and re-grant, and it means the cost of a strike is the *whole*
//! deed, not the disputed part of it.
//!
//! Deliberate, like the mint: run by a person, naming the holder, and
//! printing exactly what it spent.
//!
//!     cargo run --example strike -- assay

use isthmus::deed::{chain, Act, Ledger, Standing};
use isthmus::layout::Layout;

fn main() {
    let Some(holder) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example strike -- <holder>");
        std::process::exit(1);
    };

    let stored = match datum::ledger::stored() {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("no chain to append to ({e}) — mint first");
            std::process::exit(1);
        }
    };
    let acts = match chain::decode(&stored) {
        Ok(acts) => acts,
        Err(e) => {
            eprintln!("the chain is malformed: {e} — nothing will be appended");
            std::process::exit(1);
        }
    };
    let court = Ledger::replay(Layout::founding(), acts.clone()).under(datum::ledger::CHAIN);

    let Some(deed) = court
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == holder)
    else {
        println!("{holder} holds no live deed — nothing to strike");
        return;
    };

    println!(
        "striking {holder}'s deed over {}-{} ({} tags)",
        deed.low(),
        deed.high(),
        deed.width(),
    );
    println!("  retired ground is NEVER reissued — this spends the whole deed,");
    println!("  not only the disputed part of it");

    let mut grown = acts;
    grown.push(Act::Retire {
        holder: holder.clone(),
    });
    let bytes = chain::encode(&grown);

    if !bytes.starts_with(&stored) {
        eprintln!("the rebuild is not a prefix extension — refusing to write");
        std::process::exit(1);
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Err(e) = std::fs::write(root.join(datum::ledger::FOUNDING), &bytes) {
        eprintln!("could not write the chain: {e}");
        std::process::exit(1);
    }

    let after = Ledger::replay(Layout::founding(), grown).under(datum::ledger::CHAIN);
    println!(
        "appended 1 act, {} -> {} bytes; the prior chain is a byte prefix",
        stored.len(),
        bytes.len(),
    );
    println!(
        "  {holder} now holds {} live deed(s); tag {} reads {:?}",
        after.deeds().iter().filter(|d| d.live && d.holder == holder).count(),
        deed.low(),
        after.standing_of(deed.low()),
    );
    debug_assert!(matches!(after.standing_of(deed.low()), Standing::Retired));
}
