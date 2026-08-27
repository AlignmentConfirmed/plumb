//! N3 — the durable court: snapshots that survive a kill, federation
//! that survives a peer.
//!
//! ```text
//!            ┌── snapshot thread ──► XDCT on disk (atomic rename)
//! court book ┼── push threads ─────► peers, one loop each, backoff
//!            └── accept thread ◄──── peers' snapshots, merged
//! ```
//!
//! Everything here composes what already exists: `court_store` is the
//! durable format, `court_live` is the wire, `RewardBook::merge_acts_from`
//! is the replay-refusing merge. This module adds only the *service*
//! shape — threads, timing, backoff, and the atomic write — because a
//! court that loses its book to a power cut is not an authority.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::court_live;
use crate::court_store;
use crate::reward::RewardBook;

/// How the durable court runs. All parts optional: a court with no
/// snapshot path is in-memory, a court with no peers is solitary —
/// both lawful, neither durable nor federated.
#[derive(Debug, Clone, Default)]
pub struct ServiceConfig {
    /// Where the XDCT snapshot lives. Written atomically.
    pub snapshot: Option<PathBuf>,
    /// Seconds between snapshots.
    pub snapshot_secs: u64,
    /// Where to accept peers' snapshots.
    pub fed_listen: Option<String>,
    /// Peers to push our snapshot to, one loop each.
    pub fed_peers: Vec<String>,
    /// Seconds between pushes to each peer.
    pub fed_secs: u64,
}

/// Load the book a snapshot recorded, or a fresh one if none exists.
///
/// A **corrupt** snapshot refuses rather than starting empty: a court
/// that silently forgot its acts would re-credit replayed work, which
/// is the exact failure the store exists to prevent.
pub fn load_book(path: &Path) -> Result<RewardBook, court_store::StoreBroken> {
    match std::fs::read(path) {
        Ok(bytes) => court_store::decode(&bytes),
        Err(_) => Ok(RewardBook::new()),
    }
}

/// Write the snapshot atomically: temp file, then rename. A kill
/// mid-write leaves the old snapshot, never half of a new one.
pub fn snapshot_atomic(path: &Path, book: &RewardBook) -> std::io::Result<()> {
    let bytes = court_store::encode(book);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)
}

/// A running court service. Dropping the handle does not stop the
/// threads; call [`ServiceHandle::stop`] for an orderly halt with a
/// final snapshot.
pub struct ServiceHandle {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
    snapshot: Option<PathBuf>,
    book: Arc<Mutex<RewardBook>>,
}

impl ServiceHandle {
    /// Halt every loop, join the threads, and write a final snapshot.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
        if let (Some(path), Ok(guard)) = (&self.snapshot, self.book.lock()) {
            let _ = snapshot_atomic(path, &guard);
        }
    }
}

/// Start the durable court around a shared book.
///
/// Restores from the snapshot first (refusing a corrupt one), then
/// spawns the loops the config asks for. Returns the restored act
/// count with the handle so a caller can log the resume.
pub fn start(
    config: &ServiceConfig,
    book: &Arc<Mutex<RewardBook>>,
) -> Result<(ServiceHandle, usize), court_store::StoreBroken> {
    let mut restored = 0usize;
    if let Some(path) = &config.snapshot {
        let loaded = load_book(path)?;
        restored = loaded.act_len();
        if restored > 0 {
            if let Ok(mut guard) = book.lock() {
                guard.merge_acts_from(&loaded);
            }
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();

    // Snapshot loop.
    if let Some(path) = config.snapshot.clone() {
        let every = Duration::from_secs(config.snapshot_secs.max(1));
        let book = Arc::clone(book);
        let stop_flag = Arc::clone(&stop);
        threads.push(std::thread::spawn(move || {
            while !stop_flag.load(Ordering::SeqCst) {
                if let Ok(guard) = book.lock() {
                    let _ = snapshot_atomic(&path, &guard);
                }
                let mut slept = Duration::ZERO;
                while slept < every && !stop_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    slept += Duration::from_millis(50);
                }
            }
        }));
    }

    // Federation accept loop: peers push snapshots; we merge. The
    // merge refuses replay by work identity, so a peer pushing the
    // same snapshot forever adds nothing forever.
    if let Some(listen) = config.fed_listen.clone() {
        let book = Arc::clone(book);
        let stop_flag = Arc::clone(&stop);
        threads.push(std::thread::spawn(move || {
            let Ok(listener) = std::net::TcpListener::bind(&listen) else {
                return;
            };
            let _ = listener.set_nonblocking(true);
            while !stop_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let _ = stream.set_nonblocking(false);
                        if let Ok(bytes) = court_live::recv_snapshot(&mut stream) {
                            if let Ok(mut guard) = book.lock() {
                                let _ = court_live::import_merge(&mut guard, &bytes);
                            }
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(50)),
                }
            }
        }));
    }

    // One push loop per peer, with backoff: an absent peer costs
    // patience, never correctness — the next push carries everything.
    for peer in config.fed_peers.clone() {
        let every = Duration::from_secs(config.fed_secs.max(1));
        let book = Arc::clone(book);
        let stop_flag = Arc::clone(&stop);
        threads.push(std::thread::spawn(move || {
            let mut backoff = Duration::from_millis(200);
            while !stop_flag.load(Ordering::SeqCst) {
                let sent = {
                    match book.lock() {
                        Ok(guard) => court_live::push_to(peer.as_str(), &guard).is_ok(),
                        Err(_) => false,
                    }
                };
                let wait = if sent {
                    backoff = Duration::from_millis(200);
                    every
                } else {
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                    backoff
                };
                let mut slept = Duration::ZERO;
                while slept < wait && !stop_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    slept += Duration::from_millis(50);
                }
            }
        }));
    }

    Ok((
        ServiceHandle {
            stop,
            threads,
            snapshot: config.snapshot.clone(),
            book: Arc::clone(book),
        },
        restored,
    ))
}
