//! `plumbd` — the node daemon.
//!
//! ```text
//! plumbd <config>
//! plumbd keygen <path>
//! plumbd tls-cert <holder> <cert-path> <key-path>
//! plumbd status <dir> [interval_secs]
//! ```
//!
//! `keygen` draws a fresh identity from OS entropy and writes ONLY the
//! seed to `<path>` (mode 0600, refuses to overwrite). Nothing in this
//! binary's participant-facing path accepts a hand-chosen or
//! repeated-digit seed as a substitute — a role that needs a key reads
//! it from a file made this way (`seed_file =`), never pastes it into
//! a config that might be shared or committed.
//!
//! `tls-cert` draws a fresh self-signed Ed25519 certificate (P4,
//! `IS-6/6`) and writes the cert and key DER to two paths, printing
//! the fingerprint a `certify =` genesis line — or a live `Act::Certify`
//! a court records of itself at startup — should carry.
//!
//! `status` is continuous monitoring, compiled: scans every `.conf`
//! in `<dir>`, probes each listener with a real bounded TCP connect
//! (not a pidfile check), and decodes each court's `.xdct` snapshot
//! and the shared chain — real economic and ledger state, never a
//! log line pattern-matched into looking like one. Repaints every
//! `interval_secs` (default 2) until killed.
//!
//! Config is plain `key = value` lines, no dependency spent on it:
//!
//! ```text
//! role      = court            # court | producer
//! holder    = my-node          # what the chain calls you
//! listen    = 127.0.0.1:9401   # court: where to accept
//! peer      = 127.0.0.1:9401   # producer: where the court is (repeatable)
//! bound     = 65536            # largest record value this deployment accepts
//! chain     = ledger/founding.tlv   # optional: replay this founding chain
//! seed_file = ident/my-node.seed    # from `plumbd keygen` — never inline
//! register  = true                  # court: accept live registration (P2)
//! max_connections         = 256     # court: total sessions held at once, 0=unwalled
//! max_connections_per_ip  = 16      # court: sessions held from one IP, 0=unwalled
//! handshake_deadline_secs = 10      # court: drop a silent connection, 0=unwalled
//! tls          = true                    # court: encrypt the wire (P4)
//! tls_cert_file = ident/my-node.cert.der  # from `plumbd tls-cert`
//! tls_key_file  = ident/my-node.key.der   # from `plumbd tls-cert`
//! court        = court-a          # client-facing roles: whose Act::Certify to pin
//! ```
//!
//! A client-facing role (producer, client, solver, witness, join,
//! carrier) dials over TLS automatically whenever BOTH `court =` and
//! `chain =` are set and the named holder has a live `Act::Certify`
//! on that chain — the fingerprint comes from the chain, never from a
//! config line, because the chain is the whole trust anchor here. No
//! `Act::Certify` on record means no TLS: a stranger with a stale
//! chain talks plaintext rather than trusting a fingerprint it never
//! actually read.
//!
//! The producer role sends the demo triangle claim and exits — it
//! exists so two machines can prove the seam. Real producers are
//! kernels attached through the SDK.
//!
//! `role = join` is how a STRANGER gets onto a live network in one
//! command: point `seed_file =` at a path (generated on the spot if
//! it does not exist yet) and `peer =` at a court running
//! `register = true`, and run it. One connection proves possession of
//! the fresh key, registers it live, and sends a proof-of-life claim
//! — no operator, no restart, no hand-edited genesis config.

use std::net::{TcpListener, TcpStream};
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
    register: bool,
    seed: Option<String>,
    seed_file: Option<String>,
    market: Option<String>,
    epoch_label: Option<String>,
    demo: String,
    upstream: Option<String>,
    every: u64,
    start_n: u32,
    step: u32,
    out: Option<String>,
    grants: Vec<String>,
    binds: Vec<(String, String)>,
    declares: Vec<String>,
    max_connections: usize,
    max_connections_per_ip: usize,
    handshake_deadline_secs: u64,
    tls: bool,
    tls_cert_file: Option<String>,
    tls_key_file: Option<String>,
    court: Option<String>,
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
        register: false,
        seed: None,
        seed_file: None,
        market: None,
        epoch_label: None,
        demo: "triangle".into(),
        upstream: None,
        every: 5,
        start_n: 3,
        step: 1,
        out: None,
        grants: Vec::new(),
        binds: Vec::new(),
        declares: Vec::new(),
        // Real defaults, not "off": a court run bare, with no wall
        // config at all, still has one. `0` disables a given wall for
        // deployments (the simnet, load tests) that need to say so
        // explicitly.
        max_connections: 256,
        max_connections_per_ip: 16,
        handshake_deadline_secs: 10,
        tls: false,
        tls_cert_file: None,
        tls_key_file: None,
        court: None,
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
            "register" => config.register = value == "true",
            "max_connections" => {
                if let Ok(n) = value.parse() {
                    config.max_connections = n;
                }
            }
            "max_connections_per_ip" => {
                if let Ok(n) = value.parse() {
                    config.max_connections_per_ip = n;
                }
            }
            "handshake_deadline_secs" => {
                if let Ok(n) = value.parse() {
                    config.handshake_deadline_secs = n;
                }
            }
            "tls" => config.tls = value == "true",
            "tls_cert_file" => config.tls_cert_file = Some(value),
            "tls_key_file" => config.tls_key_file = Some(value),
            "court" => config.court = Some(value),
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
            "seed_file" => config.seed_file = Some(value),
            "market" => config.market = Some(value),
            "epoch_label" => config.epoch_label = Some(value),
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

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The seed feeding this role's key: a `plumbd keygen`-made FILE
/// (never round-tripped through a config a person might commit or
/// paste), or — for tests and fixtures only — inline hex. Either
/// source, once named, is validated here: a malformed value is a
/// refusal, never a silent fall-through to keyless. `None` means
/// neither was configured at all.
fn resolve_seed(config: &Config) -> Option<[u8; 32]> {
    if let Some(path) = &config.seed_file {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("plumbd: seed_file {path} unreadable: {e}");
            std::process::exit(2);
        });
        return Some(seed_from_hex(text.trim()).unwrap_or_else(|| {
            eprintln!("plumbd: seed_file {path} does not hold 64 hex chars");
            std::process::exit(2);
        }));
    }
    config.seed.as_deref().map(|hex| {
        seed_from_hex(hex).unwrap_or_else(|| {
            eprintln!("plumbd: seed must be 64 hex chars");
            std::process::exit(2);
        })
    })
}

/// P4 — the fingerprint a client-facing role dials over TLS, IF the
/// named court has one on record. The chain is the whole trust
/// anchor: no `court =`, no chain, or no `Act::Certify` on it each
/// mean the same thing — plaintext, not a guess at a fingerprint
/// nobody actually read.
fn expected_fingerprint(config: &Config, ledger: &Ledger) -> Option<[u8; 32]> {
    let court = config.court.as_deref()?;
    ledger.fingerprint_of(court)
}

/// Why `keygen` could not produce or persist an identity.
#[derive(Debug)]
enum KeygenBroken {
    NoEntropy,
    AlreadyExists(String),
    Io(String),
}

impl std::fmt::Display for KeygenBroken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeygenBroken::NoEntropy => write!(f, "the operating system's entropy source refused"),
            KeygenBroken::AlreadyExists(path) => write!(
                f,
                "{path} already holds an identity — refusing to overwrite it"
            ),
            KeygenBroken::Io(msg) => write!(f, "{msg}"),
        }
    }
}

/// Draw a fresh identity from OS entropy and persist ONLY the seed —
/// no static/hardcoded/repeated-digit seed is ever an acceptable
/// substitute for this path. Refuses outright rather than clobbering
/// an existing identity file.
fn run_keygen(path: &std::path::Path) -> Result<[u8; 32], KeygenBroken> {
    let key = sig::Keypair::generate().map_err(|_| KeygenBroken::NoEntropy)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                KeygenBroken::AlreadyExists(path.display().to_string())
            }
            _ => KeygenBroken::Io(format!("{}: {e}", path.display())),
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file
            .metadata()
            .map_err(|e| KeygenBroken::Io(e.to_string()))?
            .permissions();
        perms.set_mode(0o600);
        file.set_permissions(perms)
            .map_err(|e| KeygenBroken::Io(e.to_string()))?;
    }
    use std::io::Write;
    writeln!(file, "{}", hex_encode(&key.seed())).map_err(|e| KeygenBroken::Io(e.to_string()))?;
    Ok(key.public())
}

fn main() {
    let first = std::env::args().nth(1);
    if first.as_deref() == Some("keygen") {
        let Some(out) = std::env::args().nth(2) else {
            eprintln!("usage: plumbd keygen <path>");
            std::process::exit(2);
        };
        match run_keygen(std::path::Path::new(&out)) {
            Ok(public) => {
                println!("plumbd: identity written to {out} (mode 0600) — do not share this file");
                println!("plumbd: public identity {}", hex_encode(&public));
            }
            Err(e) => {
                eprintln!("plumbd: keygen failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if first.as_deref() == Some("tls-cert") {
        let mut args = std::env::args().skip(2);
        let (Some(holder), Some(cert_path), Some(key_path)) =
            (args.next(), args.next(), args.next())
        else {
            eprintln!("usage: plumbd tls-cert <holder> <cert-path> <key-path>");
            std::process::exit(2);
        };
        let identity = datum::tls::generate_identity(&holder).unwrap_or_else(|e| {
            eprintln!("plumbd: tls-cert failed: {e}");
            std::process::exit(1);
        });
        for (path, bytes) in [(&cert_path, &identity.cert_der), (&key_path, &identity.key_der)] {
            if let Err(e) = std::fs::write(path, bytes) {
                eprintln!("plumbd: cannot write {path}: {e}");
                std::process::exit(1);
            }
        }
        println!("plumbd: certificate written to {cert_path}, key to {key_path}");
        println!(
            "plumbd: fingerprint {} — a court records this of itself at startup (P4)",
            hex_encode(&identity.fingerprint())
        );
        return;
    }
    if first.as_deref() == Some("status") {
        let Some(dir) = std::env::args().nth(2) else {
            eprintln!("usage: plumbd status <dir> [interval_secs]");
            std::process::exit(2);
        };
        let interval = std::env::args()
            .nth(3)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2)
            .max(1);
        run_status_loop(std::path::Path::new(&dir), interval);
        return;
    }
    let path = match first {
        Some(p) => p,
        None => {
            eprintln!("usage: plumbd <config> | plumbd keygen <path> | plumbd tls-cert <holder> <cert-path> <key-path> | plumbd status <dir> [interval_secs]");
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
            let mut ledger = ledger;
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
            // R3 — epochs are LIVE: the court opens one at start if
            // none is open, so bind windows can actually bite.
            if let Ok(mut guard) = book.lock() {
                if guard.open_epoch().is_none() {
                    let label = config
                        .epoch_label
                        .clone()
                        .unwrap_or_else(|| format!("court {} session epoch", config.holder));
                    match guard.open_epoch_named(label) {
                        Ok(epoch) => println!("plumbd: epoch {epoch} open"),
                        Err(e) => eprintln!("plumbd: epoch refused: {e:?}"),
                    }
                }
            }
            // R1/P5 — the native market: `market = dihedral` posts a
            // genuine theorem (P5's corpus: `bab = aa` in the dihedral
            // group of order 6, a real, citable instance of the
            // defining relation `bab^-1 = a^-1`) as an SQ4 conjecture,
            // served on the wire (tag 85 out, receipts back on 81).
            // Needs the court's seed to sign receipts.
            let market = match config.market.as_deref() {
                Some("dihedral") => {
                    let Some(seed) = resolve_seed(&config) else {
                        eprintln!("plumbd: market needs `seed_file =` or `seed =` to sign receipts");
                        std::process::exit(2);
                    };
                    let (_, conjecture) = datum::corpus::dihedral_conjecture().unwrap_or_else(|e| {
                        eprintln!("plumbd: dihedral corpus failed to compile: {e:?}");
                        std::process::exit(1);
                    });
                    let query = datum::query::Query {
                        poser: config.holder.clone(),
                        shape: vec![2, 3],
                        domain_tag: isthmus::work::SHAPE_CLAIM_TAG,
                        guarantee: datum::query::Guarantee::Rederivation,
                        statement: conjecture.encode(),
                    };
                    println!(
                        "plumbd: native market open — dihedral (bab = aa in D3), query {}",
                        {
                            let id = query.query_id();
                            format!("{:02x}{:02x}{:02x}{:02x}…", id[0], id[1], id[2], id[3])
                        }
                    );
                    Some(std::sync::Arc::new(plumbd::MarketPost {
                        bounty: datum::bounty::Bounty {
                            query_id: query.query_id(),
                            // The dihedral corpus's own derivation
                            // spends ~464 fuel and ~8.7KB — measured,
                            // not guessed; real headroom without
                            // inflating the yield rebate the way
                            // assay::complex::DEFAULT_FUEL's
                            // 1,000,000 ceiling would.
                            max_fuel: 2_000,
                            max_bytes: 10_000,
                            base: 1_000,
                            per_saved_fuel: 10,
                            per_saved_byte: 3,
                        },
                        query,
                        court: config.holder.clone(),
                        key: sig::Keypair::from_seed(seed),
                    }))
                }
                Some(other) => {
                    eprintln!("plumbd: unknown market '{other}' (dihedral)");
                    std::process::exit(2);
                }
                None => None,
            };
            if config.register {
                println!("plumbd: live registration OPEN (P2) — an unbound key that proves possession gets a deed and a bind, no restart");
            }
            // P4 — a court that turns TLS on certifies ITSELF: it
            // already holds this ledger, so proving possession over a
            // wire challenge (P2's proof) is not the question — the
            // question is just "does the chain know this fingerprint
            // yet," and the court is the one party that can answer
            // that by simply recording it.
            let tls_server_config = if config.tls {
                let (Some(cert_path), Some(key_path)) =
                    (&config.tls_cert_file, &config.tls_key_file)
                else {
                    eprintln!("plumbd: tls needs `tls_cert_file =` and `tls_key_file =` — see `plumbd tls-cert`");
                    std::process::exit(2);
                };
                let cert_der = std::fs::read(cert_path).unwrap_or_else(|e| {
                    eprintln!("plumbd: tls_cert_file {cert_path} unreadable: {e}");
                    std::process::exit(2);
                });
                let key_der = std::fs::read(key_path).unwrap_or_else(|e| {
                    eprintln!("plumbd: tls_key_file {key_path} unreadable: {e}");
                    std::process::exit(2);
                });
                let identity = datum::tls::Identity { cert_der, key_der };
                let fingerprint = identity.fingerprint();
                if ledger.fingerprint_of(&config.holder) != Some(fingerprint) {
                    ledger.record(isthmus::deed::Act::Certify {
                        holder: config.holder.clone(),
                        fingerprint,
                    });
                    if let Some(path) = &config.chain {
                        let _ = datum::registration::persist_chain_atomic(
                            std::path::Path::new(path),
                            &ledger,
                        );
                    }
                }
                println!(
                    "plumbd: TLS ON (P4) — certified fingerprint {}",
                    hex_encode(&fingerprint)
                );
                let server_config = datum::tls::server_config(&identity).unwrap_or_else(|e| {
                    eprintln!("plumbd: tls setup failed: {e}");
                    std::process::exit(1);
                });
                Some(Arc::new(server_config))
            } else {
                None
            };
            let rules = plumbd::SessionRules {
                holder: config.holder.clone(),
                bound: config.bound,
                enforce: config.require_signatures,
                market,
                register: config.register,
                chain_path: config.chain.clone().map(std::path::PathBuf::from),
                max_total_connections: config.max_connections,
                max_connections_per_ip: config.max_connections_per_ip,
                handshake_deadline: (config.handshake_deadline_secs > 0)
                    .then(|| std::time::Duration::from_secs(config.handshake_deadline_secs)),
                connections: Arc::new(Mutex::new(plumbd::ConnectionCounts::default())),
                tls: tls_server_config,
            };
            let ledger = Arc::new(Mutex::new(ledger));
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
            let key = resolve_seed(&config).map(sig::Keypair::from_seed);
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
                expected_fingerprint(&config, &ledger),
                |forwarded| println!("plumbd: carried session — {forwarded} frame(s) forwarded, none read"),
            );
            eprintln!("plumbd: carrier failed: {err}");
            std::process::exit(1);
        }
        "client" => {
            let Some(seed) = resolve_seed(&config) else {
                eprintln!("plumbd: client needs `seed_file =` or `seed =` — the network is signed");
                std::process::exit(2);
            };
            if config.peers.is_empty() {
                eprintln!("plumbd: client needs at least one `peer =` line");
                std::process::exit(2);
            }
            let key = sig::Keypair::from_seed(seed);
            let tls_fingerprint = expected_fingerprint(&config, &ledger);
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
                    match plumbd::produce_signed_over_tls(
                        peer.as_str(),
                        &layout,
                        &ledger,
                        &config.holder,
                        config.bound,
                        std::slice::from_ref(&envelope),
                        &key,
                        tls_fingerprint,
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
        "solver" => {
            let Some(seed) = resolve_seed(&config) else {
                eprintln!("plumbd: solver needs `seed_file =` or `seed =`");
                std::process::exit(2);
            };
            let Some(peer) = config.peers.first() else {
                eprintln!("plumbd: solver needs a `peer =` line");
                std::process::exit(2);
            };
            let key = sig::Keypair::from_seed(seed);
            // P5 — a real proof: the shortest derivation this
            // presentation licenses for `bab = aa`, the dihedral
            // group's own defining relation.
            let (compiled, _) = datum::corpus::dihedral_conjecture().unwrap_or_else(|e| {
                eprintln!("plumbd: dihedral corpus failed to compile: {e:?}");
                std::process::exit(1);
            });
            let witness = datum::corpus::dihedral_derivation(&compiled).unwrap_or_else(|e| {
                eprintln!("plumbd: dihedral derivation failed: {e:?}");
                std::process::exit(1);
            });
            let target = (|| {
                let bab = compiled.word(b"bab")?;
                let aa = compiled.word(b"aa")?;
                compiled.target(bab, aa)
            })()
            .unwrap_or_else(|e| {
                eprintln!("plumbd: dihedral target failed: {e:?}");
                std::process::exit(1);
            });
            let body = assay::complex::ProofClaim {
                transport: 1,
                complex: compiled.complex.clone(),
                dim: 1,
                target,
                witness,
                deps: Vec::new(),
            }
            .encode();
            match plumbd::solve_market(
                peer.as_str(),
                &layout,
                &ledger,
                &config.holder,
                config.bound,
                &body,
                &key,
                expected_fingerprint(&config, &ledger),
            ) {
                Ok((query, receipt)) => {
                    println!(
                        "plumbd: solved natively — query {}…, receipt axes {:?}, epoch {}",
                        query.query_id().first().map(|b| format!("{b:02x}")).unwrap_or_default(),
                        receipt.receipt.axes,
                        receipt.receipt.epoch,
                    );
                }
                Err(e) => {
                    eprintln!("plumbd: solving failed: {e:?}");
                    std::process::exit(1);
                }
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
            let key = resolve_seed(&config).map(sig::Keypair::from_seed);
            for peer in &config.peers {
                match plumbd::witness_to(
                    peer.as_str(),
                    &layout,
                    &ledger,
                    &config.holder,
                    config.bound,
                    std::slice::from_ref(&witness),
                    key.as_ref(),
                    expected_fingerprint(&config, &ledger),
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
        "join" => {
            let Some(peer) = config.peers.first() else {
                eprintln!("plumbd: join needs a `peer =` line — the court to join");
                std::process::exit(2);
            };
            let Some(path) = config.seed_file.as_deref() else {
                eprintln!("plumbd: join needs `seed_file =` — the identity to generate or reuse");
                std::process::exit(2);
            };
            let path_ref = std::path::Path::new(path);
            if !path_ref.exists() {
                match run_keygen(path_ref) {
                    Ok(public) => println!(
                        "plumbd: no identity at {path} yet — generated one ({})",
                        hex_encode(&public)
                    ),
                    Err(e) => {
                        eprintln!("plumbd: keygen failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            let Some(seed) = resolve_seed(&config) else {
                eprintln!("plumbd: join could not read the identity it just ensured exists");
                std::process::exit(1);
            };
            let key = sig::Keypair::from_seed(seed);
            let Some(envelope) = demo_envelope(&config.demo) else {
                eprintln!("plumbd: could not build the proof-of-life claim");
                std::process::exit(1);
            };
            match plumbd::register_and_produce(
                peer.as_str(),
                &layout,
                &config.holder,
                config.bound,
                &key,
                &envelope,
                expected_fingerprint(&config, &ledger),
            ) {
                Ok(outcome) => {
                    println!(
                        "plumbd: joined as '{}' — deed [{}, {}], epoch window [{}, {}]",
                        config.holder,
                        outcome.low,
                        outcome.high,
                        outcome.from_epoch,
                        outcome.until_epoch
                    );
                    println!("plumbd: sent the proof-of-life claim on the same connection");
                }
                Err(e) => {
                    eprintln!("plumbd: join failed: {e:?} — the court may not have `register = true` set, or the holder/key is already taken");
                    std::process::exit(1);
                }
            }
        }
        other => {
            eprintln!("plumbd: unknown role '{other}' (court | producer | carrier | client | solver | witness | genesis | join)");
            std::process::exit(2);
        }
    }
}

/// P7 — continuous monitoring, compiled: reads what is actually
/// TRUE (a live TCP reachability probe, a decoded `.xdct` reward-book
/// snapshot, the decoded chain) rather than pattern-matching log
/// text. Loops until killed (Ctrl-C); there is nothing to clean up.
fn run_status_loop(dir: &std::path::Path, interval_secs: u64) {
    use std::io::Write;
    loop {
        let report = render_status_report(dir);
        // Clear screen, home cursor — a plain repaint, not a scrolling
        // log; ANSI, which every terminal this binary ships for reads.
        print!("\x1B[2J\x1B[H{report}");
        // Rust block-buffers stdout when it is not a TTY (redirected
        // to a file, piped, watched by another process) — an
        // unflushed monitor is a contradiction, so flush every frame
        // rather than trust the next print to eventually force it.
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

/// One row: what a `.conf` file in the directory claims to be, and
/// whether its listener is ACTUALLY answering right now.
struct NodeRow {
    name: String,
    role: String,
    listen: Option<String>,
    up: Option<bool>,
}

fn probe_tcp(addr: &str) -> Option<bool> {
    use std::net::ToSocketAddrs;
    let target = addr.to_socket_addrs().ok()?.next()?;
    Some(TcpStream::connect_timeout(&target, std::time::Duration::from_millis(300)).is_ok())
}

/// A read-only `key = value` lookup, deliberately separate from the
/// shared `parse()`: that function WARNS on every key it does not
/// recognize, which is correct for loading a role this binary is
/// about to run and wrong for a status scan that reads configs
/// belonging to a DIFFERENT binary (the gateway) it never loads.
fn quiet_lookup(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.split('#').next().unwrap_or("").trim();
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then(|| v.trim().to_owned())
    })
}

#[must_use]
fn render_status_report(dir: &std::path::Path) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "plumb status — {}  (refresh; Ctrl-C to stop)\n\n",
        dir.display()
    ));

    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "conf"))
                .collect()
        })
        .unwrap_or_default();
    entries.sort();

    let mut rows = Vec::new();
    let mut snapshot_paths: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut chain_paths: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();

    for entry in &entries {
        let name = entry
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.display().to_string());
        let Ok(text) = std::fs::read_to_string(entry) else {
            continue;
        };
        // A quiet, read-only lookup — NOT the shared `parse()`, which
        // warns on every key it does not recognize. The gateway binary
        // has its OWN config shape (no `role =`, `facilitator =`
        // instead), so running it through plumbd's parser here would
        // print a stream of "unknown config key" noise for a config
        // this binary never actually loads.
        let role = match quiet_lookup(&text, "role") {
            Some(r) => r,
            None if quiet_lookup(&text, "facilitator").is_some() => "gateway".to_owned(),
            None => "court".to_owned(),
        };
        let listen = quiet_lookup(&text, "listen");
        let up = listen.as_deref().and_then(probe_tcp);
        if let Some(path) = quiet_lookup(&text, "snapshot") {
            snapshot_paths.push((name.clone(), std::path::PathBuf::from(path)));
        }
        if let Some(path) = quiet_lookup(&text, "chain") {
            chain_paths.insert(std::path::PathBuf::from(path));
        }
        rows.push(NodeRow {
            name,
            role,
            listen,
            up,
        });
    }

    out.push_str(&format!(
        "{:<12} {:<9} {:<22} {}\n",
        "NODE", "ROLE", "ADDR", "LIVE"
    ));
    for row in &rows {
        let addr = row.listen.as_deref().unwrap_or("-");
        let up = match row.up {
            Some(true) => "UP",
            Some(false) => "DOWN",
            None => "-",
        };
        out.push_str(&format!("{:<12} {:<9} {:<22} {}\n", row.name, row.role, addr, up));
    }

    out.push_str("\nCOURT BOOKS (decoded from .xdct snapshots — real credit state)\n");
    if snapshot_paths.is_empty() {
        out.push_str("  (none configured)\n");
    }
    for (name, path) in &snapshot_paths {
        match datum::court_store::load(path) {
            Ok(book) => out.push_str(&format!(
                "  {:<12} acts={:<6} distinct_work={:<6} credited={}\n",
                name,
                book.act_len(),
                book.seen().len(),
                book.total()
            )),
            Err(_) if !path.exists() => {
                out.push_str(&format!("  {name:<12} (no snapshot written yet)\n"));
            }
            Err(e) => out.push_str(&format!("  {name:<12} snapshot unreadable: {e:?}\n")),
        }
    }

    out.push_str("\nCHAIN (decoded — real ledger state, not a log line)\n");
    if chain_paths.is_empty() {
        out.push_str("  (none configured)\n");
    }
    for path in &chain_paths {
        match std::fs::read(path)
            .ok()
            .and_then(|bytes| isthmus::deed::chain::decode(&bytes).ok())
        {
            Some(acts) => {
                let ledger = Ledger::replay(Layout::founding(), acts);
                let live_holders = ledger.deeds().iter().filter(|d| d.live).count();
                let bound = ledger
                    .acts()
                    .iter()
                    .filter(|a| matches!(a, isthmus::deed::Act::Bind { .. }))
                    .count();
                let certified = ledger
                    .acts()
                    .iter()
                    .filter(|a| matches!(a, isthmus::deed::Act::Certify { .. }))
                    .count();
                out.push_str(&format!(
                    "  {:<40} acts={:<6} live_holders={:<4} binds={:<4} certifies(TLS)={}\n",
                    path.display(),
                    ledger.acts().len(),
                    live_holders,
                    bound,
                    certified
                ));
            }
            None => out.push_str(&format!("  {}: unreadable or not written yet\n", path.display())),
        }
    }

    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_config_parser_reads_what_the_simnet_writes() {
        let config = parse(
            "role = court\n\
             holder = court-a   # trailing comment\n\
             listen = 127.0.0.1:9501\n\
             peer = a:1\npeer = b:2\n\
             bound = 4096\n\
             require_signatures = true\n\
             market = theta\n\
             epoch_label = live\n\
             unknown_key = ignored\n",
        );
        assert_eq!(config.role, "court");
        assert_eq!(config.holder, "court-a", "comments strip");
        assert_eq!(config.peers, vec!["a:1", "b:2"], "peers repeat");
        assert_eq!(config.bound, 4096);
        assert!(config.require_signatures);
        assert_eq!(config.market.as_deref(), Some("theta"));
        assert_eq!(config.epoch_label.as_deref(), Some("live"));
    }

    #[test]
    fn seeds_are_sixty_four_hex_chars_or_nothing() {
        assert!(seed_from_hex(&"07".repeat(32)).is_some());
        assert_eq!(seed_from_hex(&"07".repeat(32)), Some([7u8; 32]));
        assert!(seed_from_hex("short").is_none());
        assert!(seed_from_hex(&"zz".repeat(32)).is_none(), "not hex");
        assert!(seed_from_hex(&"07".repeat(33)).is_none(), "too long");
    }

    fn scratch_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "plumbd-test-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    #[test]
    fn keygen_draws_real_entropy_and_the_saved_seed_restores_the_same_identity() {
        let dir = scratch_dir("keygen");
        let path_a = dir.join("a.seed");
        let path_b = dir.join("b.seed");
        let public_a = run_keygen(&path_a).expect("first keygen");
        let public_b = run_keygen(&path_b).expect("second keygen");
        assert_ne!(
            public_a, public_b,
            "two keygen calls must not produce the same identity — this is entropy, not a fixture"
        );

        let saved = std::fs::read_to_string(&path_a).expect("seed file readable");
        let seed = seed_from_hex(saved.trim()).expect("seed file holds 64 hex chars");
        assert_eq!(
            sig::Keypair::from_seed(seed).public(),
            public_a,
            "the saved seed restores the identity keygen reported"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path_a)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "identity file must not be group/world readable");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keygen_refuses_to_overwrite_an_existing_identity() {
        let dir = scratch_dir("keygen-clobber");
        let path = dir.join("node.seed");
        run_keygen(&path).expect("first keygen");
        assert!(
            matches!(run_keygen(&path), Err(KeygenBroken::AlreadyExists(_))),
            "a second keygen at the same path must refuse, not clobber"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_seed_prefers_a_keygen_file_over_inline_hex() {
        let dir = scratch_dir("resolve-seed");
        let path = dir.join("node.seed");
        let generated_public = run_keygen(&path).expect("keygen");

        let mut config = parse("role = client\n");
        config.seed = Some("11".repeat(32)); // present, but must lose to seed_file
        config.seed_file = Some(path.to_string_lossy().into_owned());
        let seed = resolve_seed(&config).expect("seed_file resolves");
        assert_eq!(
            sig::Keypair::from_seed(seed).public(),
            generated_public,
            "seed_file must take precedence over an inline seed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_seed_is_none_when_neither_source_is_configured() {
        let config = parse("role = client\n");
        assert_eq!(resolve_seed(&config), None);
    }

    #[test]
    fn quiet_lookup_reads_a_key_and_never_touches_stderr_on_unknown_ones() {
        let text = "role = court\nlisten = 127.0.0.1:9501  # trailing comment\nfacilitator = 0xDEAD\n";
        assert_eq!(quiet_lookup(text, "role"), Some("court".into()));
        assert_eq!(quiet_lookup(text, "listen"), Some("127.0.0.1:9501".into()));
        assert_eq!(quiet_lookup(text, "facilitator"), Some("0xDEAD".into()));
        assert_eq!(quiet_lookup(text, "nonexistent"), None);
    }

    #[test]
    fn status_labels_a_gatewayshaped_config_without_a_role_line() {
        // The gateway binary's config has no `role =` line at all;
        // `facilitator =` is the tell that distinguishes it from a
        // plain court config (which also defaults role-less to court
        // when plumbd itself is asked to run one).
        let gateway_text = "listen = 127.0.0.1:9801\nchain = x\ncourt = court-a\nfacilitator = 0xABC\n";
        assert!(quiet_lookup(gateway_text, "role").is_none());
        assert!(quiet_lookup(gateway_text, "facilitator").is_some());
    }

    #[test]
    fn render_status_report_reads_real_decoded_state_not_log_text() {
        let dir = scratch_dir("status-report");
        std::fs::write(
            dir.join("court-a.conf"),
            "role = court\nholder = court-a\nlisten = 127.0.0.1:1\nsnapshot = ident-does-not-exist.xdct\nchain = chain-does-not-exist.tlv\n",
        )
        .expect("write conf");
        let report = render_status_report(&dir);
        assert!(report.contains("court-a"), "the node shows up");
        assert!(report.contains("court"), "its role is read");
        assert!(
            report.contains("no snapshot written yet") || report.contains("unreadable"),
            "a missing snapshot is named, not silently skipped or panicked over"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
