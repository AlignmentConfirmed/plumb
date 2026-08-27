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
//! **What this is not yet:** unsigned (S4 lands attestation checking
//! here), unfresh (IS-2 §6 is N2), and unfederated (N3 wires
//! `court_live` peering into the serve loop).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};

use isthmus::deed::Ledger;
use isthmus::hello::Hello;
use isthmus::layout::{Layout, Tag};
use isthmus::session::{self, Step};

use crate::reward::RewardBook;

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
    holder: &str,
    book: &Arc<Mutex<RewardBook>>,
    bound: usize,
) -> Result<SessionReport, NodeBroken> {
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    let mut buffer = Vec::new();
    let _theirs = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    send_hello(&mut stream, layout, hello_tag(ledger, holder), &ours)?;

    let mut report = SessionReport::default();
    while let Some((tag, frame)) = read_record(&mut stream, &mut buffer, layout, bound)? {
        if isthmus::work::is_work_tag(tag) && tag != isthmus::work::RECEIPT_TAG {
            let value = frame.get(layout.header()..).unwrap_or(&[]);
            let mut guard = book.lock().map_err(|_| NodeBroken::CourtUnreachable)?;
            match guard.credit_claim(value) {
                Ok(_) => report.credited += 1,
                Err(_) => report.refused += 1,
            }
        } else {
            report.skipped += 1;
        }
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
    holder: &str,
    book: &Arc<Mutex<RewardBook>>,
    bound: usize,
    on_session: impl Fn(&SessionReport) + Send + Sync + 'static,
) -> std::io::Error {
    let on_session = Arc::new(on_session);
    loop {
        match listener.accept() {
            Ok((stream, _peer)) => {
                let layout = layout.clone();
                let ledger = ledger.clone();
                let holder = holder.to_owned();
                let book = Arc::clone(book);
                let on_session = Arc::clone(&on_session);
                std::thread::spawn(move || {
                    if let Ok(report) =
                        court_session(stream, &layout, &ledger, &holder, &book, bound)
                    {
                        on_session(&report);
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
    let mut stream = TcpStream::connect(addr)?;
    let ours = Hello::of(ledger, holder, u32::try_from(bound).unwrap_or(u32::MAX));
    send_hello(&mut stream, layout, hello_tag(ledger, holder), &ours)?;
    let mut buffer = Vec::new();
    let _court = read_hello(&mut stream, &mut buffer, layout, &ours, bound)?;
    let mut sent = 0usize;
    for envelope in envelopes {
        stream.write_all(envelope)?;
        sent += 1;
    }
    Ok(sent)
}
