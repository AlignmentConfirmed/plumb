//! **Two chains link, and the link is verified.** The uplink, run.
//!
//! The tests hold the laws; this is the thing itself, printing what
//! crosses so the direction that did not exist can be watched working
//! rather than only asserted.
//!
//! ```text
//! south          declares who it is over IS-5/2
//!   -> north     records ONE anchor, over south's own chain
//!   -> north     confirms the anchor against south's actual chain
//!   -> both      compare frontiers: ordered, or concurrent
//! ```
//!
//! Read-only and in-process: it appends to no stored chain and touches
//! no file. The authority is read, never written — appending is
//! `examples/observe.rs`'s job and is a person's decision.

use isthmus::deed::{chain, Act, Ledger};
use isthmus::hello::{Hello, Uplink};
use isthmus::layout::Layout;

/// FNV-1a, eight bytes. **Not a recommendation.**
///
/// `IS-5` §3.1 frames a digest and names no function, so somebody has
/// to choose one per edge. This example chooses a deterministic toy so
/// the printed bytes are reproducible; a real edge picks a function
/// whose collision resistance it has actually thought about.
fn fnv(bytes: &[u8]) -> Vec<u8> {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash.to_le_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    // SOUTH is the real authority, read from disk. Naming it is what
    // makes it citable at all -- see ledger::CHAIN.
    let south = match datum::ledger::authority() {
        Ok(court) => court,
        Err(why) => {
            eprintln!("no authority to link to: {why}");
            std::process::exit(1);
        }
    };
    println!("south  {} acts, name {:?}", south.height(), south.name());

    // NORTH is a stranger's chain, built here, with an estate of its
    // own. It has never heard of south.
    let mut north = Ledger::new(Layout::founding()).under("north");
    if let Err(why) = north.issue("newcomer", 8) {
        eprintln!("north could not deed itself an estate: {why:?}");
        std::process::exit(1);
    }
    println!("north  {} acts, name {:?}", north.height(), north.name());

    // ---- south declares -------------------------------------------
    let declared = Hello::of(&south, "isthmus", 1 << 20).declaring(Uplink::of(&south, fnv));
    let bytes = declared.encode();
    println!("\ndeclaration  {} bytes", bytes.len());
    println!("  {}", hex(&bytes));

    let heard = match Hello::decode(&bytes) {
        Ok(hello) => hello,
        Err(why) => {
            eprintln!("the declaration did not survive the wire: {why}");
            std::process::exit(1);
        }
    };
    let Some(uplink) = heard.uplink.as_ref() else {
        eprintln!("south declared no uplink — nothing to anchor to");
        std::process::exit(1);
    };
    println!(
        "\nheard  chain {:?} at height {}, digest {}",
        uplink.chain,
        uplink.height(),
        hex(&uplink.digest),
    );
    for name in uplink.frontier.chains() {
        println!("       frontier: {name} at {}", uplink.frontier.height_of(name));
    }

    // ---- north records the vertical -------------------------------
    //
    // ONE act, over south's own chain. The frontier may name others;
    // those are south's observations and minting them here would be
    // recording "I observed X" because somebody said they did.
    let act = uplink.anchor("declared over an IS-5/2 session");
    north.record(act);
    println!("\nnorth appended the vertical; height is now {}", north.height());

    // ---- and checks it --------------------------------------------
    let Some(vertical) = north.acts().last() else {
        eprintln!("the anchor did not land");
        std::process::exit(1);
    };
    match isthmus::sphere::confirms(vertical, &south, fnv) {
        Some(true) => println!("confirms  the anchor matches south's chain at that height"),
        Some(false) => {
            eprintln!("REFUSED   the digest does not match south's chain");
            std::process::exit(1);
        }
        None => {
            eprintln!("unanswerable — wrong chain, or a height south does not have");
            std::process::exit(1);
        }
    }

    // The vertical granted nothing. Anchoring is observing.
    let deeds = north.deeds().iter().filter(|d| d.live).count();
    println!("north still holds {deeds} live deed(s) — a vertical grants no ground");

    // ---- the order ------------------------------------------------
    let mine = Hello::of(&north, "newcomer", 1 << 20).declaring(Uplink::of(&north, fnv));
    match mine.against(&heard) {
        None => println!("\norder  not comparable — one of us declared no uplink"),
        Some(None) => println!("\norder  CONCURRENT — neither has seen the other"),
        Some(Some(std::cmp::Ordering::Greater)) => {
            println!("\norder  north is AHEAD — it has seen everything south has, and more");
        }
        Some(Some(std::cmp::Ordering::Less)) => println!("\norder  north is BEHIND"),
        Some(Some(std::cmp::Ordering::Equal)) => println!("\norder  the same observations"),
    }

    // ---- and whether they collide ---------------------------------
    //
    // Printed, not asserted: whether these two overlap depends on what
    // the authority happens to hold today, and an example that
    // asserted a collision would be asserting a fact about a moving
    // chain. North deeds from the bottom of a bare edge; the authority
    // has its low tags encumbered and its estates higher up, so today
    // they are disjoint and this prints nothing.
    //
    // The point stands either way: a conflict between independent
    // appenders is DETECTED and classified, never prevented.
    // Preventing it would be a lock nobody holds.
    let found = isthmus::sphere::standoffs(&north, &south);
    println!("\nstandoffs  {}", found.len());
    for standoff in &found {
        println!(
            "  {:?} at {:?}: {} ({}) vs {} ({})",
            standoff.order,
            standoff.point,
            standoff.here.holder,
            standoff.here.chain,
            standoff.there.holder,
            standoff.there.chain,
        );
    }
    if found.is_empty() {
        println!("  the two estates are disjoint — nothing for the board");
    }

    // Finally, the chain north would store. Bytes, like everything
    // else in the hierarchy.
    let stored = chain::encode(north.acts());
    println!("\nnorth's chain  {} bytes", stored.len());
    match chain::decode(&stored) {
        Ok(acts) if acts == north.acts() => println!("  round trips"),
        Ok(_) => println!("  DECODED TO SOMETHING ELSE"),
        Err(why) => println!("  does not decode: {why}"),
    }
    let verticals = north
        .acts()
        .iter()
        .filter(|a| matches!(a, Act::Anchor { .. }))
        .count();
    println!("  {verticals} vertical(s), {} horizontal(s)", north.height() as usize - verticals);
}
