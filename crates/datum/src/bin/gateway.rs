//! `gateway` — the court's x402 HTTP face (X3).
//!
//! ```text
//! gateway <config>
//!   listen      = 127.0.0.1:9801
//!   chain       = path/to/founding.tlv   # receipts verify against this
//!   court       = court-a                # the issuing court's name
//!   seed        = <64 hex>               # the court's signing key
//!   facilitator = 0x...                  # OQ3: the Base escrow party
//! ```
//!
//! Serves the demo theta bounty: GET /query answers 402 with the
//! declared challenge; POST /answer settles, pays the yield rebate,
//! and returns a signed receipt any facilitator verifies offline.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use datum::bounty::Bounty;
use datum::query::{Guarantee, Query};
use datum::reward::RewardBook;
use datum::x402::{self, Gateway};
use isthmus::deed::Ledger;
use isthmus::layout::Layout;

fn seed_from_hex(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = hex.get(i * 2..i * 2 + 2)?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: gateway <config>");
            std::process::exit(2);
        }
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("gateway: config at {path} unreadable: {e}");
            std::process::exit(2);
        }
    };
    let mut listen = "127.0.0.1:9801".to_owned();
    let mut chain_path = None;
    let mut court = "court-a".to_owned();
    let mut seed_hex = None;
    let mut facilitator = "0xUNSET".to_owned();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().to_owned();
        match key.trim() {
            "listen" => listen = value,
            "chain" => chain_path = Some(value),
            "court" => court = value,
            "seed" => seed_hex = Some(value),
            "facilitator" => facilitator = value,
            other => eprintln!("gateway: unknown config key ignored: {other}"),
        }
    }
    let Some(seed) = seed_hex.as_deref().and_then(seed_from_hex) else {
        eprintln!("gateway: needs `seed = <64 hex>` — receipts are signed");
        std::process::exit(2);
    };
    let chain = match chain_path {
        Some(p) => match std::fs::read(&p).ok().and_then(|b| isthmus::deed::chain::decode(&b).ok()) {
            Some(acts) => Ledger::replay(Layout::founding(), acts),
            None => {
                eprintln!("gateway: chain at {p} unreadable or malformed");
                std::process::exit(1);
            }
        },
        None => {
            eprintln!("gateway: needs `chain = <path>` — receipts verify against it");
            std::process::exit(2);
        }
    };
    let query = Query {
        poser: court.clone(),
        shape: vec![2, 3],
        domain_tag: 82,
        guarantee: Guarantee::Rederivation,
        statement: datum::domains::demo_theta_universe().encode(),
    };
    let bounty = Bounty {
        query_id: query.query_id(),
        max_fuel: 200,
        max_bytes: 400,
        base: 1_000,
        per_saved_fuel: 10,
        per_saved_byte: 3,
    };
    let gateway = Gateway {
        query,
        bounty,
        book: Arc::new(Mutex::new(RewardBook::new())),
        chain,
        court,
        key: sig::Keypair::from_seed(seed),
        facilitator,
    };
    let listener = match TcpListener::bind(&listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("gateway: cannot listen on {listen}: {e}");
            std::process::exit(1);
        }
    };
    println!("gateway: 402 challenges on http://{listen}/query — answers to /answer");
    let err = x402::serve(&listener, &gateway);
    eprintln!("gateway: listener failed: {err}");
    std::process::exit(1);
}
