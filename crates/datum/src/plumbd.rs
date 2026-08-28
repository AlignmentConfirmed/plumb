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
use crate::reward::RewardBook;
use crate::witnessing;

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
    stream: &mut TcpStream,
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
                let got = stream.read(&mut chunk)?;
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
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
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
#[derive(Debug, Clone)]
pub struct SessionRules {
    /// What the chain calls this court.
    pub holder: String,
    /// Largest record value this deployment accepts — measured.
    pub bound: usize,
    /// S4: hold every work envelope for its attestation and refuse
    /// forged / stale / unbound / orphaned presentations.
    pub enforce: bool,
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
}

/// Serve one inbound session as the court.
///
/// Declaration first, both ways; then every work record's value goes
/// to the shared book. The value is decoded by the book, never here —
/// the session layer stays payload-blind about everything but the tag.
pub fn court_session(
    mut stream: TcpStream,
    layout: &Layout,
    ledger: &Ledger,
    rules: &SessionRules,
    book: &Arc<Mutex<RewardBook>>,
    witnesses: &WitnessLog,
) -> Result<SessionReport, NodeBroken> {
    let (holder, bound, enforce) = (rules.holder.as_str(), rules.bound, rules.enforce);
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    let mut buffer = Vec::new();
    let _theirs = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    send_hello(&mut stream, layout, hello_tag(ledger, holder), &ours)?;

    // IS-2/2 — the session challenge: eight bytes of entropy, framed,
    // sent once per session right after the declaration. A replayed
    // session dies here: its recorded answer covers a token this court
    // never issued again.
    let token = sig::session_token().map_err(|_| NodeBroken::CannotFrame)?;
    let mut challenge_frame = Vec::new();
    isthmus::frame::put_frame(layout, hello_tag(ledger, holder), &token, &mut challenge_frame)
        .map_err(|_| NodeBroken::CannotFrame)?;
    stream.write_all(&challenge_frame)?;

    let mut report = SessionReport::default();
    let mut session_live = !enforce; // enforcement holds the session until the challenge is answered
    // Under enforcement a work envelope is held until its attestation
    // arrives (the next record); an envelope displaced or orphaned
    // without one is refused, not credited.
    let mut pending: Option<Vec<u8>> = None;
    while let Some((tag, frame)) = read_record(&mut stream, &mut buffer, layout, bound)? {
        if isthmus::work::is_work_tag(tag) && tag != isthmus::work::RECEIPT_TAG {
            if !enforce {
                let value = frame.get(layout.header()..).unwrap_or(&[]);
                let mut guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                match guard.credit_claim(value) {
                    Ok(_) => report.credited += 1,
                    Err(_) => report.refused += 1,
                }
            } else if !session_live {
                report.refused += 1; // work before the challenge was answered
            } else {
                if pending.take().is_some() {
                    report.refused += 1; // unattested envelope displaced
                }
                pending = Some(frame);
            }
        } else if enforce && tag == admission::ATTESTATION_TAG {
            if !session_live {
                // The first attestation must answer THIS session's
                // challenge. A stale answer — a replayed session —
                // refuses, and the session never goes live.
                let attestation = frame.get(layout.header()..).unwrap_or(&[]);
                let epoch = {
                    let guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
                    guard.open_epoch().unwrap_or(0)
                };
                match admission::admit(ledger, epoch, &challenge_frame, attestation) {
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
            let mut guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
            let epoch = guard.open_epoch().unwrap_or(0);
            match admission::admit(ledger, epoch, &envelope, attestation) {
                Ok(_holder) => {
                    let value = envelope.get(layout.header()..).unwrap_or(&[]);
                    match guard.credit_claim(value) {
                        Ok(_) => report.credited += 1,
                        Err(_) => report.refused += 1,
                    }
                }
                Err(_) => report.refused += 1,
            }
        } else if ledger.declaration_of(tag).is_some() {
            // UC4, LIVE: a tag with a registered definition on this
            // court's chain is judged against that definition — the
            // discipline the chain taught, applied on the wire. Under
            // enforcement the attestation rules still apply upstream;
            // here the claim must inhabit the registered universe and
            // close in it.
            let value = frame.get(layout.header()..).unwrap_or(&[]);
            match crate::domains::verify_registered(
                ledger,
                tag,
                value,
                assay::complex::DEFAULT_FUEL,
            ) {
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
    ledger: &Ledger,
    rules: &SessionRules,
    book: &Arc<Mutex<RewardBook>>,
    witnesses: &WitnessLog,
    on_session: impl Fn(&SessionReport) + Send + Sync + 'static,
) -> std::io::Error {
    let on_session = Arc::new(on_session);
    loop {
        match listener.accept() {
            Ok((stream, _peer)) => {
                let layout = layout.clone();
                let ledger = ledger.clone();
                let rules = rules.clone();
                let book = Arc::clone(book);
                let witnesses = Arc::clone(witnesses);
                let on_session = Arc::clone(&on_session);
                std::thread::spawn(move || {
                    match court_session(stream, &layout, &ledger, &rules, &book, &witnesses) {
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
    produce_inner(addr, layout, ledger, holder, bound, envelopes, None)
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
    produce_inner(addr, layout, ledger, holder, bound, envelopes, Some(key))
}

fn produce_inner(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    envelopes: &[Vec<u8>],
    key: Option<&sig::Keypair>,
) -> Result<usize, NodeBroken> {
    let mut stream = TcpStream::connect(addr)?;
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
    Ok(sent)
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
) -> Result<usize, NodeBroken> {
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));

    // Face the client: hear their declaration, answer with ours.
    let mut client_buf = Vec::new();
    let _client_hello = read_hello(&mut client, &mut client_buf, layout, &ours, bound)?;
    send_hello(&mut client, layout, hello_tag(ledger, holder), &ours)?;

    // Face upstream: declare, hear the court.
    let mut court = TcpStream::connect(upstream)?;
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
    Ok(forwarded)
}

/// Accept and carry forever, one thread per client session.
pub fn carry(
    listener: &TcpListener,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    upstream: String,
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
                    if let Ok(forwarded) =
                        carrier_session(stream, &layout, &ledger, &holder, bound, &upstream)
                    {
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
pub fn witness_to(
    addr: impl ToSocketAddrs,
    layout: &Layout,
    ledger: &Ledger,
    holder: &str,
    bound: usize,
    witnesses: &[isthmus::witness::Witness],
    key: Option<&sig::Keypair>,
) -> Result<usize, NodeBroken> {
    let mut stream = TcpStream::connect(addr)?;
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
    Ok(sent)
}
