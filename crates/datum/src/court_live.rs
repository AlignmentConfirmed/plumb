//! Wave 4 **V16 / D-L3c** — court live multi-host federation.
//!
//! Spec: monorepo `docs/WAVE4-FEDERATION.md`.
//! Extends D-L3b file carriers with **TCP exchange of XDCT snapshots**.
//!
//! ```text
//! host A  encode(XDCT) ──TCP──► host B  decode + merge_acts_from
//!         work_id once on both; replay refuses both sides
//! ```
//!
//! **Court authority only.** No edge cover invent. No mesh Address.
//! Transport is opaque XDCT bytes (already durable format) with a
//! length-prefix carrier envelope — not mesh wire frames.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

use crate::court_store::{self, StoreBroken};
use crate::reward::RewardBook;

/// Why live court federation refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CourtLiveRefused {
    /// Durable store decode failed.
    Store(StoreBroken),
    /// Transport IO.
    Io,
    /// Empty snapshot.
    Empty,
    /// Length prefix / size absurd.
    Malformed,
}

impl From<StoreBroken> for CourtLiveRefused {
    fn from(e: StoreBroken) -> Self {
        CourtLiveRefused::Store(e)
    }
}

impl From<io::Error> for CourtLiveRefused {
    fn from(_: io::Error) -> Self {
        CourtLiveRefused::Io
    }
}

/// Export court acts as durable XDCT bytes (same as file store).
#[must_use]
pub fn export_snapshot(book: &RewardBook) -> Vec<u8> {
    court_store::encode(book)
}

/// Import remote XDCT and merge into local book. Returns acts added.
pub fn import_merge(local: &mut RewardBook, bytes: &[u8]) -> Result<usize, CourtLiveRefused> {
    if bytes.is_empty() {
        return Err(CourtLiveRefused::Empty);
    }
    let remote = court_store::decode(bytes)?;
    Ok(local.merge_acts_from(&remote))
}

/// Send one length-prefixed XDCT snapshot on a stream.
pub fn send_snapshot(stream: &mut TcpStream, book: &RewardBook) -> Result<(), CourtLiveRefused> {
    let bytes = export_snapshot(book);
    if bytes.is_empty() {
        return Err(CourtLiveRefused::Empty);
    }
    if bytes.len() > u32::MAX as usize {
        return Err(CourtLiveRefused::Malformed);
    }
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

/// Receive one length-prefixed XDCT snapshot.
pub fn recv_snapshot(stream: &mut TcpStream) -> Result<Vec<u8>, CourtLiveRefused> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > 64 * 1024 * 1024 {
        return Err(CourtLiveRefused::Malformed);
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

/// Push local book to peer address and return acts they would merge
/// (caller on peer side runs import_merge). Helper for one-shot send.
pub fn push_to(addr: impl ToSocketAddrs, book: &RewardBook) -> Result<(), CourtLiveRefused> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    send_snapshot(&mut stream, book)
}

/// Accept one connection on listener, read snapshot, merge into local.
pub fn accept_merge(
    listener: &TcpListener,
    local: &mut RewardBook,
) -> Result<usize, CourtLiveRefused> {
    listener.set_nonblocking(false)?;
    let (mut stream, _) = listener.accept()?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let bytes = recv_snapshot(&mut stream)?;
    import_merge(local, &bytes)
}

/// **Pin helper:** two books federate over loopback TCP.
///
/// 1. A credits → push XDCT to B  
/// 2. B merges → credits own work → push to A  
/// 3. A merges → both hold both work_ids; further merge adds 0
pub fn federate_loopback_ab(
    book_a: &mut RewardBook,
    book_b: &mut RewardBook,
) -> Result<(usize, usize), CourtLiveRefused> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    // A → B
    let snap_a = export_snapshot(book_a);
    let handle = {
        let listener = listener.try_clone()?;
        thread::spawn(move || -> Result<Vec<u8>, CourtLiveRefused> {
            listener.set_nonblocking(false)?;
            let (mut stream, _) = listener.accept().map_err(|_| CourtLiveRefused::Io)?;
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            recv_snapshot(&mut stream)
        })
    };
    thread::sleep(Duration::from_millis(15));
    let mut client = TcpStream::connect(addr)?;
    client.set_write_timeout(Some(Duration::from_secs(5)))?;
    // send snap_a
    client.write_all(&(snap_a.len() as u32).to_le_bytes())?;
    client.write_all(&snap_a)?;
    client.flush()?;
    let bytes_for_b = handle.join().map_err(|_| CourtLiveRefused::Io)??;
    let added_b = import_merge(book_b, &bytes_for_b)?;

    // B → A (second connection)
    let listener2 = TcpListener::bind("127.0.0.1:0")?;
    let addr2 = listener2.local_addr()?;
    let snap_b = export_snapshot(book_b);
    let handle2 = {
        let listener = listener2.try_clone()?;
        thread::spawn(move || -> Result<Vec<u8>, CourtLiveRefused> {
            listener.set_nonblocking(false)?;
            let (mut stream, _) = listener.accept().map_err(|_| CourtLiveRefused::Io)?;
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            recv_snapshot(&mut stream)
        })
    };
    thread::sleep(Duration::from_millis(15));
    let mut client2 = TcpStream::connect(addr2)?;
    client2.set_write_timeout(Some(Duration::from_secs(5)))?;
    client2.write_all(&(snap_b.len() as u32).to_le_bytes())?;
    client2.write_all(&snap_b)?;
    client2.flush()?;
    let bytes_for_a = handle2.join().map_err(|_| CourtLiveRefused::Io)??;
    let added_a = import_merge(book_a, &bytes_for_a)?;

    Ok((added_b, added_a))
}
