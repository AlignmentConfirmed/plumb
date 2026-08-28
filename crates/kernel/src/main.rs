//! K3 — the kernel daemon: a derivation-finding producer.
//!
//! Attaches to a court over plain Plumbline (no HTTP, no TLS — a
//! court that requires TLS is a follow-on, not this binary's scope),
//! hears its query announcement (tag 85), derives a witness by
//! bounded traversal of the announced conjecture's own licensed
//! 1-cells (K2, `sdk::derivation`), submits the attested proof, and
//! takes the receipt (tag 81) — looping, forever, on its own clock.
//!
//! Built against `sdk` and the leaves (`isthmus`, `assay`, `sig`)
//! ONLY. There is no `datum` dependency anywhere in this crate's
//! manifest (see `tests/no_court_dependency.rs`) — a kernel joins a
//! court without linking it, which is the entire point of moving the
//! market vocabulary out of the court in K1.
//!
//! ```text
//! kernel.conf:
//!   holder = kernel-1
//!   chain = /path/to/chain.tlv
//!   peer = 127.0.0.1:9501
//!   seed_file = /path/to/kernel-1.seed
//!   budget = 100000          # optional, default 100000
//!   interval_secs = 5        # optional, default 5
//! ```

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use isthmus::deed::Ledger;
use isthmus::layout::{Layout, Tag};
use isthmus::session::{self, Step};

/// Generous enough for any record this binary sends or reads —
/// derivations run over small bounded universes (SQ3's `max_len`), so
/// the wire records around them are small too.
const RECORD_BOUND: usize = 1 << 20;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: kernel <config-file>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("kernel: cannot read {path}: {e}");
        std::process::exit(2);
    });
    let config = Config::parse(&text);

    let seed = config.resolve_seed().unwrap_or_else(|| {
        eprintln!("kernel: needs `seed_file =` or `seed =` — the network is signed");
        std::process::exit(2);
    });
    let key = sig::Keypair::from_seed(seed);

    let chain_bytes = std::fs::read(&config.chain).unwrap_or_else(|e| {
        eprintln!("kernel: cannot read chain {}: {e}", config.chain);
        std::process::exit(2);
    });
    let acts = isthmus::deed::chain::decode(&chain_bytes).unwrap_or_else(|e| {
        eprintln!("kernel: chain {} does not decode: {e:?}", config.chain);
        std::process::exit(2);
    });
    let ledger = Ledger::replay(Layout::founding(), acts);
    let layout = Layout::founding();

    // Query ids this kernel has already settled. A market re-announces
    // the same conjecture every session, so without this the kernel
    // would re-derive the identical proof forever — and the court's
    // (correct, content-addressed) anti-replay would refuse every
    // resubmission in silence, stalling the kernel on a receipt that
    // never comes. Remembering what is settled is the difference
    // between "idle, nothing new to earn" and a wedged loop (#43).
    let mut settled: HashSet<[u8; 32]> = HashSet::new();
    let mut idle_announced = false;
    loop {
        match derive_and_settle(&config, &layout, &ledger, &key, &mut settled) {
            Ok(Round::Earned(axes)) => {
                println!("kernel: derived and settled, credited axes {axes:?}");
                idle_announced = false;
            }
            Ok(Round::AlreadySettled) => {
                // Steady state: every posed query is already earned.
                // Say so ONCE, then idle quietly rather than spamming.
                if !idle_announced {
                    println!("kernel: every posed query already settled — idle, watching for new work");
                    idle_announced = true;
                }
            }
            Err(e) => {
                eprintln!("kernel: round refused: {e:?}");
                idle_announced = false;
            }
        }
        std::thread::sleep(Duration::from_secs(config.interval_secs));
    }
}

/// What one round accomplished.
enum Round {
    /// A fresh proof settled; these axes were credited.
    Earned(Vec<u128>),
    /// The announced query was already settled by this kernel — no
    /// resubmission attempted (it would only draw a replay refusal),
    /// no receipt awaited.
    AlreadySettled,
}

/// This binary's own configuration — a `.conf` file, same `key =
/// value` convention every plumbd role reads, but parsed here rather
/// than shared: the kernel does not link `datum`, and this parser is
/// a dozen lines, not worth a dependency to avoid duplicating.
struct Config {
    holder: String,
    chain: String,
    peer: String,
    seed_file: Option<String>,
    seed: Option<String>,
    budget: u64,
    interval_secs: u64,
}

impl Config {
    fn parse(text: &str) -> Self {
        let mut holder = None;
        let mut chain = None;
        let mut peer = None;
        let mut seed_file = None;
        let mut seed = None;
        let mut budget = 100_000u64;
        let mut interval_secs = 5u64;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim().to_string());
            match key {
                "holder" => holder = Some(value),
                "chain" => chain = Some(value),
                "peer" => peer = Some(value),
                "seed_file" => seed_file = Some(value),
                "seed" => seed = Some(value),
                "budget" => budget = value.parse().unwrap_or(budget),
                "interval_secs" => interval_secs = value.parse().unwrap_or(interval_secs),
                _ => eprintln!("kernel: unknown config key ignored: {key}"),
            }
        }
        let require = |field: Option<String>, name: &str| {
            field.unwrap_or_else(|| {
                eprintln!("kernel: config needs `{name} =`");
                std::process::exit(2);
            })
        };
        Self {
            holder: require(holder, "holder"),
            chain: require(chain, "chain"),
            peer: require(peer, "peer"),
            seed_file,
            seed,
            budget,
            interval_secs,
        }
    }

    /// The seed feeding this kernel's key: a `plumbd keygen`-made
    /// FILE (never round-tripped through a config a person might
    /// commit), or — for fixtures only — inline hex.
    fn resolve_seed(&self) -> Option<[u8; 32]> {
        if let Some(path) = &self.seed_file {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("kernel: seed_file {path} unreadable: {e}");
                std::process::exit(2);
            });
            return Some(seed_from_hex(text.trim()).unwrap_or_else(|| {
                eprintln!("kernel: seed_file {path} does not hold 64 hex chars");
                std::process::exit(2);
            }));
        }
        self.seed.as_deref().and_then(seed_from_hex)
    }
}

fn seed_from_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (slot, pair) in out.iter_mut().zip(0..32usize) {
        let byte = text.get(pair.saturating_mul(2)..pair.saturating_mul(2).saturating_add(2))?;
        *slot = u8::from_str_radix(byte, 16).ok()?;
    }
    Some(out)
}

/// Why one connect-derive-submit round refused.
///
/// Every field here is read — through `{e:?}` at the one call site
/// that reports a refusal to the operator, which dead-code analysis
/// does not count as a read.
#[derive(Debug)]
#[allow(dead_code)]
enum KernelRefused {
    /// The TCP round-trip itself failed.
    Io(std::io::Error),
    /// The wire went quiet, or handed back something this reader does
    /// not recognize as a well-formed record.
    Malformed,
    /// The court's declaration shares no revision with ours.
    NoSharedRevision,
    /// The posed query is not a conjecture (SQ4) — a plain-closure
    /// market asks for something a kernel does not attempt: finding
    /// ANY closing cycle is a different, unscoped problem. Named
    /// honestly rather than guessed at.
    NotAConjecture,
    /// No forward derivation closes the announced target at any
    /// dimension the universe declares — the theorem is not
    /// forward-derivable in this calculus.
    Unsolvable,
}

impl From<std::io::Error> for KernelRefused {
    fn from(e: std::io::Error) -> Self {
        KernelRefused::Io(e)
    }
}

/// One full round: connect, attach, hear the query, derive, submit,
/// take the receipt. A fresh connection every round — the reference
/// solver does the same, and a kernel with nothing queued yet has no
/// reason to hold a socket open between rounds.
fn derive_and_settle(
    config: &Config,
    layout: &Layout,
    ledger: &Ledger,
    key: &sig::Keypair,
    settled: &mut HashSet<[u8; 32]>,
) -> Result<Round, KernelRefused> {
    let mut stream = TcpStream::connect(&config.peer)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut buffer = Vec::new();

    // Attach: declare what we speak, under whichever tag our own
    // grant (if any) holds — an unattached kernel still declares,
    // under the substrate's own fallback tag.
    let ours = sdk::attach::declare(ledger, &config.holder, 1 << 16);
    let tag = sdk::grant::holdings(ledger, &config.holder)
        .first()
        .map(isthmus::deed::Deed::low)
        .unwrap_or(64);
    let wire = sdk::attach::wire(layout, tag, &ours).map_err(|_| KernelRefused::Malformed)?;
    stream.write_all(&wire)?;

    let (_, hello_frame) =
        read_record(&mut stream, &mut buffer, layout)?.ok_or(KernelRefused::Malformed)?;
    let theirs = isthmus::hello::Hello::decode(hello_frame.get(layout.header()..).unwrap_or(&[]))
        .map_err(|_| KernelRefused::Malformed)?;
    sdk::attach::agree(&ours, &theirs).map_err(|_| KernelRefused::NoSharedRevision)?;

    // The session challenge: attest the exact frame bytes we were
    // handed, proving possession of the key our declared holder binds.
    let (_, challenge_frame) =
        read_record(&mut stream, &mut buffer, layout)?.ok_or(KernelRefused::Malformed)?;
    let answer = key.attest(&challenge_frame);
    send_frame(&mut stream, layout, sdk::submit::ATTESTATION_TAG, &answer.encode())?;

    // The question, announced.
    let (qtag, qframe) =
        read_record(&mut stream, &mut buffer, layout)?.ok_or(KernelRefused::Malformed)?;
    if qtag != sdk::submit::QUERY_TAG {
        return Err(KernelRefused::Malformed);
    }
    let query = sdk::query::Query::decode(qframe.get(layout.header()..).unwrap_or(&[]))
        .map_err(|_| KernelRefused::Malformed)?;

    // Already earned this exact question? Then submitting again would
    // only draw the court's (correct) replay refusal, which it answers
    // with silence — so we would stall on a receipt that never comes.
    // Disconnect cleanly instead and idle until a NEW query appears.
    let query_id = query.query_id();
    if settled.contains(&query_id) {
        return Ok(Round::AlreadySettled);
    }

    let conjecture = sdk::query::Conjecture::decode(&query.statement)
        .map_err(|_| KernelRefused::NotAConjecture)?;

    // K2 (#42): DERIVE by the court's own algebra, not a graph walk.
    // No lemmas cited — a fresh kernel builds from the announced
    // conjecture alone, nothing it was handed.
    let (dim, witness) = solve_conjecture(&conjecture.universe, &conjecture.target, config.budget)
        .ok_or(KernelRefused::Unsolvable)?;
    let proof = assay::complex::ProofClaim {
        transport: 1,
        complex: conjecture.universe,
        dim,
        target: conjecture.target,
        witness,
        deps: Vec::new(),
    };
    let body = proof.encode();

    // The answer, enveloped and attested like any claim.
    let envelope = sdk::submit::shape(&body).map_err(|_| KernelRefused::Malformed)?;
    stream.write_all(&envelope)?;
    let attestation = key.attest(&envelope);
    send_frame(
        &mut stream,
        layout,
        sdk::submit::ATTESTATION_TAG,
        &attestation.encode(),
    )?;

    // The receipt, on the same wire — verified against chain state
    // alone before this kernel believes it earned anything.
    let (rtag, rframe) =
        read_record(&mut stream, &mut buffer, layout)?.ok_or(KernelRefused::Malformed)?;
    if rtag != sdk::submit::RECEIPT_TAG {
        return Err(KernelRefused::Malformed);
    }
    let value = rframe.get(layout.header()..).unwrap_or(&[]);
    // Split by parsing the receipt forward, not by subtracting a fixed
    // attestation width — a scheme ≥ 0x02 signature is variable-length.
    let (receipt, consumed) =
        sdk::receipt::Receipt::decode_prefix(value).map_err(|_| KernelRefused::Malformed)?;
    let receipt_attestation =
        sig::Attestation::decode(value.get(consumed..).unwrap_or(&[])).map_err(|_| KernelRefused::Malformed)?;
    let signed = sdk::receipt::SignedReceipt {
        receipt,
        attestation: receipt_attestation,
    };
    sdk::receipt::verify(&signed, ledger).map_err(|_| KernelRefused::Malformed)?;

    settled.insert(query_id);
    Ok(Round::Earned(signed.receipt.axes))
}

/// Derive a witness for `target` by the court's own incidence algebra
/// (#42) — the linear-algebra replacement for the old dim-1 graph
/// walk. Returns the winning `(dim, witness)`.
///
/// `solve_forward` is `min Σx s.t. ∂ₖx = target, x ≥ 0`: the
/// cost-minimal FORWARD chain (the non-negativity keeps every step a
/// licensed forward rewrite, SQ3), which is exactly the profit the O1
/// rebate pays for. The dimension isn't on the wire, so the kernel
/// tries each dimension the universe declares, lowest first, and takes
/// the first witness that independently re-verifies under `fuel` —
/// never trusting the solver's own say-so. This spans dim ≥ 2 and
/// multi-term boundaries a graph walk cannot even pose.
fn solve_conjecture(
    universe: &assay::complex::DeclaredComplex,
    target: &[(u32, assay::Exact)],
    fuel: u64,
) -> Option<(u32, Vec<(u32, assay::Exact)>)> {
    let max_dim = u32::try_from(universe.ops.len()).unwrap_or(u32::MAX);
    for dim in 1..=max_dim {
        let Ok(witness) = universe.solve_forward(dim, target) else {
            continue;
        };
        if universe.closes_to(dim, &witness, target, fuel).is_ok() {
            return Some((dim, witness));
        }
    }
    None
}

fn read_record(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    layout: &Layout,
) -> Result<Option<(Tag, Vec<u8>)>, KernelRefused> {
    let mut chunk = [0u8; 4096];
    loop {
        match session::step(layout, buffer, RECORD_BOUND) {
            Step::Take(whole) => {
                let frame: Vec<u8> = buffer.drain(..whole).collect();
                let tag = layout.take_tag(&frame).ok_or(KernelRefused::Malformed)?;
                return Ok(Some((tag, frame)));
            }
            Step::Refuse(_) => return Err(KernelRefused::Malformed),
            Step::Wait => {
                let got = stream.read(&mut chunk)?;
                if got == 0 {
                    return if buffer.is_empty() {
                        Ok(None)
                    } else {
                        Err(KernelRefused::Malformed)
                    };
                }
                buffer.extend_from_slice(chunk.get(..got).unwrap_or(&[]));
            }
        }
    }
}

fn send_frame(
    stream: &mut TcpStream,
    layout: &Layout,
    tag: Tag,
    value: &[u8],
) -> Result<(), KernelRefused> {
    let mut wire = Vec::new();
    isthmus::frame::put_frame(layout, tag, value, &mut wire).map_err(|_| KernelRefused::Malformed)?;
    stream.write_all(&wire)?;
    Ok(())
}
