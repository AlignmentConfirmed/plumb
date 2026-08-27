//! `plumbd` — the node daemon.
//!
//! ```text
//! plumbd <config>
//! ```
//!
//! Config is plain `key = value` lines, no dependency spent on it:
//!
//! ```text
//! role   = court            # court | producer
//! holder = my-node          # what the chain calls you
//! listen = 127.0.0.1:9401   # court: where to accept
//! peer   = 127.0.0.1:9401   # producer: where the court is (repeatable)
//! bound  = 65536            # largest record value this deployment accepts
//! chain  = ledger/founding.tlv   # optional: replay this founding chain
//! ```
//!
//! The producer role sends the demo triangle claim and exits — it
//! exists so two machines can prove the seam. Real producers are
//! kernels attached through the SDK.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use datum::plumbd;
use datum::reward::RewardBook;
use isthmus::deed::Ledger;
use isthmus::layout::Layout;

struct Config {
    role: String,
    holder: String,
    listen: String,
    peers: Vec<String>,
    bound: usize,
    chain: Option<String>,
    snapshot: Option<String>,
    snapshot_secs: u64,
    fed_listen: Option<String>,
    fed_peers: Vec<String>,
    fed_secs: u64,
    require_signatures: bool,
    seed: Option<String>,
}

fn parse(text: &str) -> Config {
    let mut config = Config {
        role: "court".into(),
        holder: "plumbd".into(),
        listen: "127.0.0.1:9401".into(),
        peers: Vec::new(),
        bound: 1 << 16,
        chain: None,
        snapshot: None,
        snapshot_secs: 10,
        fed_listen: None,
        fed_peers: Vec::new(),
        fed_secs: 10,
        require_signatures: false,
        seed: None,
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim().to_owned());
        match key {
            "role" => config.role = value,
            "holder" => config.holder = value,
            "listen" => config.listen = value,
            "peer" => config.peers.push(value),
            "bound" => {
                if let Ok(n) = value.parse() {
                    config.bound = n;
                }
            }
            "chain" => config.chain = Some(value),
            "snapshot" => config.snapshot = Some(value),
            "snapshot_secs" => {
                if let Ok(n) = value.parse() {
                    config.snapshot_secs = n;
                }
            }
            "require_signatures" => config.require_signatures = value == "true",
            "seed" => config.seed = Some(value),
            "fed_listen" => config.fed_listen = Some(value),
            "fed_peer" => config.fed_peers.push(value),
            "fed_secs" => {
                if let Ok(n) = value.parse() {
                    config.fed_secs = n;
                }
            }
            _ => eprintln!("plumbd: unknown config key ignored: {key}"),
        }
    }
    config
}

fn ledger_for(config: &Config) -> Ledger {
    if let Some(path) = &config.chain {
        match std::fs::read(path) {
            Ok(bytes) => match isthmus::deed::chain::decode(&bytes) {
                Ok(acts) => return Ledger::replay(Layout::founding(), acts),
                Err(e) => {
                    eprintln!("plumbd: chain at {path} is malformed ({e:?}); refusing to invent an authority");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("plumbd: chain at {path} unreadable ({e}); refusing to invent an authority");
                std::process::exit(1);
            }
        }
    }
    // No chain named: a fresh edge with the founding encumbrances and
    // a deed for this holder, so declarations carry a real range.
    let mut ledger = Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    if ledger.issue(&config.holder, 16).is_err() {
        eprintln!("plumbd: could not issue a local deed; declarations will carry no range");
    }
    ledger
}

/// The demo claim a producer sends: the 3-orb triangle, enveloped.
fn demo_envelope() -> Option<Vec<u8>> {
    let shape = datum::onramp::shape_from_edges(
        3,
        [
            (0, 1, assay::whole(1)),
            (1, 2, assay::whole(1)),
            (0, 2, assay::whole(1)),
        ],
    )
    .ok()?;
    let body = datum::onramp::shape_body(0, shape).ok()?;
    let mut wire = Vec::new();
    isthmus::work::put_shape_claim(&body, &mut wire).ok()?;
    Some(wire)
}

/// 64 hex chars -> 32 bytes; anything else is None.
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
            eprintln!("usage: plumbd <config>");
            std::process::exit(2);
        }
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("plumbd: config at {path} unreadable: {e}");
            std::process::exit(2);
        }
    };
    let config = parse(&text);
    let layout = Layout::founding();
    let ledger = ledger_for(&config);

    match config.role.as_str() {
        "court" => {
            let listener = match TcpListener::bind(&config.listen) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("plumbd: cannot listen on {}: {e}", config.listen);
                    std::process::exit(1);
                }
            };
            let book = Arc::new(Mutex::new(RewardBook::new()));
            let service = datum::court_service::ServiceConfig {
                snapshot: config.snapshot.clone().map(std::path::PathBuf::from),
                snapshot_secs: config.snapshot_secs,
                fed_listen: config.fed_listen.clone(),
                fed_peers: config.fed_peers.clone(),
                fed_secs: config.fed_secs,
            };
            let _service = match datum::court_service::start(&service, &book) {
                Ok((handle, restored)) => {
                    if restored > 0 {
                        println!("plumbd: resumed {restored} act(s) from snapshot");
                    }
                    Some(handle)
                }
                Err(e) => {
                    eprintln!(
                        "plumbd: snapshot is corrupt ({e:?}); refusing to start with a forgotten book"
                    );
                    std::process::exit(1);
                }
            };
            println!(
                "plumbd: court '{}' listening on {}",
                config.holder, config.listen
            );
            if config.require_signatures {
                println!("plumbd: signature enforcement ON (S4)");
            }
            let rules = plumbd::SessionRules {
                holder: config.holder.clone(),
                bound: config.bound,
                enforce: config.require_signatures,
            };
            let err = plumbd::serve(
                &listener,
                &layout,
                &ledger,
                &rules,
                &book,
                |report| {
                    println!(
                        "plumbd: session closed — credited {}, refused {}, skipped {}",
                        report.credited, report.refused, report.skipped
                    );
                },
            );
            eprintln!("plumbd: listener failed: {err}");
            std::process::exit(1);
        }
        "producer" => {
            let Some(envelope) = demo_envelope() else {
                eprintln!("plumbd: could not build the demo claim");
                std::process::exit(1);
            };
            if config.peers.is_empty() {
                eprintln!("plumbd: producer needs at least one `peer =` line");
                std::process::exit(2);
            }
            let key = config.seed.as_deref().map(|hex| {
                match seed_from_hex(hex) {
                    Some(seed) => sig::Keypair::from_seed(seed),
                    None => {
                        eprintln!("plumbd: seed must be 64 hex chars");
                        std::process::exit(2);
                    }
                }
            });
            for peer in &config.peers {
                let result = match &key {
                    Some(key) => plumbd::produce_signed(
                        peer.as_str(),
                        &layout,
                        &ledger,
                        &config.holder,
                        config.bound,
                        std::slice::from_ref(&envelope),
                        key,
                    ),
                    None => plumbd::produce(
                        peer.as_str(),
                        &layout,
                        &ledger,
                        &config.holder,
                        config.bound,
                        std::slice::from_ref(&envelope),
                    ),
                };
                match result {
                    Ok(sent) => println!("plumbd: sent {sent} envelope(s) to {peer}"),
                    Err(e) => {
                        eprintln!("plumbd: producing to {peer} failed: {e:?}");
                        std::process::exit(1);
                    }
                }
            }
        }
        other => {
            eprintln!("plumbd: unknown role '{other}' (court | producer)");
            std::process::exit(2);
        }
    }
}
