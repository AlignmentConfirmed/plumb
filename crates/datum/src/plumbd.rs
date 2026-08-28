//! The node: isthmus sessions over TCP.
//!
//! ```text
//! producer ──TCP──► court
//!   1. declaration        the FIRST record on the edge, both ways (IS-5)
//!   2. claim envelopes    tags 80/82, opaque in transit
//!   3. credit             the court's book, work_id-primary
//! ```
//!
//! This module is the daemon's engine; `bin/plumbd.rs` is the thin
//! command over it. The seam between the two halves of a session here
//! is the seam the wider network runs on: nothing in a session names a
//! kernel type, and the court decodes only what it owns.
//!
//! Sessions are signed (S4: attestations beside envelopes), **fresh**
//! (IS-2/2: an entropy challenge after the declaration, answered by a
//! signature over its exact frame bytes — a replayed session's answer
//! covers a token this court never issued again), and federated
//! (`court_service` wires `court_live` peering around the serve loop).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};

use isthmus::deed::Ledger;
use isthmus::hello::Hello;
use isthmus::layout::{Layout, Tag};
use isthmus::session::{self, Step};

use crate::admission;
use crate::bounty::{settle_answer, Bounty};
use crate::query::Query;
use crate::receipt;
use crate::reward::RewardBook;
use crate::witnessing;

/// A session's wire, plaintext or TLS — chosen once, at connect or
/// accept time, never after. Everything downstream (`read_record`,
/// `send_hello`, the claim loop) reads and writes through this alone,
/// which is what makes P4 a substitution at the edges rather than a
/// second copy of the session logic.
pub trait ReadWrite: Read + Write + Send {
    /// Signal "no more writes coming" WITHOUT fully closing — so a
    /// peer still mid-read-loop gets a clean end to its input rather
    /// than a drop. Needed by a relay (the carrier) that must not
    /// close its upstream leg while records it forwarded might still
    /// be unread by the peer: closing a socket with unread data of
    /// its OWN still queued sends RST instead of FIN, and RST at a
    /// record boundary elsewhere reads as a graceful departure — a
    /// carrier that closes early can make a court silently drop
    /// records that genuinely arrived.
    fn shutdown_write(&mut self) -> std::io::Result<()>;
}

impl ReadWrite for TcpStream {
    fn shutdown_write(&mut self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Write)
    }
}

impl ReadWrite for rustls::StreamOwned<rustls::ClientConnection, TcpStream> {
    fn shutdown_write(&mut self) -> std::io::Result<()> {
        self.conn.send_close_notify();
        Write::flush(self)
    }
}

impl ReadWrite for rustls::StreamOwned<rustls::ServerConnection, TcpStream> {
    fn shutdown_write(&mut self) -> std::io::Result<()> {
        self.conn.send_close_notify();
        Write::flush(self)
    }
}

impl<T: ReadWrite + ?Sized> ReadWrite for Box<T> {
    fn shutdown_write(&mut self) -> std::io::Result<()> {
        (**self).shutdown_write()
    }
}

// K1: canonical home is `sdk::submit` now — the court announces its
// question right after the session challenge, natively, no HTTP; a
// kernel that never links datum must agree on this byte too.
// Re-exported so every existing `QUERY_TAG` reference is unchanged.
pub use sdk::submit::QUERY_TAG;

/// A demand-posed market a court serves natively: the question, its
/// bounty, and the receipt-signing identity.
pub struct MarketPost {
    /// The question (X1).
    pub query: Query,
    /// Its priced budget and rates (O1).
    pub bounty: Bounty,
    /// The issuing court's name on the chain.
    pub court: String,
    /// The court's receipt-signing key.
    pub key: sig::Keypair,
}

/// Where a court keeps what witnesses put on the record.
pub type WitnessLog = Arc<Mutex<Vec<isthmus::witness::Witness>>>;

/// Why a node session ended or refused.
#[derive(Debug)]
pub enum NodeBroken {
    /// Transport IO failed or closed mid-record.
    Io(std::io::Error),
    /// The head record can never complete; the peer is dropped.
    Unsatisfiable,
    /// The first record on the edge did not decode as a declaration.
    NoDeclaration,
    /// The peer and this node share no revision string. Neither is
    /// wrong; they cannot talk.
    NoSharedRevision,
    /// The record could not be framed for the wire.
    CannotFrame,
    /// The court's book is unreachable (a poisoned lock is a crashed
    /// sibling thread, answered rather than unwrapped).
    CourtUnreachable,
}

impl From<std::io::Error> for NodeBroken {
    fn from(e: std::io::Error) -> Self {
        NodeBroken::Io(e)
    }
}

/// One whole record off the stream: its tag and its full frame bytes.
///
/// `Ok(None)` is a clean close between records — the peer left, and
/// that is not an error. A close *inside* a record is [`NodeBroken::Io`]:
/// bytes were promised and never arrived.
pub fn read_record(
    stream: &mut dyn ReadWrite,
    buffer: &mut Vec<u8>,
    layout: &Layout,
    bound: usize,
) -> Result<Option<(Tag, Vec<u8>)>, NodeBroken> {
    let mut chunk = [0u8; 4096];
    loop {
        match session::step(layout, buffer, bound) {
            Step::Take(whole) => {
                let frame: Vec<u8> = buffer.drain(..whole).collect();
                let tag = match layout.take_tag(&frame) {
                    Some(t) => t,
                    None => return Err(NodeBroken::Unsatisfiable),
                };
                return Ok(Some((tag, frame)));
            }
            Step::Refuse(_) => return Err(NodeBroken::Unsatisfiable),
            Step::Wait => {
                let got = match stream.read(&mut chunk) {
                    Ok(got) => got,
                    // A reset at a record boundary is a peer that left
                    // with unread data in ITS buffer (e.g. a fixture
                    // producer that never reads the market
                    // announcement) — TCP sends RST instead of FIN.
                    // P4's TLS wrap surfaces the SAME shape of
                    // departure as `UnexpectedEof` instead: a peer
                    // that drops the connection without a TLS
                    // close_notify alert (rustls's own docs call this
                    // expected when the other side just closes rather
                    // than shutting down cleanly). Between records
                    // either is a departure, not a failure; inside one
                    // either is still an error.
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::UnexpectedEof
                        ) && buffer.is_empty() =>
                    {
                        return Ok(None)
                    }
                    Err(e) => return Err(NodeBroken::Io(e)),
                };
                if got == 0 {
                    if buffer.is_empty() {
                        return Ok(None);
                    }
                    return Err(NodeBroken::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "closed inside a record",
                    )));
                }
                buffer.extend_from_slice(chunk.get(..got).unwrap_or(&[]));
            }
        }
    }
}

/// The tag a declaration travels under: the first live deed's low tag
/// for this holder, or the substrate's granted floor when the holder
/// holds nothing yet. Position makes it the declaration; the tag just
/// has to be writable in the layout.
#[must_use]
pub fn hello_tag(ledger: &Ledger, holder: &str) -> Tag {
    ledger
        .deeds()
        .into_iter()
        .find(|d| d.live && d.holder == holder)
        .map(|d| d.low())
        .unwrap_or(64)
}

/// Frame and send this node's declaration.
pub fn send_hello(
    stream: &mut dyn ReadWrite,
    layout: &Layout,
    tag: Tag,
    hello: &Hello,
) -> Result<(), NodeBroken> {
    let mut wire = Vec::new();
    isthmus::frame::put_frame(layout, tag, &hello.encode(), &mut wire)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&wire)?;
    Ok(())
}

/// Read the peer's declaration — the first record on the edge — and
/// hold the session to the revisions both sides share.
pub fn read_hello(
    stream: &mut dyn ReadWrite,
    buffer: &mut Vec<u8>,
    layout: &Layout,
    ours: &Hello,
    bound: usize,
) -> Result<Hello, NodeBroken> {
    let (_, frame) = read_record(stream, buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;
    let value = frame
        .get(layout.header()..)
        .ok_or(NodeBroken::NoDeclaration)?;
    let theirs = Hello::decode(value).map_err(|_| NodeBroken::NoDeclaration)?;
    if ours.shared_revisions(&theirs).is_empty() {
        return Err(NodeBroken::NoSharedRevision);
    }
    Ok(theirs)
}

/// How a court runs its sessions: one struct, so the daemon, the
/// tests, and future roles pass the same rules the same way.
#[derive(Clone)]
pub struct SessionRules {
    /// What the chain calls this court.
    pub holder: String,
    /// Largest record value this deployment accepts — measured.
    pub bound: usize,
    /// S4: hold every work envelope for its attestation and refuse
    /// forged / stale / unbound / orphaned presentations.
    pub enforce: bool,
    /// A posted market, served natively over the wire (tag 85 out,
    /// receipts back on tag 81). `None` is a court with no question.
    pub market: Option<Arc<MarketPost>>,
    /// P2: accept live registration from an unbound key that proves
    /// possession over this session's own challenge. `false` is a
    /// court that only ever admits the parties genesis already knew.
    pub register: bool,
    /// Where to flush the ledger's act log after a live registration
    /// lands, atomically — so a restart replays it. `None` means a
    /// successful registration would not survive a restart; courts
    /// that set `register = true` should set this too.
    pub chain_path: Option<std::path::PathBuf>,
    /// P3 — the admission wall: no more than this many sessions held
    /// at once, total. `0` is unwalled (today's behaviour) — every
    /// connection gets a thread, unconditionally.
    pub max_total_connections: usize,
    /// P3 — no more than this many sessions held at once FROM ONE IP.
    /// `0` is unwalled.
    pub max_connections_per_ip: usize,
    /// P3 — a connection that has not sent its declaration within
    /// this long is dropped before it ever reaches the work loop.
    /// `None` is unwalled: `read_hello` blocks forever, as it always
    /// has. A bare TCP accept spawns a thread before ANY check runs —
    /// this wall bounds what an unauthenticated connection can hold.
    pub handshake_deadline: Option<std::time::Duration>,
    /// The live count behind the wall — shared across every session
    /// this court is holding. Cloning `SessionRules` clones the `Arc`,
    /// not the count: every session thread sees the same wall.
    pub connections: Arc<Mutex<ConnectionCounts>>,
    /// P4 — this court's own TLS identity. `None` is plaintext, the
    /// wire exactly as every prior revision left it. `Some` wraps
    /// every accepted connection in TLS before `court_session` ever
    /// sees it — every inbound tag (claims, registration, market
    /// answers, witness records) rides the same encrypted channel,
    /// since they all cross the SAME accept loop.
    pub tls: Option<Arc<rustls::ServerConfig>>,
}

/// How many sessions a court is holding right now: in total, and per
/// peer IP. Checked and updated ONLY at accept time and at a
/// session's end — never inside a session, which has no reason to
/// know the wall exists.
#[derive(Debug, Default)]
pub struct ConnectionCounts {
    total: usize,
    per_ip: std::collections::HashMap<std::net::IpAddr, usize>,
}

impl ConnectionCounts {
    /// How many sessions are held right now, across every peer.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// How many sessions are held right now from one peer IP.
    #[must_use]
    pub fn for_ip(&self, ip: std::net::IpAddr) -> usize {
        self.per_ip.get(&ip).copied().unwrap_or(0)
    }
}

impl std::fmt::Debug for SessionRules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRules")
            .field("holder", &self.holder)
            .field("bound", &self.bound)
            .field("enforce", &self.enforce)
            .field("market", &self.market.is_some())
            .field("register", &self.register)
            .field("chain_path", &self.chain_path)
            .field("max_total_connections", &self.max_total_connections)
            .field("max_connections_per_ip", &self.max_connections_per_ip)
            .field("handshake_deadline", &self.handshake_deadline)
            .field("tls", &self.tls.is_some())
            .finish()
    }
}

/// Keep the last few crossed envelopes for the session's watcher —
/// bounded, so a long session cannot hoard memory.
fn remember(seen: &mut Vec<Vec<u8>>, envelope: Vec<u8>) {
    if seen.len() >= 8 {
        seen.remove(0);
    }
    seen.push(envelope);
}

/// Credit a work value — through the posted market when the claim
/// inhabits the poser's universe (settling the bounty and returning
/// the signed receipt on the wire), through the plain book otherwise.
fn credit_value(
    stream: &mut dyn ReadWrite,
    layout: &Layout,
    rules: &SessionRules,
    book: &mut RewardBook,
    value: &[u8],
    report: &mut SessionReport,
) -> Result<(), NodeBroken> {
    if let Some(market) = &rules.market {
        match settle_answer(&market.bounty, &market.query, value, book) {
            Ok(answer) => {
                let epoch = book.open_epoch().unwrap_or(0);
                let signed = receipt::issue(
                    &market.court,
                    epoch,
                    market.query.query_id(),
                    answer.credit.work_id.as_bytes(),
                    answer.credit.axes.components(),
                    &market.key,
                );
                let mut body = signed.receipt.encode();
                body.extend_from_slice(&signed.attestation.encode());
                let mut wire = Vec::new();
                isthmus::frame::put_frame(layout, isthmus::work::RECEIPT_TAG, &body, &mut wire)
                    .map_err(|_| NodeBroken::CannotFrame)?;
                stream.write_all(&wire)?;
                report.credited += 1;
                return Ok(());
            }
            Err(crate::bounty::AnswerRefused::NotThePosersUniverse)
            | Err(crate::bounty::AnswerRefused::NotAProof)
            | Err(crate::bounty::AnswerRefused::NotDeclared(_)) => {
                // Not an answer to the question — ordinary work. This
                // has to cover BOTH market shapes: a plain-universe
                // market's "wrong universe" (NotThePosersUniverse) and
                // a conjecture market's "not even a proof"
                // (NotAProof) / "not even a declared claim"
                // (NotDeclared) all mean the same thing — the
                // submission never engaged with what was posed. A
                // court's ordinary background traffic (a producer's
                // plain claims, unrelated to whatever question is
                // posted) must credit the same way regardless of
                // which shape of question happens to be live; before
                // this, a conjecture-shaped market silently refused
                // every non-market claim it ever saw.
            }
            Err(_) => {
                report.refused += 1;
                return Ok(());
            }
        }
    }
    match book.credit_claim(value) {
        Ok(_) => report.credited += 1,
        Err(_) => report.refused += 1,
    }
    Ok(())
}

/// What one court session did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionReport {
    /// Work records whose claims credited.
    pub credited: usize,
    /// Work records refused by the book (replay, open work, forged).
    pub refused: usize,
    /// Records skipped — tags this court does not own, forwarded in a
    /// mesh and merely counted here.
    pub skipped: usize,
    /// Witness frames put on the record (IS-4).
    pub witnessed: usize,
    /// Witnesses this session's own watcher re-derived (the subject
    /// crossed the same session).
    pub watched: usize,
    /// Watched witnesses whose subjects FAILED re-derivation — a
    /// dispute on the record.
    pub disputed: usize,
    /// Live registrations that landed this session (P2) — an unbound
    /// key proved possession and was issued a deed and bound, with no
    /// restart and no re-genesis.
    pub registered: usize,
}

/// Serve one inbound session as the court.
///
/// Declaration first, both ways; then every work record's value goes
/// to the shared book. The value is decoded by the book, never here —
/// the session layer stays payload-blind about everything but the tag.
pub fn court_session(
    mut stream: Box<dyn ReadWrite>,
    handshake_socket: Option<TcpStream>,
    layout: &Layout,
    ledger: &Arc<Mutex<Ledger>>,
    rules: &SessionRules,
    book: &Arc<Mutex<RewardBook>>,
    witnesses: &WitnessLog,
) -> Result<SessionReport, NodeBroken> {
    let (holder, bound, enforce) = (rules.holder.as_str(), rules.bound, rules.enforce);
    // A snapshot for framing OUR OWN declaration and challenge tag —
    // read once, at session start, exactly as a per-thread clone did
    // before this ledger could change live. Admission checks below
    // re-lock for the CURRENT state on purpose: a registration that
    // lands mid-session must be visible to that same session's next
    // claim.
    let opening = ledger
        .lock()
        .map_err(|_| NodeBroken::CourtUnreachable)?
        .clone();
    let ours = Hello::of(&opening, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    let mut buffer = Vec::new();
    // P3's handshake deadline is set on the raw socket in `serve`,
    // before this function ever sees the stream — it has to cover a
    // TLS handshake too, when P4 wraps one, not just the declaration
    // this function reads. `handshake_socket` is a clone of that same
    // fd, held only long enough to lift the deadline once the
    // declaration arrives — a working session's later idle gaps (a
    // producer sleeping between claims) are not this wall's concern.
    let _theirs = read_hello(stream.as_mut(), &mut buffer, layout, &ours, bound)?;
    if let Some(raw) = &handshake_socket {
        let _ = raw.set_read_timeout(None);
    }
    send_hello(stream.as_mut(), layout, hello_tag(&opening, holder), &ours)?;

    // IS-2/2 — the session challenge: eight bytes of entropy, framed,
    // sent once per session right after the declaration. A replayed
    // session dies here: its recorded answer covers a token this court
    // never issued again.
    let token = sig::session_token().map_err(|_| NodeBroken::CannotFrame)?;
    let mut challenge_frame = Vec::new();
    isthmus::frame::put_frame(layout, hello_tag(&opening, holder), &token, &mut challenge_frame)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&challenge_frame)?;

    // The native market: the question is ANNOUNCED on the wire the
    // session already speaks. HTTP exists only for foreign payers, at
    // the gateway edge; a Plumbline solver never touches it.
    if let Some(market) = &rules.market {
        let mut announcement = Vec::new();
        isthmus::frame::put_frame(layout, QUERY_TAG, &market.query.encode(), &mut announcement)
            .map_err(|_| NodeBroken::CannotFrame)?;
        stream.write_all(&announcement)?;
    }

    let mut report = SessionReport::default();
    let mut session_live = !enforce; // enforcement holds the session until the challenge is answered
    // Under enforcement a work envelope is held until its attestation
    // arrives (the next record); an envelope displaced or orphaned
    // without one is refused, not credited.
    let mut pending: Option<Vec<u8>> = None;
    // What this session saw cross, most recent last — the subjects a
    // session-local watcher can be HANDED (it may not fetch, §6.1).
    let mut seen_envelopes: Vec<Vec<u8>> = Vec::new();
    // P2: a register request, held until the attestation proving
    // possession of ITS key arrives — the same displaced-envelope
    // discipline a work claim gets, applied to a bind-in-waiting.
    let mut pending_register: Option<crate::registration::RegisterRequest> = None;
    while let Some((tag, frame)) = read_record(&mut stream, &mut buffer, layout, bound)? {
        if isthmus::work::is_work_tag(tag) && tag != isthmus::work::RECEIPT_TAG {
            if !enforce {
                let value = frame.get(layout.header()..).unwrap_or(&[]).to_vec();
                remember(&mut seen_envelopes, frame.clone());
                let mut guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                credit_value(&mut stream, layout, rules, &mut guard, &value, &mut report)?;
            } else if !session_live {
                report.refused += 1; // work before the challenge was answered
            } else {
                if pending.take().is_some() {
                    report.refused += 1; // unattested envelope displaced
                }
                pending = Some(frame);
            }
        } else if rules.register && tag == crate::registration::REGISTER_TAG {
            if session_live {
                report.skipped += 1; // registration is a pre-admission act only
            } else {
                match crate::registration::RegisterRequest::decode(
                    frame.get(layout.header()..).unwrap_or(&[]),
                ) {
                    Ok(request) => {
                        if pending_register.replace(request).is_some() {
                            report.refused += 1; // a second request displaced the first
                        }
                    }
                    Err(_) => report.refused += 1,
                }
            }
        } else if enforce && tag == admission::ATTESTATION_TAG {
            if !session_live {
                let attestation_bytes = frame.get(layout.header()..).unwrap_or(&[]);
                if let Some(request) = pending_register.take() {
                    // P2: proof of possession over THIS session's own
                    // challenge, then the ledger-level rules — never
                    // the ordinary admission path, which would
                    // (correctly) refuse a key that is not bound yet.
                    let outcome = (|| {
                        crate::registration::verify_possession(
                            &request,
                            &challenge_frame,
                            attestation_bytes,
                        )
                        .map_err(|_| ())?;
                        let epoch = {
                            let guard =
                                book.lock().map_err(|_| ())?;
                            guard.open_epoch().unwrap_or(0)
                        };
                        let mut guard = ledger.lock().map_err(|_| ())?;
                        let deed = crate::registration::bind_live(&mut guard, &request, epoch)
                            .map_err(|_| ())?;
                        if let Some(path) = &rules.chain_path {
                            let _ = crate::registration::persist_chain_atomic(path, &guard);
                        }
                        Ok::<_, ()>((deed, epoch))
                    })();
                    match outcome {
                        Ok((deed, epoch)) => {
                            let ack = crate::registration::RegisterOutcome {
                                low: deed.low(),
                                high: deed.high(),
                                from_epoch: epoch,
                                until_epoch: u64::MAX,
                            };
                            let mut wire = Vec::new();
                            isthmus::frame::put_frame(
                                layout,
                                crate::registration::REGISTER_TAG,
                                &ack.encode(),
                                &mut wire,
                            )
                            .map_err(|_| NodeBroken::CannotFrame)?;
                            stream.write_all(&wire)?;
                            session_live = true;
                            report.registered += 1;
                        }
                        // A refused registration closes here, same as
                        // a court that refuses a market answer: the
                        // client waiting on a response cannot tell
                        // refusal from slowness any other way, and
                        // this session's challenge is already spent —
                        // looping back to await a NEW attestation
                        // would just deadlock a client that is, in
                        // turn, waiting on this ack.
                        Err(()) => {
                            report.refused += 1;
                            return Ok(report);
                        }
                    }
                    continue;
                }
                // The first attestation must answer THIS session's
                // challenge. A stale answer — a replayed session —
                // refuses, and the session never goes live.
                let epoch = {
                    let guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                    guard.open_epoch().unwrap_or(0)
                };
                let admitted = {
                    let guard = ledger.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                    admission::admit(&guard, epoch, &challenge_frame, attestation_bytes)
                };
                match admitted {
                    Ok(_holder) => session_live = true,
                    Err(_) => report.refused += 1,
                }
                continue;
            }
            let Some(envelope) = pending.take() else {
                report.skipped += 1; // attestation with nothing to attest
                continue;
            };
            let attestation = frame.get(layout.header()..).unwrap_or(&[]);
            let guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
            let epoch = guard.open_epoch().unwrap_or(0);
            let admitted = {
                let ledger_guard = ledger.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                admission::admit(&ledger_guard, epoch, &envelope, attestation)
            };
            match admitted {
                Ok(_holder) => {
                    let value = envelope.get(layout.header()..).unwrap_or(&[]).to_vec();
                    remember(&mut seen_envelopes, envelope.clone());
                    drop(guard);
                    let mut relocked =
                        book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                    credit_value(&mut stream, layout, rules, &mut relocked, &value, &mut report)?;
                }
                Err(_) => report.refused += 1,
            }
        } else if ledger
            .lock()
            .map_err(|_| NodeBroken::CourtUnreachable)?
            .declaration_of(tag)
            .is_some()
        {
            // UC4, LIVE: a tag with a registered definition on this
            // court's chain is judged against that definition — the
            // discipline the chain taught, applied on the wire. Under
            // enforcement the attestation rules still apply upstream;
            // here the claim must inhabit the registered universe and
            // close in it.
            let value = frame.get(layout.header()..).unwrap_or(&[]);
            let verified = {
                let guard = ledger.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                crate::domains::verify_registered(&guard, tag, value, assay::complex::DEFAULT_FUEL)
            };
            match verified {
                Ok(_spent) => {
                    let mut guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                    match guard.credit_claim(value) {
                        Ok(_) => report.credited += 1,
                        Err(_) => report.refused += 1,
                    }
                }
                Err(_) => report.refused += 1,
            }
        } else if tag == witnessing::WITNESS_TAG {
            // IS-4: a witness put something on the record. The court
            // KEEPS it — decoded (refuse-not-repair), never judged
            // here; judging is a watcher's act, and a watcher is
            // handed its subject elsewhere.
            let value = frame.get(layout.header()..).unwrap_or(&[]);
            match isthmus::witness::Witness::decode(value) {
                Ok(witness) => {
                    // The watcher, live (IS-4 §6): if the witnessed
                    // subject crossed THIS session, the court was
                    // handed it — so it watches, and a failed
                    // re-derivation is a dispute on the record.
                    if let Some(subject) = seen_envelopes
                        .iter()
                        .find(|e| witnessing::subject_of(e) == witness.subject)
                    {
                        if let Ok(verdict) = witnessing::watch(&witness, subject) {
                            report.watched += 1;
                            if !verdict.verified {
                                report.disputed += 1;
                            }
                        }
                    }
                    let mut log = witnesses
                        .lock()
                        .map_err(|_| NodeBroken::CourtUnreachable)?;
                    log.push(witness);
                    report.witnessed += 1;
                }
                Err(_) => report.refused += 1,
            }
        } else {
            report.skipped += 1;
        }
    }
    if pending.is_some() {
        report.refused += 1; // session closed on an unattested envelope
    }
    Ok(report)
}

/// Accept sessions forever, one thread per peer, one shared book.
///
/// Returns only on listener failure. `on_session` sees each session's
/// report — the binary logs it; a test asserts on the book instead.
pub fn serve(
    listener: &TcpListener,
    layout: &Layout,
    ledger: &Arc<Mutex<Ledger>>,
    rules: &SessionRules,
    book: &Arc<Mutex<RewardBook>>,
    witnesses: &WitnessLog,
    on_session: impl Fn(&SessionReport) + Send + Sync + 'static,
) -> std::io::Error {
    let on_session = Arc::new(on_session);
    loop {
        match listener.accept() {
            Ok((stream, peer)) => {
                // P3 — the wall: checked and CHARGED before a thread
                // ever gets spawned. An over-quota connection is
                // dropped here, at zero cost past the accept itself —
                // today's behaviour (every accept gets a thread,
                // unconditionally) is what `max_total_connections = 0`
                // and `max_connections_per_ip = 0` still mean.
                let ip = peer.ip();
                let admitted = {
                    let Ok(mut counts) = rules.connections.lock() else {
                        continue; // a poisoned wall refuses rather than guesses
                    };
                    let total_room =
                        rules.max_total_connections == 0 || counts.total < rules.max_total_connections;
                    let ip_count = counts.per_ip.get(&ip).copied().unwrap_or(0);
                    let ip_room =
                        rules.max_connections_per_ip == 0 || ip_count < rules.max_connections_per_ip;
                    if total_room && ip_room {
                        counts.total = counts.total.saturating_add(1);
                        counts.per_ip.insert(ip, ip_count.saturating_add(1));
                        true
                    } else {
                        false
                    }
                };
                if !admitted {
                    continue; // the connection is simply dropped: no thread, no response
                }
                let layout = layout.clone();
                let ledger = Arc::clone(ledger);
                let rules = rules.clone();
                let book = Arc::clone(book);
                let witnesses = Arc::clone(witnesses);
                let on_session = Arc::clone(&on_session);
                let connections = Arc::clone(&rules.connections);
                std::thread::spawn(move || {
                    let release_wall = |connections: &Arc<Mutex<ConnectionCounts>>| {
                        if let Ok(mut counts) = connections.lock() {
                            counts.total = counts.total.saturating_sub(1);
                            if let Some(c) = counts.per_ip.get_mut(&ip) {
                                *c = c.saturating_sub(1);
                                if *c == 0 {
                                    counts.per_ip.remove(&ip);
                                }
                            }
                        }
                    };
                    // P3's deadline is set on the RAW socket, before
                    // P4 might wrap it — it has to bound a stalled TLS
                    // handshake too, not just a stalled declaration.
                    // The clone is held only so `court_session` can
                    // lift it later; it shares the same underlying fd.
                    if let Some(deadline) = rules.handshake_deadline {
                        let _ = stream.set_read_timeout(Some(deadline));
                    }
                    let handshake_socket = stream.try_clone().ok();
                    // P4 — every inbound tag on this listener (claims,
                    // registration, market answers, witness records)
                    // crosses the SAME accept loop, so wrapping here
                    // covers all of them with nothing role-specific.
                    let wire = match &rules.tls {
                        Some(server_tls) => rustls::ServerConnection::new(Arc::clone(server_tls))
                            .map(|conn| {
                                Box::new(rustls::StreamOwned::new(conn, stream)) as Box<dyn ReadWrite>
                            })
                            .map_err(|e| format!("{e:?}")),
                        None => Ok(Box::new(stream) as Box<dyn ReadWrite>),
                    };
                    let wire = match wire {
                        Ok(wire) => wire,
                        Err(e) => {
                            release_wall(&connections);
                            println!("plumbd: TLS setup failed: {e}");
                            return;
                        }
                    };
                    let result = court_session(
                        wire,
                        handshake_socket,
                        &layout,
                        &ledger,
                        &rules,
                        &book,
                        &witnesses,
                    );
                    // Release the wall regardless of how the session
                    // ended — a session that errors still held a slot.
                    release_wall(&connections);
                    match result {
                        Ok(report) => on_session(&report),
                        // The audit's lesson: a session that dies
                        // silently looks identical to a healthy idle
                        // court. Failures say so.
                        Err(e) => println!("plumbd: session failed: {e:?}"),
                    }
                });
            }
            Err(e) => return e,
        }
    }
}

/// Connect as a producer: declare, hear the court's declaration, send
/// every envelope, and close. The envelopes are already whole frames
/// (SDK-built or `isthmus::work`-built); this function moves bytes
/// and never reads them.
pub fn produce(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    envelopes: &[Vec<u8>],
) -> Result<usize, NodeBroken> {
    produce_inner(addr, layout, ledger, holder, bound, envelopes, None, None)
}

/// [`produce`], signing: each envelope is followed by its attestation
/// record (tag 83), built over the envelope's exact frame bytes.
pub fn produce_signed(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    envelopes: &[Vec<u8>],
    key: &sig::Keypair,
) -> Result<usize, NodeBroken> {
    produce_inner(addr, layout, ledger, holder, bound, envelopes, Some(key), None)
}

/// [`produce_signed`], over TLS when `tls_fingerprint` names one (P4)
/// — the fingerprint a peer's own `Act::Certify` recorded. `None`
/// dials plaintext, exactly like [`produce_signed`] — this is the one
/// a role that might OR might not be talking to a certified court
/// reaches for, so it is not two separate decisions to wire in.
#[allow(clippy::too_many_arguments)] // every argument names something the caller must decide independently; a wrapper struct here would group unrelated concerns just to dodge the count
pub fn produce_signed_over_tls(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    envelopes: &[Vec<u8>],
    key: &sig::Keypair,
    tls_fingerprint: Option<[u8; 32]>,
) -> Result<usize, NodeBroken> {
    produce_inner(addr, layout, ledger, holder, bound, envelopes, Some(key), tls_fingerprint)
}

/// P4 — dial a peer and wrap the socket for TLS if `tls_fingerprint`
/// names one. The chain-pinned fingerprint IS the whole trust
/// decision; nothing here validates a hostname or a CA chain, because
/// nothing in this network has either.
fn dial(
    addr: impl ToSocketAddrs,
    tls_fingerprint: Option<[u8; 32]>,
) -> Result<Box<dyn ReadWrite>, NodeBroken> {
    dial_with_timeout(addr, tls_fingerprint, None)
}

/// [`dial`], setting a read timeout on the RAW socket first — a
/// timeout has to bound a stalled TLS handshake too, not just a
/// stalled read afterward, the same reasoning `serve`'s handshake
/// wall applies on the accept side.
fn dial_with_timeout(
    addr: impl ToSocketAddrs,
    tls_fingerprint: Option<[u8; 32]>,
    read_timeout: Option<std::time::Duration>,
) -> Result<Box<dyn ReadWrite>, NodeBroken> {
    let stream = TcpStream::connect(addr)?;
    if let Some(timeout) = read_timeout {
        stream.set_read_timeout(Some(timeout))?;
    }
    let Some(fingerprint) = tls_fingerprint else {
        return Ok(Box::new(stream));
    };
    let ip = stream
        .peer_addr()
        .map(|a| a.ip())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    let config = Arc::new(crate::tls::client_config(fingerprint));
    let conn = rustls::ClientConnection::new(config, rustls::pki_types::ServerName::from(ip))
        .map_err(|e| {
            NodeBroken::Io(std::io::Error::other(format!("TLS setup failed: {e:?}")))
        })?;
    Ok(Box::new(rustls::StreamOwned::new(conn, stream)))
}

/// A client's final act before dropping a connection to a court: say
/// "no more input from me," then drain whatever the court still has
/// queued — a market announcement this role never reads, say — before
/// the actual close. Skipping this leaves data unread in THIS side's
/// own receive buffer at close, which makes the OS send RST instead
/// of FIN; a RST at a record boundary elsewhere reads as a graceful
/// departure, and the cost lands on the COURT, which may not have
/// finished reading the records this side just sent. Every function
/// that writes to a court and then walks away needs this — a market
/// being posted is a fact about the COURT, not about which client
/// role happens to be dialing it.
fn finish_politely(stream: &mut dyn ReadWrite) {
    let _ = stream.shutdown_write();
    let mut sink = [0u8; 4096];
    for _ in 0..1024 {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)] // see produce_signed_over_tls
fn produce_inner(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    envelopes: &[Vec<u8>],
    key: Option<&sig::Keypair>,
    tls_fingerprint: Option<[u8; 32]>,
) -> Result<usize, NodeBroken> {
    let mut stream = dial(addr, tls_fingerprint)?;
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    send_hello(&mut stream, layout, hello_tag(ledger, holder), &ours)?;
    let mut buffer = Vec::new();
    let _court = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    // IS-2/2: the court's challenge follows its declaration. A signing
    // producer answers it — an attestation over the challenge's exact
    // frame bytes — before any work; an unsigned producer reads past it.
    let (_ctag, challenge_frame) = read_record(&mut stream, &mut buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;
    if let Some(key) = key {
        let answer = key.attest(&challenge_frame);
        let mut wire = Vec::new();
        isthmus::frame::put_frame(
            layout,
            admission::ATTESTATION_TAG,
            &answer.encode(),
            &mut wire,
        )
        .map_err(|_| NodeBroken::CannotFrame)?;
        stream.write_all(&wire)?;
    }
    let mut sent = 0usize;
    for envelope in envelopes {
        stream.write_all(envelope)?;
        if let Some(key) = key {
            let attestation = key.attest(envelope);
            let mut wire = Vec::new();
            isthmus::frame::put_frame(
                layout,
                admission::ATTESTATION_TAG,
                &attestation.encode(),
                &mut wire,
            )
            .map_err(|_| NodeBroken::CannotFrame)?;
            stream.write_all(&wire)?;
        }
        sent += 1;
    }
    finish_politely(&mut stream);
    Ok(sent)
}

/// Join a live network in one connection: declare with no deed (there
/// is none yet — a fresh empty ledger is exactly the right thing to
/// declare with), prove possession of a freshly generated key over
/// THIS session's own challenge (registering, not admitting), and —
/// once the court's ack shows the bind landed — send one signed claim
/// on the SAME connection through the now-ordinary admission path.
/// Registration and first credit in one run: no restart, no
/// re-genesis, no operator hand-editing a config.
pub fn register_and_produce(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    holder: &str,
    bound: usize,
    key: &sig::Keypair,
    envelope: &[u8],
    tls_fingerprint: Option<[u8; 32]>,
) -> Result<crate::registration::RegisterOutcome, NodeBroken> {
    let empty = Ledger::new(Layout::founding());
    let mut stream = dial(addr, tls_fingerprint)?;
    let ours = Hello::of(&empty, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    send_hello(&mut stream, layout, hello_tag(&empty, holder), &ours)?;
    let mut buffer = Vec::new();
    let _court = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    let (_ctag, challenge_frame) = read_record(&mut stream, &mut buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;

    let request = crate::registration::RegisterRequest {
        holder: holder.to_owned(),
        scheme: sig::SCHEME_ED25519_BLAKE3,
        key: key.public(),
    };
    let mut request_wire = Vec::new();
    isthmus::frame::put_frame(
        layout,
        crate::registration::REGISTER_TAG,
        &request.encode(),
        &mut request_wire,
    )
    .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&request_wire)?;

    let proof = key.attest(&challenge_frame);
    let mut proof_wire = Vec::new();
    isthmus::frame::put_frame(layout, admission::ATTESTATION_TAG, &proof.encode(), &mut proof_wire)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&proof_wire)?;

    let Some((ack_tag, ack_frame)) = read_record(&mut stream, &mut buffer, layout, bound)? else {
        return Err(NodeBroken::Unsatisfiable); // closed with no ack: the court refused
    };
    if ack_tag != crate::registration::REGISTER_TAG {
        return Err(NodeBroken::Unsatisfiable);
    }
    let outcome = crate::registration::RegisterOutcome::decode(
        ack_frame.get(layout.header()..).unwrap_or(&[]),
    )
    .map_err(|_| NodeBroken::Unsatisfiable)?;

    // Bound now — the SAME connection's challenge was already spent
    // proving possession, so this claim answers admission the
    // ordinary way: envelope, then the attestation over it.
    stream.write_all(envelope)?;
    let attestation = key.attest(envelope);
    let mut wire = Vec::new();
    isthmus::frame::put_frame(layout, admission::ATTESTATION_TAG, &attestation.encode(), &mut wire)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&wire)?;
    finish_politely(&mut stream);

    Ok(outcome)
}

/// One carried session: records from `client` forwarded to `upstream`
/// **unread** — the carrier's whole capability, and its whole limit.
///
/// The carrier declares itself to both sides; after the declarations
/// it never decodes another value. What a carrier cannot read, a
/// carrier cannot front-run.
pub fn carrier_session(
    mut client: TcpStream,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    upstream: impl ToSocketAddrs,
    upstream_tls_fingerprint: Option<[u8; 32]>,
) -> Result<usize, NodeBroken> {
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));

    // Face the client: hear their declaration, answer with ours. The
    // client-facing leg stays plaintext in this pass — P4 covers a
    // court's own listener and every direct court-facing dial; a
    // carrier's downstream trust is a narrower, separate concern
    // (it forwards unread, decodes nothing, and never holds a key).
    let mut client_buf = Vec::new();
    let _client_hello = read_hello(&mut client, &mut client_buf, layout, &ours, bound)?;
    send_hello(&mut client, layout, hello_tag(ledger, holder), &ours)?;

    // Face upstream: declare, hear the court. THIS leg is a plain
    // court-facing dial like any other — it gets TLS the same way.
    let mut court = dial(upstream, upstream_tls_fingerprint)?;
    send_hello(&mut court, layout, hello_tag(ledger, holder), &ours)?;
    let mut court_buf = Vec::new();
    let _court_hello = read_hello(&mut court, &mut court_buf, layout, &ours, bound)?;

    // IS-2/2: the court's session challenge follows its declaration.
    // Relay it to the client VERBATIM — the client's answer signs the
    // exact frame bytes, so carriage costs the freshness nothing,
    // for the same reason it costs the signature nothing.
    if let Some((_tag, challenge)) = read_record(&mut court, &mut court_buf, layout, bound)? {
        client.write_all(&challenge)?;
    }

    // Forward whole frames, unread, in order.
    let mut forwarded = 0usize;
    while let Some((_tag, frame)) = read_record(&mut client, &mut client_buf, layout, bound)? {
        court.write_all(&frame)?;
        forwarded += 1;
    }
    // The client is gone — no more records will ever cross this leg.
    // finish_politely tells the court so and drains whatever it still
    // sends (a market announcement this carrier never relays, say)
    // before the drop — the same reasoning as every other role that
    // dials a court and then walks away.
    finish_politely(&mut court);
    Ok(forwarded)
}

/// Accept and carry forever, one thread per client session.
#[allow(clippy::too_many_arguments)] // see produce_signed_over_tls
pub fn carry(
    listener: &TcpListener,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    upstream: String,
    upstream_tls_fingerprint: Option<[u8; 32]>,
    on_session: impl Fn(usize) + Send + Sync + 'static,
) -> std::io::Error {
    let on_session = Arc::new(on_session);
    loop {
        match listener.accept() {
            Ok((stream, _peer)) => {
                let layout = layout.clone();
                let ledger = ledger.clone();
                let holder = holder.to_owned();
                let upstream = upstream.clone();
                let on_session = Arc::clone(&on_session);
                std::thread::spawn(move || {
                    if let Ok(forwarded) = carrier_session(
                        stream,
                        &layout,
                        &ledger,
                        &holder,
                        bound,
                        &upstream,
                        upstream_tls_fingerprint,
                    ) {
                        on_session(forwarded);
                    }
                });
            }
            Err(e) => return e,
        }
    }
}

/// Attach and put witness frames on a court's record (IS-4). The
/// fourth role: a peer that produces nothing and verifies nothing
/// still attests to what crossed.
#[allow(clippy::too_many_arguments)] // see produce_signed_over_tls
pub fn witness_to(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    witnesses: &[isthmus::witness::Witness],
    key: Option<&sig::Keypair>,
    tls_fingerprint: Option<[u8; 32]>,
) -> Result<usize, NodeBroken> {
    let mut stream = dial(addr, tls_fingerprint)?;
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    send_hello(&mut stream, layout, hello_tag(ledger, holder), &ours)?;
    let mut buffer = Vec::new();
    let _court = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    let (_ctag, challenge_frame) = read_record(&mut stream, &mut buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;
    if let Some(key) = key {
        let answer = key.attest(&challenge_frame);
        let mut wire = Vec::new();
        isthmus::frame::put_frame(
            layout,
            admission::ATTESTATION_TAG,
            &answer.encode(),
            &mut wire,
        )
        .map_err(|_| NodeBroken::CannotFrame)?;
        stream.write_all(&wire)?;
    }
    let mut sent = 0usize;
    for witness in witnesses {
        let mut wire = Vec::new();
        isthmus::frame::put_frame(layout, witnessing::WITNESS_TAG, &witness.encode(), &mut wire)
            .map_err(|_| NodeBroken::CannotFrame)?;
        stream.write_all(&wire)?;
        sent += 1;
    }
    finish_politely(&mut stream);
    Ok(sent)
}

/// Answer a court's posted question NATIVELY: attach, answer the
/// challenge, hear the query announcement (tag 85), send the claim,
/// and take the signed receipt back on the wire (tag 81). The whole
/// x402 loop with zero HTTP — the gateway edge exists only for
/// payers who cannot speak Plumbline.
#[allow(clippy::too_many_arguments)] // see produce_signed_over_tls
pub fn solve_market(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    body: &[u8],
    key: &sig::Keypair,
    tls_fingerprint: Option<[u8; 32]>,
) -> Result<(Query, receipt::SignedReceipt), NodeBroken> {
    // A court that refuses a market answer sends nothing back — so a
    // solver waiting unbounded for its receipt cannot tell refusal
    // from slowness. The deadline makes silence an answer.
    let mut stream = dial_with_timeout(addr, tls_fingerprint, Some(std::time::Duration::from_secs(10)))?;
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    send_hello(&mut stream, layout, hello_tag(ledger, holder), &ours)?;
    let mut buffer = Vec::new();
    let _court = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    let (_ctag, challenge_frame) = read_record(&mut stream, &mut buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;
    let answer = key.attest(&challenge_frame);
    let mut wire = Vec::new();
    isthmus::frame::put_frame(layout, admission::ATTESTATION_TAG, &answer.encode(), &mut wire)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&wire)?;

    // The question, announced.
    let (qtag, qframe) = read_record(&mut stream, &mut buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;
    if qtag != QUERY_TAG {
        return Err(NodeBroken::NoDeclaration);
    }
    let query = Query::decode(qframe.get(layout.header()..).unwrap_or(&[]))
        .map_err(|_| NodeBroken::NoDeclaration)?;

    // The answer, enveloped and attested like any claim.
    let mut envelope = Vec::new();
    isthmus::work::put_shape_claim(body, &mut envelope)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&envelope)?;
    let attestation = key.attest(&envelope);
    let mut wire = Vec::new();
    isthmus::frame::put_frame(
        layout,
        admission::ATTESTATION_TAG,
        &attestation.encode(),
        &mut wire,
    )
    .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&wire)?;

    // The receipt, on the same wire.
    let (rtag, rframe) = read_record(&mut stream, &mut buffer, layout, bound)?
        .ok_or(NodeBroken::NoDeclaration)?;
    if rtag != isthmus::work::RECEIPT_TAG {
        return Err(NodeBroken::NoDeclaration);
    }
    let value = rframe.get(layout.header()..).unwrap_or(&[]);
    let split = value.len().saturating_sub(sig::ATTESTATION_LEN);
    let parsed = receipt::Receipt::decode(value.get(..split).unwrap_or(&[]))
        .map_err(|_| NodeBroken::NoDeclaration)?;
    let attestation = sig::Attestation::decode(value.get(split..).unwrap_or(&[]))
        .map_err(|_| NodeBroken::NoDeclaration)?;
    Ok((
        query,
        receipt::SignedReceipt {
            receipt: parsed,
            attestation,
        },
    ))
}
