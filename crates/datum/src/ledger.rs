//! THE AUTHORITY — the founding edge's chain, held here.
//!
//! ```text
//! .tlv -> kernel -> local mesh -> substrate -> local mesh | kernel
//! ```
//!
//! The roles, ruled: **datum is the authority, isthmus is the issuer.**
//!
//! `isthmus::deed::Ledger` is machinery — anyone can construct one and
//! issue to themselves, and before this file the actual founding edge's
//! acts were recorded nowhere durable: a test built them, asserted on
//! them, and threw them away. An authority whose record dies with a
//! process is not an authority.
//!
//! The chain lives at `ledger/founding.tlv`, in this repository, as the
//! same `.tlv` everything in the hierarchy grounds in. Git history is
//! the append-only guarantee: a rewritten act is a rewritten commit, and
//! a rewritten commit is visible.
//!
//! ## What enters the chain
//!
//! - **Observations** — both ancestors' registry claims, one
//!   [`Act::Encumber`] per claimed run, each carrying where it was read.
//!   Someone else's claim with provenance. Never our own documents.
//! - **Historical issuances** — the four standing deeds (`isthmus`,
//!   `assay`, `lith`, `chitin`), recorded with the ranges they already
//!   hold. Recording is transcription; the fold validates it.
//!
//! `IS-3` §5's table is a **rendering** of this chain. If they disagree,
//! the document is stale — that direction, never the reverse.

use isthmus::deed::{chain, Ledger};
use isthmus::layout::Layout;

/// Where the founding chain is stored, relative to this repository.
pub const FOUNDING: &str = "ledger/founding.tlv";

/// **What this chain calls itself**, so a stranger's
/// [`Act::Anchor`] can address it.
///
/// Before this constant the authority was *unaddressable*: a substrate
/// could attach to it, be deeded, and have frames recognised — all
/// downstream — and no other chain could record having observed it,
/// because an anchor names its target and there was no name to write.
/// Downstream worked and upstream did not, exactly as
/// [`isthmus::deed::Ledger::name`] describes the `None` case.
///
/// The name is **not in the stored bytes**. It is context the acts are
/// read in, like the layout, so a party reading `founding.tlv` off disk
/// learns the history and not who kept it. Which means the name has to
/// be *declared* over a session before an anchor to it means anything —
/// the wire half of the uplink, and not yet built.
pub const CHAIN: &str = "datum";

/// The founding acts are REBUILT in the lab, not here.
///
/// `found()` read both ancestors' live registries off this machine and
/// rebuilt the chain for byte-comparison against [`stored`]. That is a
/// measurement against working trees, so it lives in the lab alongside
/// the kernel edges. Here the stored chain IS the record.
/// The stored chain's bytes.
pub fn stored() -> std::io::Result<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read(root.join(FOUNDING))
}

/// The authority: the stored chain, decoded and replayed.
///
/// Refuses rather than substituting: if the chain is unreadable or
/// malformed there is **no authority to answer with**, and a fresh empty
/// ledger returned here would be an authority invented on the spot.
pub fn authority() -> Result<Ledger, String> {
    let bytes = stored().map_err(|e| format!("the chain is unreadable: {e}"))?;
    let acts =
        chain::decode(&bytes).map_err(|e| format!("the chain is malformed: {e}"))?;
    Ok(Ledger::replay(Layout::founding(), acts).under(CHAIN))
}
