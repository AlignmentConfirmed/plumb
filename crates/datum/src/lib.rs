//! # DATUM — THE AUTHORITY (highway court + measurement bench).
//!
//! ```text
//! independent nodes (producer | verifier | carrier)
//!         │
//!    ISTHMUS  superhighway   (issuer: frames, skip-unknown, session)
//!         │
//!    DATUM    this crate     (authority: deeds, rewards, measurements)
//!         ▲
//!    tollways: strand (xylarium) · netstratum mesh · future meshes
//!         │
//!    kernels: lith · chitin · …
//! ```
//!
//! **Superhighway, not a domain mesh.** `strand` is xylarium's local
//! tollway (kernel-specific instructions). `isthmus` is the up-level
//! substrate every mesh and kernel can link into. **datum** is not a
//! mesh node everyone must run: it holds the founding chain, prices
//! estates, credits multi-axial POW++ work, and measures that tollways
//! and kernels agree on the wire.
//!
//! **Issuer vs authority.** `isthmus` executes and holds nothing that
//! outlives a process. This crate holds the record at
//! [`ledger::FOUNDING`]; a deed is real when it is in that chain. Git
//! is the append-only guarantee.
//!
//! **Decentralized nodes.** Peers are independent. There is no
//! coordinator and no node identity registry — a peer is what it can
//! demonstrate (work, verification, carriage). Rewards credit
//! multi-axial closure work against deed-priced space ([`reward`]),
//! not a scalar fee and not a name.
//!
//! **Edge-free by construction.** This crate reaches nothing outside
//! its own workspace. Kernel-edge measurements (the crossing suites
//! against live kernels) live in the lab, which depends on this crate
//! the way any outsider would. Nothing depends on this crate.

// Edge-free: these measure nothing that needs a kernel, so they stand
// whatever is mid-edit anywhere.
pub mod admission;
pub mod block;
pub mod board;
pub mod bounty;
pub mod court_store;
/// Wave 4 V16 / D-L3c live multi-host court federation (TCP XDCT).
pub mod court_live;
pub mod court_service;
pub mod corpus;
pub mod domains;
pub mod escrow;
pub mod extent;
pub mod geometry;
pub mod hygiene;
pub mod ledger;
pub mod merge;
pub mod negotiation;
pub mod onramp;
pub mod plumbd;
pub mod sched;
// K1: Query/Conjecture/Receipt now live in the leaf-importable sdk
// crate (a kernel may need them with zero datum in its tree); the
// court re-exports them under their original names so every existing
// `datum::query::*` / `datum::receipt::*` path keeps resolving.
pub use sdk::query;
pub use sdk::receipt;
pub mod registration;
pub mod registry;
pub mod reward;
pub mod sample;
pub mod section;
pub mod settle;
pub mod tls;
pub mod witnessing;

