//! X3 + X5 — the x402 gateway: the court's HTTP face.
//!
//! ```text
//! agent ── GET /query ──────────► 402 + challenge JSON (X5: the
//!                                 guarantee DECLARED, never implied)
//! agent ── POST /answer ────────► settle_answer → signed receipt JSON
//! facilitator ── receipt ───────► verifies offline (V20/V21 recipe),
//!                                 executes EIP-3009 on Base
//! ```
//!
//! The division of labor holds by construction: this module lives in
//! the court's crate (the gateway is the court operator's face, like
//! `plumbd`), the substrate never learns HTTP, and **custody never
//! enters** — the gateway ASSEMBLES the `transferWithAuthorization`
//! calldata for the facilitator and executes nothing. The receipt
//! makes facilitator misbehavior provable, not impossible; that
//! boundary is the ruling, restated in code.
//!
//! No HTTP or JSON dependency is spent: the requests this speaks are
//! two fixed shapes, and a parser that cannot express anything else
//! is the smaller attack surface.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use isthmus::deed::Ledger;

use crate::bounty::{settle_answer, AnswerRefused, Bounty};
use crate::query::{Guarantee, Query};
use crate::receipt;
use crate::reward::RewardBook;

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
        _ => (404, "{\"refused\":\"no such resource\"}".into()),
    }
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
