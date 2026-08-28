//! `gateway` — the court's x402 HTTP face (X3).
//!
//! ```text
//! gateway <config>
//!   listen      = 127.0.0.1:9801
//!   chain       = path/to/founding.tlv   # receipts verify against this
//!   court       = court-a                # the issuing court's name
//!   seed_file   = ident/court-a.seed     # from `plumbd keygen` — never inline
//!   seed        = <64 hex>               # fixtures/tests only
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
// The x402 machinery lives HERE, in the edge binary — the
// quarantine: no library a node links contains one byte of HTTP.
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
    let mut seed_file = None;
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
            "seed_file" => seed_file = Some(value),
            "facilitator" => facilitator = value,
            other => eprintln!("gateway: unknown config key ignored: {other}"),
        }
    }
    let seed = if let Some(path) = &seed_file {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("gateway: seed_file {path} unreadable: {e}");
            std::process::exit(2);
        });
        seed_from_hex(text.trim()).unwrap_or_else(|| {
            eprintln!("gateway: seed_file {path} does not hold 64 hex chars");
            std::process::exit(2);
        })
    } else {
        let Some(seed) = seed_hex.as_deref().and_then(seed_from_hex) else {
            eprintln!("gateway: needs `seed_file =` or `seed =` — receipts are signed");
            std::process::exit(2);
        };
        seed
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
    // P5 — a real theorem, not a fixture: `bab = aa` in the dihedral
    // group of order 6, a genuine instance of the defining relation
    // `bab^-1 = a^-1`. See datum::corpus for the citation.
    let (_, conjecture) = datum::corpus::dihedral_conjecture().unwrap_or_else(|e| {
        eprintln!("gateway: dihedral corpus failed to compile: {e:?}");
        std::process::exit(1);
    });
    let query = Query {
        poser: court.clone(),
        shape: vec![2, 3],
        domain_tag: 82,
        guarantee: Guarantee::Rederivation,
        statement: conjecture.encode(),
    };
    let bounty = Bounty {
        query_id: query.query_id(),
        // The dihedral corpus's own derivation spends ~464 fuel and
        // ~8.7KB — measured, not guessed; these budgets give real
        // headroom without inflating the yield rebate the way
        // assay::complex::DEFAULT_FUEL's 1,000,000 ceiling would.
        max_fuel: 2_000,
        max_bytes: 10_000,
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
    let err = serve(&listener, &gateway);
    eprintln!("gateway: listener failed: {err}");
    std::process::exit(1);
}

// ═══ the quarantined machinery (formerly datum::x402) ═══


use std::io::{Read, Write};
use std::net::TcpStream;

use datum::bounty::settle_answer;
use datum::bounty::AnswerRefused;
use datum::receipt;

/// One posted question, served over HTTP.
pub struct Gateway {
    /// The demand-posed problem (X1).
    pub query: Query,
    /// Its priced budget and rates (O1).
    pub bounty: Bounty,
    /// The court's book, shared with whatever else settles.
    pub book: Arc<Mutex<RewardBook>>,
    /// The chain receipts verify against.
    pub chain: Ledger,
    /// The issuing court's name on that chain.
    pub court: String,
    /// The court's signing key.
    pub key: sig::Keypair,
    /// The escrow facilitator the challenge names (OQ3: Base). A
    /// counterparty address, not a trust decision — the receipt is
    /// what narrows the trust.
    pub facilitator: String,
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The 402 challenge body (X5): everything the price buys, declared.
#[must_use]
pub fn challenge_json(gateway: &Gateway) -> String {
    let guarantee = match gateway.query.guarantee {
        Guarantee::Rederivation => "rederivation",
        Guarantee::Convergence => "convergence",
    };
    format!(
        concat!(
            "{{\"price\":\"{}\",\"token\":\"USDC\",\"chain\":\"base\",",
            "\"escrow_facilitator\":\"{}\",\"query_id\":\"{}\",",
            "\"guarantee\":\"{}\",",
            "\"settlement_condition\":\"plumb receipt for this query_id, ",
            "verifiable per conformance/MANIFEST.md (V20/V21)\"}}"
        ),
        gateway.bounty.escrow_bound(),
        gateway.facilitator,
        hex(&gateway.query.query_id()),
        guarantee,
    )
}

/// The payer's EIP-3009 authorization — everything their wallet
/// signed. Nine fields because the standard has nine; grouping them
/// is the honest shape (clippy agreed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferAuthorization {
    /// The payer.
    pub from: [u8; 20],
    /// The payee.
    pub to: [u8; 20],
    /// USDC atomic units.
    pub value: u128,
    /// Not valid before this unix time.
    pub valid_after: u64,
    /// Not valid at or after this unix time.
    pub valid_before: u64,
    /// The authorization's one-time nonce.
    pub nonce: [u8; 32],
    /// Signature recovery byte — the PAYER's, never ours.
    pub v: u8,
    /// Signature r.
    pub r: [u8; 32],
    /// Signature s.
    pub s: [u8; 32],
}

/// EIP-3009 `transferWithAuthorization` calldata, ASSEMBLED for the
/// facilitator — the gateway never signs and never executes.
///
/// The selector `0xe3ee160e` is the standard's, hard-coded because
/// this workspace carries BLAKE3 and not keccak — and importing an
/// EVM stack to recompute a well-known constant would be spending a
/// dependency to re-derive a number the standard already states.
#[must_use]
pub fn eip3009_calldata(auth: &TransferAuthorization) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 9);
    out.extend_from_slice(&[0xe3, 0xee, 0x16, 0x0e]);
    let mut word = |fill: &dyn Fn(&mut [u8; 32])| {
        let mut w = [0u8; 32];
        fill(&mut w);
        out.extend_from_slice(&w);
    };
    word(&|w| w[12..].copy_from_slice(&auth.from));
    word(&|w| w[12..].copy_from_slice(&auth.to));
    word(&|w| w[16..].copy_from_slice(&auth.value.to_be_bytes()));
    word(&|w| w[24..].copy_from_slice(&auth.valid_after.to_be_bytes()));
    word(&|w| w[24..].copy_from_slice(&auth.valid_before.to_be_bytes()));
    word(&|w| w.copy_from_slice(&auth.nonce));
    word(&|w| w[31] = auth.v);
    word(&|w| w.copy_from_slice(&auth.r));
    word(&|w| w.copy_from_slice(&auth.s));
    out
}

/// Answer one HTTP request. Pure — the server loop and the tests
/// drive the same function.
pub fn handle(method: &str, path: &str, body: &[u8], gateway: &Gateway) -> (u16, String) {
    match (method, path) {
        ("GET", "/query") => (402, challenge_json(gateway)),
        ("POST", "/answer") => {
            let mut book = match gateway.book.lock() {
                Ok(guard) => guard,
                Err(_) => return (500, "{\"refused\":\"court unreachable\"}".into()),
            };
            match settle_answer(&gateway.bounty, &gateway.query, body, &mut book) {
                Ok(answer) => {
                    let epoch = book.open_epoch().unwrap_or(0);
                    let signed = receipt::issue(
                        &gateway.court,
                        epoch,
                        gateway.query.query_id(),
                        &answer.credit,
                        &gateway.key,
                    );
                    (
                        200,
                        format!(
                            concat!(
                                "{{\"payout\":\"{}\",\"spent_fuel\":{},",
                                "\"spent_bytes\":{},\"receipt\":\"{}\",",
                                "\"attestation\":\"{}\"}}"
                            ),
                            answer.payout,
                            answer.spent_fuel,
                            answer.spent_bytes,
                            hex(&signed.receipt.encode()),
                            hex(&signed.attestation.encode()),
                        ),
                    )
                }
                Err(refused) => (422, refusal_json(&refused)),
            }
        }
        ("POST", "/authorize") => {
            // R5: the calldata assembler, SERVED. The payer's wallet
            // signed the authorization; the facilitator needs the
            // standard's calldata. Fixed-width raw body — 169 bytes,
            // one shape, no parser surface:
            // from(20) ‖ to(20) ‖ value(16 BE) ‖ after(8 BE) ‖
            // before(8 BE) ‖ nonce(32) ‖ v(1) ‖ r(32) ‖ s(32).
            match parse_authorization(body) {
                Some(auth) => (
                    200,
                    format!("{{\"calldata\":\"{}\"}}", hex(&eip3009_calldata(&auth))),
                ),
                None => (
                    422,
                    "{\"refused\":\"authorization is 169 fixed bytes\"}".into(),
                ),
            }
        }
        _ => (404, "{\"refused\":\"no such resource\"}".into()),
    }
}

/// The one fixed shape `/authorize` accepts.
fn parse_authorization(body: &[u8]) -> Option<TransferAuthorization> {
    if body.len() != 169 {
        return None;
    }
    let grab = |from: usize, len: usize| body.get(from..from.saturating_add(len));
    let mut from = [0u8; 20];
    from.copy_from_slice(grab(0, 20)?);
    let mut to = [0u8; 20];
    to.copy_from_slice(grab(20, 20)?);
    let mut value = [0u8; 16];
    value.copy_from_slice(grab(40, 16)?);
    let mut after = [0u8; 8];
    after.copy_from_slice(grab(56, 8)?);
    let mut before = [0u8; 8];
    before.copy_from_slice(grab(64, 8)?);
    let mut nonce = [0u8; 32];
    nonce.copy_from_slice(grab(72, 32)?);
    let v = *grab(104, 1)?.first()?;
    let mut r = [0u8; 32];
    r.copy_from_slice(grab(105, 32)?);
    let mut s = [0u8; 32];
    s.copy_from_slice(grab(137, 32)?);
    Some(TransferAuthorization {
        from,
        to,
        value: u128::from_be_bytes(value),
        valid_after: u64::from_be_bytes(after),
        valid_before: u64::from_be_bytes(before),
        nonce,
        v,
        r,
        s,
    })
}

fn refusal_json(refused: &AnswerRefused) -> String {
    // The refusal NAMES itself — an HTTP peer gets the same honesty a
    // wire peer does.
    format!("{{\"refused\":\"{refused:?}\"}}").replace('\n', " ")
}

/// Serve until the listener fails. One request per connection — the
/// two shapes this speaks don't need keep-alive.
pub fn serve(listener: &TcpListener, gateway: &Gateway) -> std::io::Error {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(e) = respond(stream, gateway) {
                    println!("gateway: request failed: {e}");
                }
            }
            Err(e) => return e,
        }
    }
}

fn respond(mut stream: TcpStream, gateway: &Gateway) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    // Read headers, then exactly Content-Length body bytes.
    let (head_end, mut have) = loop {
        let got = stream.read(&mut chunk)?;
        if got == 0 {
            return Ok(()); // peer left mid-request; nothing to answer
        }
        buffer.extend_from_slice(chunk.get(..got).unwrap_or(&[]));
        if let Some(at) = find_head_end(&buffer) {
            break (at, buffer.len().saturating_sub(at));
        }
        if buffer.len() > 1 << 16 {
            return Ok(()); // a header that never ends is not a request
        }
    };
    let head = String::from_utf8_lossy(buffer.get(..head_end).unwrap_or(&[])).into_owned();
    let mut lines = head.split("\r\n");
    let request = lines.next().unwrap_or("");
    let mut parts = request.split(' ');
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts.next().unwrap_or("").to_owned();
    let length: usize = lines
        .filter_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(str::trim).map(str::to_owned))
        .find_map(|v| v.parse().ok())
        .unwrap_or(0);
    let length = length.min(1 << 20);
    while have < length {
        let got = stream.read(&mut chunk)?;
        if got == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(chunk.get(..got).unwrap_or(&[]));
        have = have.saturating_add(got);
    }
    let body = buffer
        .get(head_end..head_end.saturating_add(length))
        .unwrap_or(&[]);
    let (status, response_body) = handle(&method, &path, body, gateway);
    let reason = match status {
        200 => "OK",
        402 => "Payment Required",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        _ => "Internal Server Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
        response_body.len(),
    );
    stream.write_all(response.as_bytes())
}

fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|at| at.saturating_add(4))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
fn test_edge_with(holder: &str) -> isthmus::deed::Ledger {
    let mut ledger = isthmus::deed::Ledger::new(Layout::founding());
    ledger.encumber(1, 31, "ancestral", "founding registries");
    ledger.issue(holder, 16).expect("room");
    ledger
}

#[cfg(test)]
#[allow(clippy::expect_used)]
fn test_bind(ledger: &mut isthmus::deed::Ledger, holder: &str, key: &sig::Keypair, from: u64, until: u64) {
    ledger.record(isthmus::deed::Act::Bind {
        holder: holder.into(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public().to_vec(),
        from_epoch: from,
        until_epoch: until,
    });
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, clippy::indexing_slicing)]
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    use datum::bounty::Bounty;
    use datum::query::{Guarantee, Query};
    use datum::reward::RewardBook;
    use super::{self as x402, Gateway};

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
            .collect()
    }

    fn gateway() -> Gateway {
        let key = sig::Keypair::from_seed([4u8; 32]);
        let mut chain = super::test_edge_with("court-a");
        super::test_bind(&mut chain, "court-a", &key, 0, 100);
        let query = Query {
            poser: "agent-7".into(),
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
        Gateway {
            query,
            bounty,
            book: Arc::new(Mutex::new(RewardBook::new())),
            chain,
            court: "court-a".into(),
            key,
            facilitator: "0xFaci1itat0rPlaceholder".into(),
        }
    }

    fn lean_answer() -> Vec<u8> {
        assay::complex::DeclaredClaim {
            transport: 1,
            complex: datum::domains::demo_theta_universe(),
            dim: 1,
            witness: vec![(0, assay::whole(1)), (1, assay::whole(-1))],
        }
        .encode()
    }

    fn field(json: &str, name: &str) -> String {
        let key = format!("\"{name}\":\"");
        let start = json.find(&key).map(|at| at + key.len()).unwrap_or(0);
        json.get(start..)
            .and_then(|rest| rest.find('\"').map(|end| rest.get(..end).unwrap_or("")))
            .unwrap_or("")
            .to_owned()
    }

    #[test]
    fn the_challenge_declares_everything_the_price_buys() {
        let gw = gateway();
        let (status, body) = x402::handle("GET", "/query", &[], &gw);
        assert_eq!(status, 402, "payment required IS the challenge");
        assert_eq!(field(&body, "guarantee"), "rederivation", "X5: declared, never implied");
        assert_eq!(field(&body, "chain"), "base");
        assert_eq!(
            field(&body, "price"),
            gw.bounty.escrow_bound().to_string(),
            "the price is the escrow bound — underwritable by construction"
        );
        assert_eq!(field(&body, "query_id").len(), 64);
    }

    #[test]
    fn the_whole_loop_over_a_real_socket_ends_in_an_offline_verification() {
        let gw = gateway();
        let chain = gw.chain.clone();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let _ = x402::serve(&listener, &gw);
        });

        // The agent: GET the challenge.
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .write_all(b"GET /query HTTP/1.1\r\nHost: x\r\n\r\n")
            .expect("writes");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("reads");
        assert!(response.starts_with("HTTP/1.1 402"), "{response}");

        // The solver: POST the lean answer.
        let answer = lean_answer();
        let mut stream = TcpStream::connect(addr).expect("connect");
        let request = format!(
            "POST /answer HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
            answer.len()
        );
        stream.write_all(request.as_bytes()).expect("writes");
        stream.write_all(&answer).expect("writes");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("reads");
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("");

        // The facilitator: verify the receipt OFFLINE, against the
        // chain alone — the V20/V21 recipe, over HTTP-carried bytes.
        let receipt_bytes = hex_to_bytes(&field(body, "receipt"));
        let attestation_bytes = hex_to_bytes(&field(body, "attestation"));
        let signed = datum::receipt::SignedReceipt {
            receipt: datum::receipt::Receipt::decode(&receipt_bytes).expect("decodes"),
            attestation: sig::Attestation::decode(&attestation_bytes).expect("decodes"),
        };
        datum::receipt::verify(&signed, &chain).expect("the facilitator's whole check");
        assert_eq!(signed.receipt.axes, vec![2, 3]);

        // The copy refuses over the same wire: the market's laws
        // reach HTTP untranslated.
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.write_all(request.as_bytes()).expect("writes");
        stream.write_all(&answer).expect("writes");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("reads");
        assert!(response.starts_with("HTTP/1.1 422"), "{response}");
        assert!(response.contains("Replay"), "the refusal names itself over HTTP too");
    }

    #[test]
    fn the_authorize_endpoint_serves_the_assembler() {
        let gw = gateway();
        let mut body = Vec::new();
        body.extend_from_slice(&[0x11; 20]);
        body.extend_from_slice(&[0x22; 20]);
        body.extend_from_slice(&5_000_000u128.to_be_bytes());
        body.extend_from_slice(&0u64.to_be_bytes());
        body.extend_from_slice(&u64::MAX.to_be_bytes());
        body.extend_from_slice(&[0x33; 32]);
        body.push(27);
        body.extend_from_slice(&[0x44; 32]);
        body.extend_from_slice(&[0x55; 32]);
        let (status, response) = x402::handle("POST", "/authorize", &body, &gw);
        assert_eq!(status, 200);
        assert!(response.contains("e3ee160e"), "the standard's selector, served");

        let (status, _) = x402::handle("POST", "/authorize", &body[..100], &gw);
        assert_eq!(status, 422, "one shape; anything else refuses");
    }

    #[test]
    fn the_calldata_is_the_standards_shape_and_nothing_signs_here() {
        let calldata = x402::eip3009_calldata(&x402::TransferAuthorization {
            from: [0x11; 20],
            to: [0x22; 20],
            value: 5_000_000,
            valid_after: 0,
            valid_before: u64::MAX,
            nonce: [0x33; 32],
            v: 27,
            r: [0x44; 32],
            s: [0x55; 32],
        });
        assert_eq!(calldata.len(), 4 + 32 * 9, "selector + nine words");
        assert_eq!(calldata.get(..4), Some(&[0xe3, 0xee, 0x16, 0x0e][..]));
        // Address left-padding: the first word's 12 zero bytes.
        assert_eq!(calldata.get(4..16), Some(&[0u8; 12][..]));
        assert_eq!(calldata.get(16..36).map(|w| w[0]), Some(0x11));
    }
}
