//! # SDK — the kernel attach surface.
//!
//! What an outside kernel needs to join the proof economy, and nothing
//! it must not have. The SDK imports the LEAVES only (K1, ratified
//! 2026-08-28: isthmus, assay, sig — laws, not the court): it builds
//! declarations, checks grants against chain state, carries the
//! portable market vocabulary a kernel derives against
//! ([`query`]/[`receipt`]), and wraps portable claim bodies in opaque
//! highway envelopes. It **never verifies a CLAIM** — that a body
//! closes a boundary is the court's act, and a kernel that wants to
//! verify claims runs a court. Checking a receipt's signature and
//! chain-binding ([`receipt::verify`]) is a different, narrower thing
//! — the same ledger-fact class of check [`grant`] already does — not
//! claim verification.
//!
//! ```text
//! kernel ──[ attach ]──▶ declared on the edge (IS-5)
//! kernel ──[ grant  ]──▶ holds an unretired range on the chain (IS-3)
//! kernel ──[ submit ]──▶ claim in an envelope, tags 80–82 (opaque)
//!                          │
//!                          ▼  carriers forward unread
//!                        court: survey · settle · credit
//! ```
//!
//! ## The calls
//!
//! - [`attach`] — say who you are and what you speak. Revisions
//!   compare for **equality, never order**: two peers on different
//!   revisions disagree about what a frame means and neither is wrong.
//! - [`grant`] — authorization is a **ledger fact**, not an allowlist:
//!   a kernel is authorized exactly when the chain shows a live deed
//!   for its holder covering the tag it writes.
//! - [`submit`] — a portable claim body becomes an opaque envelope no
//!   carrier can read and any court can open.
//! - [`query`] — X1: the demand-posed problem (and SQ4's conjecture)
//!   a kernel derives against.
//! - [`receipt`] — X2: the court's signed settlement statement, and
//!   the chain-alone check a facilitator (or a kernel) runs on it.
//! - [`derivation`] — K2: given a conjecture's universe and target
//!   boundary alone, walk the complex's own licensed 1-cells until a
//!   closing chain turns up or the derivation budget runs out. Never
//!   "search" in the generic sense — the complex already licenses
//!   which single steps exist; this only ever traverses those.
//!
//! ## What is deliberately absent
//!
//! `survey`, `settle`, and `credit` are the court's surface (`datum`),
//! not the kernel's. Until transport exists they are in-process calls
//! on a court you run; the example `join.rs` shows both sides in one
//! process. When transport lands, this crate grows the client half of
//! those calls without gaining the right to answer them.

pub mod attach;
pub mod derivation;
pub mod grant;
pub mod query;
pub mod receipt;
pub mod submit;
