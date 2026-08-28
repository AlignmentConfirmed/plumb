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
    demo: String,
    upstream: Option<String>,
    every: u64,
    start_n: u32,
    step: u32,
    out: Option<String>,
    grants: Vec<String>,
    binds: Vec<(String, String)>,
    declares: Vec<String>,
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
        demo: "triangle".into(),
        upstream: None,
        every: 5,
        start_n: 3,
        step: 1,
        out: None,
        grants: Vec::new(),
        binds: Vec::new(),
        declares: Vec::new(),
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
            "demo" => config.demo = value,
            "upstream" => config.upstream = Some(value),
            "every" => {
                if let Ok(n) = value.parse() {
                    config.every = n;
                }
            }
            "start_n" => {
                if let Ok(n) = value.parse() {
                    config.start_n = n;
                }
            }
            "step" => {
                if let Ok(n) = value.parse() {
                    config.step = n;
                }
            }
            "out" => config.out = Some(value),
            "grant" => config.grants.push(value),
            "bind" => {
                if let Some((who, seed)) = value.split_once(':') {
                    config.binds.push((who.trim().into(), seed.trim().into()));
                } else {
                    eprintln!("plumbd: bind wants `holder:seedhex`, got: {value}");
                }
            }
            "declare" => config.declares.push(value),
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

/// The demo claim a producer sends, enveloped: `triangle` (compiled
/// shape domain) or `hexagon` (declared domain — the universe rides
/// in the claim as data).
fn demo_envelope(which: &str) -> Option<Vec<u8>> {
    let body = match which {
        "hexagon" => datum::domains::demo_hexagon_claim(0).encode(),
        _ => {
            let shape = datum::onramp::shape_from_edges(
                3,
                [
                    (0, 1, assay::whole(1)),
                    (1, 2, assay::whole(1)),
                    (0, 2, assay::whole(1)),
                ],
            )
            .ok()?;
            datum::onramp::shape_body(0, shape).ok()?
        }
    };
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
            let registered = (0u64..256)
                .filter(|t| ledger.declaration_of(*t).is_some())
                .count();
            if registered > 0 {
                println!(
                    "plumbd: {registered} registered domain(s) resolved from chain state"
                );
            }
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
            let witnesses: plumbd::WitnessLog =
                Arc::new(Mutex::new(Vec::new()));
            let err = plumbd::serve(
                &listener,
                &layout,
                &ledger,
                &rules,
                &book,
                &witnesses,
                |report| {
                    println!(
                        "plumbd: session closed — credited {}, refused {}, skipped {}, witnessed {}",
                        report.credited, report.refused, report.skipped, report.witnessed
                    );
                },
            );
            eprintln!("plumbd: listener failed: {err}");
            std::process::exit(1);
        }
        "producer" => {
            let Some(envelope) = demo_envelope(&config.demo) else {
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
        "carrier" => {
            let Some(upstream) = config.upstream.clone() else {
                eprintln!("plumbd: carrier needs `upstream = host:port`");
                std::process::exit(2);
            };
            let listener = match TcpListener::bind(&config.listen) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("plumbd: cannot listen on {}: {e}", config.listen);
                    std::process::exit(1);
                }
            };
            println!(
                "plumbd: carrier '{}' on {} -> {} (forwards unread)",
                config.holder, config.listen, upstream
            );
            let err = plumbd::carry(
                &listener,
                &layout,
                &ledger,
                &config.holder,
                config.bound,
                upstream,
                |forwarded| println!("plumbd: carried session — {forwarded} frame(s) forwarded, none read"),
            );
            eprintln!("plumbd: carrier failed: {err}");
            std::process::exit(1);
        }
        "client" => {
            let Some(seed_hex) = config.seed.as_deref() else {
                eprintln!("plumbd: client needs `seed = <64 hex>` — the simnet is signed");
                std::process::exit(2);
            };
            let Some(seed) = seed_from_hex(seed_hex) else {
                eprintln!("plumbd: seed must be 64 hex chars");
                std::process::exit(2);
            };
            if config.peers.is_empty() {
                eprintln!("plumbd: client needs at least one `peer =` line");
                std::process::exit(2);
            }
            let key = sig::Keypair::from_seed(seed);
            let mut n = config.start_n.max(3);
            let mut lap: i64 = 1;
            // The bound-safe ceiling: past this, an envelope would
            // exceed a default court's record bound — so the client
            // LAPS, restarting n with a fresh charge. Every lap is
            // new structure; the record size never grows.
            let cap: u32 = 900;
            println!(
                "plumbd: client '{}' — fresh {}-cycle work every {}s, step {}, lap charge {}",
                config.holder, n, config.every, config.step, lap
            );
            loop {
                let body =
                    datum::domains::demo_cycle_claim_charged(n, lap, u64::from(n)).encode();
                let mut envelope = Vec::new();
                if isthmus::work::put_shape_claim(&body, &mut envelope).is_err() {
                    eprintln!("plumbd: could not frame the {n}-cycle");
                    std::process::exit(1);
                }
                for peer in &config.peers {
                    match plumbd::produce_signed(
                        peer.as_str(),
                        &layout,
                        &ledger,
                        &config.holder,
                        config.bound,
                        std::slice::from_ref(&envelope),
                        &key,
                    ) {
                        Ok(_) => println!("plumbd: {n}-cycle claim -> {peer}"),
                        Err(e) => println!("plumbd: {peer} unreachable ({e:?}); next round"),
                    }
                }
                n = n.saturating_add(config.step.max(1));
                if n > cap {
                    n = config.start_n.max(3);
                    lap = lap.saturating_add(1);
                    println!("plumbd: lap {lap} — fresh charges, bounded records");
                }
                std::thread::sleep(std::time::Duration::from_secs(config.every.max(1)));
            }
        }
        "witness" => {
            if config.peers.is_empty() {
                eprintln!("plumbd: witness needs at least one `peer =` line");
                std::process::exit(2);
            }
            // The demo witness: attest that the demo claim's envelope
            // crossed, against this node's chain as the observer.
            let Some(envelope) = demo_envelope(&config.demo) else {
                eprintln!("plumbd: could not build the demo subject");
                std::process::exit(1);
            };
            let chain_bytes = isthmus::deed::chain::encode(ledger.acts());
            let witness = isthmus::witness::Witness {
                arm: isthmus::witness::Arm::Replay,
                observer: isthmus::witness::Observer {
                    kind: 1, // a chain
                    identity: sig::envelope_hash(&chain_bytes),
                    revision: "IS-6/5".into(),
                    depth: 0,
                },
                subject: datum::witnessing::subject_of(&envelope),
                derivation: Vec::new(),
            };
            let key = config.seed.as_deref().and_then(seed_from_hex).map(sig::Keypair::from_seed);
            for peer in &config.peers {
                match plumbd::witness_to(
                    peer.as_str(),
                    &layout,
                    &ledger,
                    &config.holder,
                    config.bound,
                    std::slice::from_ref(&witness),
                    key.as_ref(),
                ) {
                    Ok(sent) => println!("plumbd: {sent} witness(es) on the record at {peer}"),
                    Err(e) => {
                        eprintln!("plumbd: witnessing to {peer} failed: {e:?}");
                        std::process::exit(1);
                    }
                }
            }
        }
        "genesis" => {
            let Some(out) = &config.out else {
                eprintln!("plumbd: genesis needs `out = <path>`");
                std::process::exit(2);
            };
            let mut genesis = Ledger::new(Layout::founding());
            genesis.encumber(1, 31, "ancestral", "founding registries");
            let mut issued = vec![config.holder.clone()];
            issued.extend(config.grants.iter().cloned());
            for holder in &issued {
                if genesis.issue(holder, 16).is_err() {
                    eprintln!("plumbd: no room to issue {holder}");
                    std::process::exit(1);
                }
            }
            for (holder, seed_hex) in &config.binds {
                let Some(seed) = seed_from_hex(seed_hex) else {
                    eprintln!("plumbd: bind seed for {holder} must be 64 hex chars");
                    std::process::exit(2);
                };
                let key = sig::Keypair::from_seed(seed);
                genesis.record(isthmus::deed::Act::Bind {
                    holder: holder.clone(),
                    scheme: sig::SCHEME_ED25519_BLAKE3,
                    key: key.public().to_vec(),
                    from_epoch: 0,
                    until_epoch: u64::MAX,
                });
            }
            for holder in &config.declares {
                let Some(tag) = genesis
                    .deeds()
                    .into_iter()
                    .find(|d| d.live && &d.holder == holder)
                    .map(|d| d.low())
                else {
                    eprintln!("plumbd: declare names unknown holder {holder}");
                    std::process::exit(2);
                };
                genesis.record(isthmus::deed::Act::Declare {
                    holder: holder.clone(),
                    tag,
                    definition: datum::domains::demo_hexagon_universe().encode(),
                });
            }
            let bytes = isthmus::deed::chain::encode(genesis.acts());
            if let Err(e) = std::fs::write(out, &bytes) {
                eprintln!("plumbd: cannot write {out}: {e}");
                std::process::exit(1);
            }
            println!(
                "plumbd: genesis written to {out} — {} act(s), {} holder(s), {} bind(s), {} declaration(s)",
                genesis.acts().len(),
                issued.len(),
                config.binds.len(),
                config.declares.len()
            );
        }
        other => {
            eprintln!("plumbd: unknown role '{other}' (court | producer | carrier | client | witness | genesis)");
            std::process::exit(2);
        }
    }
}
