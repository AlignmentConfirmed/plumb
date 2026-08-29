//! The node: isthmus sessions over TCP.
//!
//! ```text
//! producer ──TCP──► court
//!   1. declaration        the first record on the edge, both ways (IS-5)
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
use std::sync::{Arc, Mutex, RwLock};

use isthmus::deed::Ledger;
use isthmus::hello::Hello;
use isthmus::layout::{Layout, Tag};
use isthmus::session::{self, Step};

use crate::admission;
use crate::bounty::{settle_answer, Bounty};
use crate::sched::Governor as _;
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
    /// Signal "no more writes coming" without fully closing — so a
    /// peer still mid-read-loop gets a clean end to its input rather
    /// than a drop. Needed by a relay (the carrier) that must not
    /// close its upstream leg while records it forwarded might still
    /// be unread by the peer: closing a socket with unread data of
    /// its own still queued sends RST instead of FIN, and RST at a
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
                    // with unread data in its buffer (e.g. a fixture
                    // producer that never reads the market
                    // announcement) — TCP sends RST instead of FIN.
                    // P4's TLS wrap surfaces the same shape of
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
    /// P3 — no more than this many sessions held at once from one IP.
    /// `0` is unwalled.
    pub max_connections_per_ip: usize,
    /// P3 — a connection that has not sent its declaration within
    /// this long is dropped before it ever reaches the work loop.
    /// `None` is unwalled: `read_hello` blocks forever, as it always
    /// has. A bare TCP accept spawns a thread before any check runs —
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
    /// since they all cross the same accept loop.
    pub tls: Option<Arc<rustls::ServerConfig>>,
}

/// How many sessions a court is holding right now: in total, and per
/// peer IP. Checked and updated only at accept time and at a
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
/// The stream-free outcome of settling one work value: the receipt frame
/// to write back (if any), and how it counted. Verification produces this
/// on a settler pool ordered by the transport metric; the reader that owns
/// the connection writes `receipt` and applies the counts — so the
/// expensive homological work is scheduled, not the cheap I/O.
struct SettleOutcome {
    receipt: Option<Vec<u8>>,
    credited: bool,
    refused: bool,
}

impl SettleOutcome {
    fn refused() -> Self {
        Self { receipt: None, credited: false, refused: true }
    }
}

/// Build the signed receipt frame for a settled market answer.
fn receipt_for(
    layout: &Layout,
    market: &MarketPost,
    epoch: u64,
    answer: &crate::bounty::Answer,
) -> Option<Vec<u8>> {
    let signed = receipt::issue(
        &market.court,
        epoch,
        market.query.query_id(),
        answer.credit.work_id.as_bytes(),
        answer.credit.axes.components(),
        answer.payout,
        &market.key,
    );
    let mut body = signed.receipt.encode();
    body.extend_from_slice(&signed.attestation.encode());
    let mut wire = Vec::new();
    isthmus::frame::put_frame(layout, isthmus::work::RECEIPT_TAG, &body, &mut wire)
        .ok()
        .map(|()| wire)
}

/// Settle one work value against a locked book — verify and commit together
/// under the caller's lock. The single-shot `court_settle`/greeter paths use
/// this; the settler pool uses [`settle_work_parallel`], which verifies off
/// the lock.
fn settle_work(
    layout: &Layout,
    rules: &SessionRules,
    book: &mut RewardBook,
    value: &[u8],
) -> SettleOutcome {
    if let Some(market) = &rules.market {
        match settle_answer(&market.bounty, &market.query, value, book) {
            Ok(answer) => {
                let epoch = book.open_epoch().unwrap_or(0);
                return SettleOutcome {
                    receipt: receipt_for(layout, market, epoch, &answer),
                    credited: true,
                    refused: false,
                };
            }
            Err(crate::bounty::AnswerRefused::NotThePosersUniverse)
            | Err(crate::bounty::AnswerRefused::NotAProof)
            | Err(crate::bounty::AnswerRefused::NotDeclared(_)) => {
                // Ordinary work (both market shapes' "didn't engage with
                // what was posed") — fall through to the plain credit.
            }
            Err(_) => return SettleOutcome::refused(),
        }
    }
    match book.credit_claim(value) {
        Ok(_) => SettleOutcome { receipt: None, credited: true, refused: false },
        Err(_) => SettleOutcome::refused(),
    }
}

/// Settle one work value with the book behind an `RwLock`, **verifying
/// under a shared read lock** (#61/#66): the expensive metered
/// re-derivation runs while other settlers verify concurrently (many
/// readers hold the read lock at once), and the atomic commit takes the
/// exclusive write lock only for its cheap step. Authority is the
/// commit-with-replay-check; verification is pure, so concurrent — even
/// duplicated — verifies are harmless. `seen` is read directly under the
/// shared lock (no snapshot copy); settled work is monotonic, so a
/// dependency present at verify is still present at commit.
fn settle_work_parallel(
    layout: &Layout,
    rules: &SessionRules,
    book: &Arc<RwLock<RewardBook>>,
    section: &Arc<Mutex<crate::section::Section>>,
    value: &[u8],
) -> SettleOutcome {
    let credit_plain = || match book.write() {
        Ok(mut guard) => match guard.credit_claim(value) {
            Ok(_) => SettleOutcome { receipt: None, credited: true, refused: false },
            Err(_) => SettleOutcome::refused(),
        },
        Err(_) => SettleOutcome::refused(),
    };
    let Some(market) = &rules.market else {
        return credit_plain();
    };
    // Verify under the shared read lock — concurrent with other verifiers.
    // The guard is dropped at the end of this block, before the write lock.
    let verified = {
        let guard = match book.read() {
            Ok(guard) => guard,
            Err(_) => return SettleOutcome::refused(),
        };
        crate::bounty::verify_answer(&market.bounty, &market.query, value, guard.seen())
    };
    match verified {
        Ok(verified) => {
            // Commit under the exclusive write lock — the one atomic act.
            let mut guard = match book.write() {
                Ok(guard) => guard,
                Err(_) => return SettleOutcome::refused(),
            };
            match crate::bounty::commit_answer(&mut guard, value, verified) {
                Ok(answer) => {
                    let epoch = guard.open_epoch().unwrap_or(0);
                    drop(guard); // release the book before the section deposit
                    // §6h: the settled claim is deposited into the convergence
                    // section, built forward from settlement — the section is
                    // the convergent state itself, not derived from the log.
                    deposit_section(section, value, market.query.domain_tag);
                    SettleOutcome {
                        receipt: receipt_for(layout, market, epoch, &answer),
                        credited: true,
                        refused: false,
                    }
                }
                Err(_) => SettleOutcome::refused(),
            }
        }
        Err(crate::bounty::AnswerRefused::NotThePosersUniverse)
        | Err(crate::bounty::AnswerRefused::NotAProof)
        | Err(crate::bounty::AnswerRefused::NotDeclared(_)) => credit_plain(),
        Err(_) => SettleOutcome::refused(),
    }
}

/// Deposit a settled claim into the **convergence section** (§6h): a unit per
/// axis of each grade it engages — torsion axes converge (mod `m`, finite),
/// free axes accumulate (the market). Built forward from settlement (the
/// domain `tag` and the claim's geometry are in hand at commit), so it needs
/// no change to the act format — the section IS the convergent state, not a
/// thing reconstructed from a replicated log (that was consensus thinking).
/// The exact SNF (`grade_shapes`) runs here, off the book lock, once per
/// settled claim — the book's exact leg. Logs when a new grade first appears
/// (a genuine read of the growing convergent state).
fn deposit_section(section: &Arc<Mutex<crate::section::Section>>, value: &[u8], tag: u64) {
    for ((tag, dim), shape) in crate::geometry::claim_grades(value, tag) {
        let delta = crate::section::AxialCredit::of(
            vec![1i128; shape.free_rank],
            vec![1u64; shape.torsion.len()],
        );
        if let Ok(mut guard) = section.lock() {
            let is_new = guard.at((tag, dim)).is_none();
            guard.deposit((tag, dim), &shape, &delta);
            if is_new {
                println!(
                    "plumbd: convergence section grew to {} grade(s) — new (domain {tag}, dim {dim})",
                    guard.spanned()
                );
            }
        }
    }
}

/// Per-domain graded torsion, memoized per tag and looked up in O(1). The
/// value is the **fast leg** (`graded_torsion` → `betti_fast`, field ranks
/// over `𝔽_p`), so no integer Smith Normal Form runs on the dispatch path —
/// or anywhere in scheduling. A tag with no registered declaration
/// (ordinary work) caches an empty vector (a flat lift).
#[derive(Default)]
struct TorsionCache {
    inner: Mutex<std::collections::HashMap<u64, std::sync::Arc<[u64]>>>,
}

impl TorsionCache {
    /// The graded torsion for `tag`: a cached lookup, computed once from
    /// the registered universe the first time the domain is seen.
    fn torsion_of(&self, tag: u64, ledger: &Arc<Mutex<Ledger>>) -> std::sync::Arc<[u64]> {
        if let Ok(cache) = self.inner.lock() {
            if let Some(torsion) = cache.get(&tag) {
                return std::sync::Arc::clone(torsion);
            }
        }
        // Miss: resolve the declared complex once.
        let complex = ledger
            .lock()
            .ok()
            .and_then(|guard| guard.declaration_of(tag))
            .and_then(|bytes| assay::complex::DeclaredComplex::decode(&bytes).ok());
        // #70: flag an exotic universe the first time this court resolves it
        // — impossible in a crystallographic domain, so it is an anomaly
        // signal worth surfacing. Never a rejection: the book's exact SNF
        // settles any torsion regardless.
        if let Some(c) = &complex {
            if let crate::geometry::GradeClass::Exotic { order } = crate::geometry::classify(c) {
                println!(
                    "plumbd: domain {tag} carries exotic torsion (order {order}); \
                     betti_fast is a hint here, the book's exact SNF is the authority"
                );
            }
        }
        let torsion: std::sync::Arc<[u64]> = complex
            .map(|complex| crate::geometry::graded_torsion(&complex).into())
            .unwrap_or_else(|| Vec::new().into());
        if let Ok(mut cache) = self.inner.lock() {
            cache.insert(tag, std::sync::Arc::clone(&torsion));
        }
        torsion
    }
}

/// A settlement job on the work-transit: the value to verify and a channel
/// back to the reader that owns the connection (which writes the receipt).
struct WorkJob {
    value: Vec<u8>,
    reply: std::sync::mpsc::SyncSender<SettleOutcome>,
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
    /// Watched witnesses whose subjects failed re-derivation — a
    /// dispute on the record.
    pub disputed: usize,
    /// Live registrations that landed this session (P2) — an unbound
    /// key proved possession and was issued a deed and bound, with no
    /// restart and no re-genesis.
    pub registered: usize,
}

/// The state a completed [`court_handshake`] hands to [`court_settle`]:
/// the live stream and everything the record loop reads from, captured
/// once so the two halves compose exactly as the old single-shot
/// session did. The read `buffer` crosses the seam on purpose —
/// `read_hello` may leave residual bytes the first `read_record`
/// continues from, so framing continuity is preserved. Phase 5's
/// greeter pool runs the handshake and enqueues this state by weight;
/// the settler pool drains it.
pub struct SessionState<'a> {
    stream: Box<dyn ReadWrite>,
    layout: &'a Layout,
    ledger: &'a Arc<Mutex<Ledger>>,
    rules: &'a SessionRules,
    book: &'a Arc<RwLock<RewardBook>>,
    witnesses: &'a WitnessLog,
    /// The IS-2/2 challenge frame this session issued — the token an
    /// admission attestation must answer.
    challenge_frame: Vec<u8>,
    /// The read buffer, carried across the seam (see the type note).
    buffer: Vec<u8>,
}

/// The cheap, bounded first half of a court session: declaration both
/// ways and the IS-2/2 challenge, plus the native market announcement.
/// Everything here is covered by `serve`'s handshake deadline; nothing
/// here surveys or settles. Returns the [`SessionState`] the expensive
/// half reads from.
pub fn court_handshake<'a>(
    mut stream: Box<dyn ReadWrite>,
    handshake_socket: Option<TcpStream>,
    layout: &'a Layout,
    ledger: &'a Arc<Mutex<Ledger>>,
    rules: &'a SessionRules,
    book: &'a Arc<RwLock<RewardBook>>,
    witnesses: &'a WitnessLog,
) -> Result<SessionState<'a>, NodeBroken> {
    let (holder, bound) = (rules.holder.as_str(), rules.bound);
    // A snapshot for framing our own declaration and challenge tag —
    // read once, at session start, exactly as a per-thread clone did
    // before this ledger could change live. Admission checks in
    // `court_settle` re-lock for the current state on purpose: a
    // registration that lands mid-session must be visible to that same
    // session's next claim.
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

    // The native market: the question is announced on the wire the
    // session already speaks. HTTP exists only for foreign payers, at
    // the gateway edge; a Plumbline solver never touches it.
    if let Some(market) = &rules.market {
        let mut announcement = Vec::new();
        isthmus::frame::put_frame(layout, QUERY_TAG, &market.query.encode(), &mut announcement)
            .map_err(|_| NodeBroken::CannotFrame)?;
        stream.write_all(&announcement)?;
    }

    Ok(SessionState {
        stream,
        layout,
        ledger,
        rules,
        book,
        witnesses,
        challenge_frame,
        buffer,
    })
}

/// One court session's record loop, factored so the same per-record
/// dispatch runs whether a single worker pumps every frame
/// ([`court_settle`]) or a greeter pumps until the session goes live and
/// a settler pumps the rest (Phase 5's two-pool [`serve`]). Byte
/// behaviour is identical either way: the split is only where the pump
/// runs, never what it does per frame — so the state that crosses the
/// greeter→settler seam is exactly this struct.
#[derive(Default)]
struct Survey {
    report: SessionReport,
    session_live: bool,
    pending: Option<Vec<u8>>,
    seen_envelopes: Vec<Vec<u8>>,
    pending_register: Option<crate::registration::RegisterRequest>,
    /// The holder the go-live attestation authenticated, captured for
    /// escrow-weighted scheduling (Phase 5). `None` until the session is
    /// live, and on a non-enforce court (no attestation) or a live
    /// registration (a fresh key with no escrow yet) — all of which
    /// weight at the base floor.
    holder: Option<String>,
}

/// Whether the record loop should read the next frame or stop the
/// session (the refused-registration close).
enum Flow {
    Continue,
    Stop,
}

impl Survey {
    fn new(session_live: bool) -> Self {
        Self {
            session_live,
            ..Self::default()
        }
    }

    /// Write a settlement's receipt (if any) to the connection this
    /// session owns and apply its counts. The verification that produced
    /// the outcome may have run on another thread (scheduled by the
    /// transport metric); the connection write always happens here.
    fn apply_settlement(
        &mut self,
        stream: &mut Box<dyn ReadWrite>,
        outcome: SettleOutcome,
    ) -> Result<(), NodeBroken> {
        if let Some(receipt) = &outcome.receipt {
            stream.write_all(receipt)?;
        }
        if outcome.credited {
            self.report.credited += 1;
        }
        if outcome.refused {
            self.report.refused += 1;
        }
        Ok(())
    }

    /// Dispatch one already-read record. Returns [`Flow::Stop`] only for
    /// the refused-registration case that closes the session (exactly as
    /// the single-shot loop's early `return` did); every other outcome
    /// is [`Flow::Continue`] (a per-frame `continue` or fall-through).
    #[allow(clippy::too_many_arguments)] // the shared court state a record needs
    fn pump_one(
        &mut self,
        tag: Tag,
        frame: Vec<u8>,
        stream: &mut Box<dyn ReadWrite>,
        layout: &Layout,
        ledger: &Arc<Mutex<Ledger>>,
        rules: &SessionRules,
        book: &Arc<RwLock<RewardBook>>,
        witnesses: &WitnessLog,
        challenge_frame: &[u8],
        settle: &dyn Fn(&[u8]) -> SettleOutcome,
    ) -> Result<Flow, NodeBroken> {
        let enforce = rules.enforce;
        if isthmus::work::is_work_tag(tag) && tag != isthmus::work::RECEIPT_TAG {
            if !enforce {
                let value = frame.get(layout.header()..).unwrap_or(&[]).to_vec();
                remember(&mut self.seen_envelopes, frame.clone());
                let outcome = settle(&value);
                self.apply_settlement(stream, outcome)?;
            } else if !self.session_live {
                self.report.refused += 1; // work before the challenge was answered
            } else {
                if self.pending.take().is_some() {
                    self.report.refused += 1; // unattested envelope displaced
                }
                self.pending = Some(frame);
            }
        } else if rules.register && tag == crate::registration::REGISTER_TAG {
            if self.session_live {
                self.report.skipped += 1; // registration is a pre-admission act only
            } else {
                match crate::registration::RegisterRequest::decode(
                    frame.get(layout.header()..).unwrap_or(&[]),
                ) {
                    Ok(request) => {
                        if self.pending_register.replace(request).is_some() {
                            self.report.refused += 1; // a second request displaced the first
                        }
                    }
                    Err(_) => self.report.refused += 1,
                }
            }
        } else if enforce && tag == admission::ATTESTATION_TAG {
            if !self.session_live {
                let attestation_bytes = frame.get(layout.header()..).unwrap_or(&[]);
                if let Some(request) = self.pending_register.take() {
                    // P2: proof of possession over this session's own
                    // challenge, then the ledger-level rules — never
                    // the ordinary admission path, which would
                    // (correctly) refuse a key that is not bound yet.
                    let outcome = (|| {
                        crate::registration::verify_possession(
                            &request,
                            challenge_frame,
                            attestation_bytes,
                        )
                        .map_err(|_| ())?;
                        let epoch = {
                            let guard =
                                book.read().map_err(|_| ())?;
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
                            self.session_live = true;
                            self.report.registered += 1;
                        }
                        // A refused registration closes here, same as
                        // a court that refuses a market answer: the
                        // client waiting on a response cannot tell
                        // refusal from slowness any other way, and
                        // this session's challenge is already spent —
                        // looping back to await a new attestation
                        // would just deadlock a client that is, in
                        // turn, waiting on this ack.
                        Err(()) => {
                            self.report.refused += 1;
                            return Ok(Flow::Stop);
                        }
                    }
                    return Ok(Flow::Continue);
                }
                // The first attestation must answer this session's
                // challenge. A stale answer — a replayed session —
                // refuses, and the session never goes live.
                let epoch = {
                    let guard = book.read().map_err(|_| NodeBroken::CourtUnreachable)?;
                    guard.open_epoch().unwrap_or(0)
                };
                let admitted = {
                    let guard = ledger.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                    admission::admit(&guard, epoch, challenge_frame, attestation_bytes)
                };
                match admitted {
                    // Go live and remember who: the holder the escrow
                    // scheduler will weight this session by (Phase 5).
                    Ok(holder) => {
                        self.session_live = true;
                        self.holder = Some(holder);
                    }
                    Err(_) => self.report.refused += 1,
                }
                return Ok(Flow::Continue);
            }
            let Some(envelope) = self.pending.take() else {
                self.report.skipped += 1; // attestation with nothing to attest
                return Ok(Flow::Continue);
            };
            let attestation = frame.get(layout.header()..).unwrap_or(&[]);
            let guard = book.read().map_err(|_| NodeBroken::CourtUnreachable)?;
            let epoch = guard.open_epoch().unwrap_or(0);
            let admitted = {
                let ledger_guard = ledger.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                admission::admit(&ledger_guard, epoch, &envelope, attestation)
            };
            match admitted {
                Ok(_holder) => {
                    let value = envelope.get(layout.header()..).unwrap_or(&[]).to_vec();
                    remember(&mut self.seen_envelopes, envelope.clone());
                    drop(guard);
                    // Settlement is scheduled off this thread by the
                    // transport metric (or run inline on a single-shot
                    // session); either way it hands back the receipt bytes.
                    let outcome = settle(&value);
                    self.apply_settlement(stream, outcome)?;
                }
                Err(_) => self.report.refused += 1,
            }
        } else if ledger
            .lock()
            .map_err(|_| NodeBroken::CourtUnreachable)?
            .declaration_of(tag)
            .is_some()
        {
            // UC4, live: a tag with a registered definition on this
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
                    let mut guard = book.write().map_err(|_| NodeBroken::CourtUnreachable)?;
                    match guard.credit_claim(value) {
                        Ok(_) => self.report.credited += 1,
                        Err(_) => self.report.refused += 1,
                    }
                }
                Err(_) => self.report.refused += 1,
            }
        } else if tag == witnessing::WITNESS_TAG {
            // IS-4: a witness put something on the record. The court
            // keeps it — decoded (refuse-not-repair), never judged
            // here; judging is a watcher's act, and a watcher is
            // handed its subject elsewhere.
            let value = frame.get(layout.header()..).unwrap_or(&[]);
            match isthmus::witness::Witness::decode(value) {
                Ok(witness) => {
                    // The watcher, live (IS-4 §6): if the witnessed
                    // subject crossed this session, the court was
                    // handed it — so it watches, and a failed
                    // re-derivation is a dispute on the record.
                    if let Some(subject) = self
                        .seen_envelopes
                        .iter()
                        .find(|e| witnessing::subject_of(e) == witness.subject)
                    {
                        if let Ok(verdict) = witnessing::watch(&witness, subject) {
                            self.report.watched += 1;
                            if !verdict.verified {
                                self.report.disputed += 1;
                            }
                        }
                    }
                    let mut log = witnesses
                        .lock()
                        .map_err(|_| NodeBroken::CourtUnreachable)?;
                    log.push(witness);
                    self.report.witnessed += 1;
                }
                Err(_) => self.report.refused += 1,
            }
        } else {
            self.report.skipped += 1;
        }
        Ok(Flow::Continue)
    }
}

/// Pump every remaining record through [`Survey::pump_one`] to the end
/// of the session, then finalize. Shared by the single-shot
/// [`court_settle`] and the settler pool: whatever `survey` state it is
/// handed (fresh, or already advanced past go-live by a greeter), it
/// drains the rest identically.
#[allow(clippy::too_many_arguments)] // the shared court state a session needs
fn drain_records(
    mut survey: Survey,
    stream: &mut Box<dyn ReadWrite>,
    buffer: &mut Vec<u8>,
    layout: &Layout,
    ledger: &Arc<Mutex<Ledger>>,
    rules: &SessionRules,
    book: &Arc<RwLock<RewardBook>>,
    witnesses: &WitnessLog,
    challenge_frame: &[u8],
    settle: &dyn Fn(&[u8]) -> SettleOutcome,
) -> Result<SessionReport, NodeBroken> {
    let bound = rules.bound;
    while let Some((tag, frame)) = read_record(stream, buffer, layout, bound)? {
        match survey.pump_one(
            tag,
            frame,
            stream,
            layout,
            ledger,
            rules,
            book,
            witnesses,
            challenge_frame,
            settle,
        )? {
            Flow::Continue => {}
            Flow::Stop => return Ok(survey.report),
        }
    }
    if survey.pending.is_some() {
        survey.report.refused += 1; // session closed on an unattested envelope
    }
    Ok(survey.report)
}

/// The expensive second half: survey every record and settle it against
/// the shared book. The value is decoded by the book, never here — the
/// session layer stays payload-blind about everything but the tag. A
/// single worker pumps every frame here; the two-pool [`serve`] pumps
/// the same [`Survey`] from two threads instead (greeter to go-live,
/// settler for the rest).
pub fn court_settle(state: SessionState) -> Result<SessionReport, NodeBroken> {
    let SessionState {
        mut stream,
        layout,
        ledger,
        rules,
        book,
        witnesses,
        challenge_frame,
        mut buffer,
    } = state;
    let survey = Survey::new(!rules.enforce);
    // Single-shot: settle inline under the write lock, no metric scheduling.
    let settle = |value: &[u8]| match book.write() {
        Ok(mut guard) => settle_work(layout, rules, &mut guard, value),
        Err(_) => SettleOutcome::refused(),
    };
    drain_records(
        survey,
        &mut stream,
        &mut buffer,
        layout,
        ledger,
        rules,
        book,
        witnesses,
        &challenge_frame,
        &settle,
    )
}

/// Serve one inbound session as the court: the handshake then the
/// settlement, composed exactly as the single-shot session always ran.
/// Callers that want to schedule between the two halves (Phase 5's
/// two-pool serve) call [`court_handshake`] and [`court_settle`]
/// directly; everyone else keeps this one-call surface.
pub fn court_session(
    stream: Box<dyn ReadWrite>,
    handshake_socket: Option<TcpStream>,
    layout: &Layout,
    ledger: &Arc<Mutex<Ledger>>,
    rules: &SessionRules,
    book: &Arc<RwLock<RewardBook>>,
    witnesses: &WitnessLog,
) -> Result<SessionReport, NodeBroken> {
    let state = court_handshake(stream, handshake_socket, layout, ledger, rules, book, witnesses)?;
    court_settle(state)
}

/// A session past its handshake and (when the court enforces)
/// authenticated, waiting for a settler. Carries only owned state so it
/// can cross the greeter→settler thread boundary; the settler supplies
/// the shared court handles from its own clones.
struct LiveSession {
    stream: Box<dyn ReadWrite>,
    challenge_frame: Vec<u8>,
    buffer: Vec<u8>,
    survey: Survey,
    ip: std::net::IpAddr,
}

/// What the greeter made of one connection.
enum GreetOutcome {
    /// Handshake done, session live, holder known — hand it to a settler.
    /// Boxed: this variant is far larger than the others.
    Live(Box<LiveSession>),
    /// The session already ended in the greeter (a refused registration,
    /// or a peer that closed before going live): nothing to settle, the
    /// report is final.
    Done(SessionReport),
    /// The session errored in the greeter (transport gone, court
    /// unreachable): log and release, nothing to settle.
    Errored(NodeBroken),
    /// TLS setup failed before any session existed (already logged).
    Failed,
}

/// Accept sessions forever; two bounded pools serve them, one shared
/// book. Returns only on listener failure. `on_session` sees each
/// session's report — the binary logs it; a test asserts on the book.
///
/// The two-stage scheduler (sched Phase 5): a greeter pool runs the
/// cheap handshake and reads until the session goes live — learning the
/// holder — then enqueues the authenticated session into a per-holder
/// fair queue; a settler pool drains that queue and runs the expensive
/// survey/credit. The greeter pool sits behind the Phase 1–3 per-IP wall
/// (anti-flood before identity is known); the settler queue is keyed by
/// holder — an economic identity, never the IP — which is the seam an
/// escrow weight plugs into (#55). Load still decides only who is served
/// and when, never what work is worth (sched's boundary test).
pub fn serve(
    listener: &TcpListener,
    layout: &Layout,
    ledger: &Arc<Mutex<Ledger>>,
    rules: &SessionRules,
    book: &Arc<RwLock<RewardBook>>,
    witnesses: &WitnessLog,
    on_session: impl Fn(&SessionReport) + Send + Sync + 'static,
) -> std::io::Error {
    let on_session = Arc::new(on_session);

    let governor = crate::sched::StaticGovernor::tuned();
    // The settler pool carries the CPU bound: at most `worker_target`
    // expensive settlements run at once, so a court never pegs the
    // machine. The greeter pool is smaller and I/O-bound (handshake +
    // the go-live read); a greeter blocked on a slow attestation no
    // longer holds a settlement slot — which the single pool could not
    // say.
    let settler_count = governor.worker_target();
    let greeter_count = 1.max(settler_count / 2);

    // Accept → per-IP fair queue (Phase 1–3, anti-flood) → greeter.
    let greeter_queue: Arc<crate::sched::FairQueue<std::net::IpAddr, TcpStream>> =
        Arc::new(crate::sched::FairQueue::new());
    // Greeter → per-holder fair queue (the priority seam #55 weights) →
    // settler. The empty key is the base lane: a non-enforce court or a
    // fresh registration has no authenticated holder yet.
    let settler_queue: Arc<crate::sched::Transit<LiveSession>> =
        Arc::new(crate::sched::Transit::tuned());
    // Reader → per-work-claim transit (#60): the full metric. Each work
    // record a reader pulls off its connection is offered here with its
    // support (cheap witness decode) and graded torsion (O(1) cached from
    // the Act::Declare'd complex); a settler pool drains it in metric order
    // — escrow × torsion quantum, diagonal support curvature — and verifies
    // it off the connection thread, handing the receipt bytes back.
    let work_transit: Arc<crate::sched::Transit<WorkJob>> =
        Arc::new(crate::sched::Transit::tuned());
    let torsion_cache: Arc<TorsionCache> = Arc::new(TorsionCache::default());
    // The convergence section (§6h): the court's convergent settlement state,
    // built forward as the settler pool deposits each settled claim (torsion
    // per grade converges, free axes accumulate). Held here so it is written
    // by real settlement, not a disconnected type; a court reports its growth.
    let section: Arc<Mutex<crate::section::Section>> =
        Arc::new(Mutex::new(crate::section::Section::new()));

    // Settler pool: drains the work-transit in full-metric order, verifies
    // against the shared book (the one settlement resource — the metric
    // decides who reaches it next), and replies with the receipt.
    for _ in 0..settler_count {
        let work_transit = Arc::clone(&work_transit);
        let ledger = Arc::clone(ledger);
        let book = Arc::clone(book);
        let section = Arc::clone(&section);
        let layout = layout.clone();
        let rules = rules.clone();
        std::thread::spawn(move || {
            while let Some(claim) = work_transit
                .take_blocking(|h| ledger.lock().map(|g| g.escrow_of(h)).unwrap_or(0))
            {
                let crate::sched::Claim { payload: job, support, .. } = claim;
                // Verify under a shared read lock (concurrent across the
                // settler pool), commit under the exclusive write lock, then
                // deposit the settled claim into the convergence section.
                let outcome = settle_work_parallel(&layout, &rules, &book, &section, &job.value);
                let _ = job.reply.send(outcome);
                work_transit.complete(&support);
            }
        });
    }

    for _ in 0..settler_count {
        let settler_queue = Arc::clone(&settler_queue);
        let work_transit = Arc::clone(&work_transit);
        let torsion_cache = Arc::clone(&torsion_cache);
        let layout = layout.clone();
        let ledger = Arc::clone(ledger);
        let rules = rules.clone();
        let book = Arc::clone(book);
        let witnesses = Arc::clone(witnesses);
        let on_session = Arc::clone(&on_session);
        std::thread::spawn(move || {
            // The reader pool drains the escrow-weighted session scheduler
            // (#55: escrow projected from the ledger at take-time, #57), then
            // reads each session's records and hands every work claim to the
            // work-transit for metric-ordered settlement off this thread.
            while let Some(claim) = settler_queue
                .take_blocking(|h| ledger.lock().map(|g| g.escrow_of(h)).unwrap_or(0))
            {
                let crate::sched::Claim { payload: live, support, .. } = claim;
                let LiveSession {
                    mut stream,
                    challenge_frame,
                    mut buffer,
                    survey,
                    ip,
                } = live;
                let holder = survey.holder.clone().unwrap_or_default();
                // #60: settle each work claim through the full metric. The
                // support is decoded from the claim's witness (no SNF); the
                // torsion is an O(1) cached lookup of the declared universe.
                // Offer to the work-transit, then block for the receipt —
                // the reader is idle while a settler verifies, so the metric
                // orders which claim reaches the book next.
                let settle = |value: &[u8]| -> SettleOutcome {
                    let tag = rules.market.as_ref().map_or(0, |m| m.query.domain_tag);
                    let support = crate::geometry::claim_support(value, tag).into_boxed_slice();
                    let torsion = torsion_cache.torsion_of(tag, &ledger);
                    let (tx, rx) = std::sync::mpsc::sync_channel(1);
                    work_transit.offer(crate::sched::Claim {
                        holder: holder.clone(),
                        support,
                        torsion: torsion.to_vec().into_boxed_slice(),
                        payload: WorkJob { value: value.to_vec(), reply: tx },
                    });
                    rx.recv().unwrap_or_else(|_| SettleOutcome::refused())
                };
                let result = drain_records(
                    survey,
                    &mut stream,
                    &mut buffer,
                    &layout,
                    &ledger,
                    &rules,
                    &book,
                    &witnesses,
                    &challenge_frame,
                    &settle,
                );
                // The wall was charged at accept and held across the
                // handshake and the queue wait; release it however the
                // session ends.
                release_wall(&rules.connections, ip);
                settler_queue.complete(&support);
                match result {
                    Ok(report) => on_session(&report),
                    Err(e) => println!("plumbd: session failed: {e:?}"),
                }
            }
        });
    }

    for _ in 0..greeter_count {
        let greeter_queue = Arc::clone(&greeter_queue);
        let settler_queue = Arc::clone(&settler_queue);
        let layout = layout.clone();
        let ledger = Arc::clone(ledger);
        let rules = rules.clone();
        let book = Arc::clone(book);
        let witnesses = Arc::clone(witnesses);
        let on_session = Arc::clone(&on_session);
        std::thread::spawn(move || {
            while let Some((ip, stream)) = greeter_queue.take_blocking() {
                match greet_session(stream, ip, &layout, &ledger, &rules, &book, &witnesses) {
                    GreetOutcome::Live(live) => {
                        // Key by the authenticated holder (empty = base
                        // lane); the settler queue's per-key fairness is
                        // what #55 upgrades to escrow-weighted priority.
                        let holder = live.survey.holder.clone().unwrap_or_default();
                        // Enqueue as a Claim into the transit scheduler.
                        // Support/torsion are empty at this seam — a
                        // session's work geometry is not known until its
                        // records arrive in drain_records — so the metric
                        // here is escrow only; per-work-claim curvature
                        // attaches inside the record loop.
                        settler_queue.offer(crate::sched::Claim {
                            holder,
                            support: Box::default(),
                            torsion: Box::default(),
                            payload: *live,
                        });
                    }
                    GreetOutcome::Done(report) => {
                        release_wall(&rules.connections, ip);
                        on_session(&report);
                    }
                    GreetOutcome::Errored(e) => {
                        release_wall(&rules.connections, ip);
                        println!("plumbd: session failed: {e:?}");
                    }
                    GreetOutcome::Failed => release_wall(&rules.connections, ip),
                }
            }
        });
    }

    loop {
        match listener.accept() {
            Ok((stream, peer)) => {
                // P3 — the wall: checked and charged before the session
                // is queued. An over-quota connection is dropped here, at
                // zero cost past the accept itself; `max_total_connections
                // = 0` / `max_connections_per_ip = 0` opt a wall out.
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
                // Past the anti-Sybil wall, the scheduler decides: admit
                // to the greeter pool if the per-IP queue has room, else
                // tell the peer to wait with a named busy frame — never
                // dropped in silence.
                match governor.admit(&greeter_queue.load(&ip)) {
                    crate::sched::Admission::Admit => greeter_queue.offer(ip, stream),
                    crate::sched::Admission::Busy { retry_after_secs } => {
                        let mut wire = Vec::new();
                        if isthmus::frame::put_frame(
                            layout,
                            crate::sched::BUSY_TAG,
                            &retry_after_secs.to_le_bytes(),
                            &mut wire,
                        )
                        .is_ok()
                        {
                            let mut stream = stream;
                            let _ = stream.write_all(&wire);
                        }
                        release_wall(&rules.connections, ip);
                    }
                    crate::sched::Admission::Reject => release_wall(&rules.connections, ip),
                }
            }
            Err(e) => return e,
        }
    }
}

/// The greeter's job: TLS-wrap (P4), run the handshake under the P3
/// deadline, then read until the session goes live so the holder is
/// known before it is enqueued for a settler. A session that ends during
/// the handshake (refused registration, early close, or error) is
/// finalized here and never enqueued.
fn greet_session(
    stream: TcpStream,
    ip: std::net::IpAddr,
    layout: &Layout,
    ledger: &Arc<Mutex<Ledger>>,
    rules: &SessionRules,
    book: &Arc<RwLock<RewardBook>>,
    witnesses: &WitnessLog,
) -> GreetOutcome {
    // P3's deadline is set on the raw socket, before P4 might wrap it —
    // it has to bound a stalled TLS handshake too. The clone shares the
    // same fd; court_handshake lifts the deadline once the declaration
    // arrives.
    if let Some(deadline) = rules.handshake_deadline {
        let _ = stream.set_read_timeout(Some(deadline));
    }
    let handshake_socket = stream.try_clone().ok();
    // P4 — every inbound tag on this listener crosses the same greeter,
    // so wrapping here covers all of them with nothing role-specific.
    let wire = match &rules.tls {
        Some(server_tls) => rustls::ServerConnection::new(Arc::clone(server_tls))
            .map(|conn| Box::new(rustls::StreamOwned::new(conn, stream)) as Box<dyn ReadWrite>)
            .map_err(|e| format!("{e:?}")),
        None => Ok(Box::new(stream) as Box<dyn ReadWrite>),
    };
    let wire = match wire {
        Ok(wire) => wire,
        Err(e) => {
            println!("plumbd: TLS setup failed: {e}");
            return GreetOutcome::Failed;
        }
    };
    let state =
        match court_handshake(wire, handshake_socket, layout, ledger, rules, book, witnesses) {
            Ok(state) => state,
            Err(e) => return GreetOutcome::Errored(e),
        };
    let SessionState {
        mut stream,
        challenge_frame,
        mut buffer,
        ..
    } = state;
    let bound = rules.bound;
    let mut survey = Survey::new(!rules.enforce);
    // Pre-go-live pump never credits (work before go-live is refused), so
    // this inline settle is a formality; the reader's metric-scheduled
    // settle handles all real crediting after go-live.
    let settle = |value: &[u8]| match book.write() {
        Ok(mut guard) => settle_work(layout, rules, &mut guard, value),
        Err(_) => SettleOutcome::refused(),
    };
    // Pump only until go-live: a non-enforce court is already live and
    // reads zero frames here (holder stays None → base lane); an
    // enforcing court reads through the challenge-answering attestation,
    // which sets the holder. Pre-live `pending` is always empty, so an
    // early close leaves a final report with nothing to reconcile.
    while !survey.session_live {
        match read_record(&mut *stream, &mut buffer, layout, bound) {
            Ok(Some((tag, frame))) => match survey.pump_one(
                tag,
                frame,
                &mut stream,
                layout,
                ledger,
                rules,
                book,
                witnesses,
                &challenge_frame,
                &settle,
            ) {
                Ok(Flow::Continue) => {}
                Ok(Flow::Stop) => return GreetOutcome::Done(survey.report),
                Err(e) => return GreetOutcome::Errored(e),
            },
            Ok(None) => return GreetOutcome::Done(survey.report),
            Err(e) => return GreetOutcome::Errored(e),
        }
    }
    GreetOutcome::Live(Box::new(LiveSession {
        stream,
        challenge_frame,
        buffer,
        survey,
        ip,
    }))
}

/// Return one connection's slot to the admission wall (P3), by IP.
fn release_wall(connections: &Arc<Mutex<ConnectionCounts>>, ip: std::net::IpAddr) {
    if let Ok(mut counts) = connections.lock() {
        counts.total = counts.total.saturating_sub(1);
        if let Some(c) = counts.per_ip.get_mut(&ip) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                counts.per_ip.remove(&ip);
            }
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

/// [`dial`], setting a read timeout on the raw socket first — a
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
/// the actual close. Skipping this leaves data unread in this side's
/// own receive buffer at close, which makes the OS send RST instead
/// of FIN; a RST at a record boundary elsewhere reads as a graceful
/// departure, and the cost lands on the court, which may not have
/// finished reading the records this side just sent. Every function
/// that writes to a court and then walks away needs this — a market
/// being posted is a fact about the court, not about which client
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
/// this session's own challenge (registering, not admitting), and —
/// once the court's ack shows the bind landed — send one signed claim
/// on the same connection through the now-ordinary admission path.
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

    // Bound now — the same connection's challenge was already spent
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

    // Face upstream: declare, hear the court. this leg is a plain
    // court-facing dial like any other — it gets TLS the same way.
    let mut court = dial(upstream, upstream_tls_fingerprint)?;
    send_hello(&mut court, layout, hello_tag(ledger, holder), &ours)?;
    let mut court_buf = Vec::new();
    let _court_hello = read_hello(&mut court, &mut court_buf, layout, &ours, bound)?;

    // IS-2/2: the court's session challenge follows its declaration.
    // Relay it to the client verbatim — the client's answer signs the
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

/// Answer a court's posted question natively: attach, answer the
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
    // The split between receipt and attestation is found by parsing
    // the receipt forward, not by subtracting a fixed attestation
    // width — so a variable-length (scheme ≥ 0x02) attestation rides
    // behind the receipt with no wire break.
    let (parsed, consumed) = receipt::Receipt::decode_prefix(value)
        .map_err(|_| NodeBroken::NoDeclaration)?;
    let attestation = sig::Attestation::decode(value.get(consumed..).unwrap_or(&[]))
        .map_err(|_| NodeBroken::NoDeclaration)?;
    Ok((
        query,
        receipt::SignedReceipt {
            receipt: parsed,
            attestation,
        },
    ))
}
