//! # SDK — the kernel attach surface.
//!
//! What an outside kernel needs to join the proof economy, and nothing
//! it must not have. The SDK imports the substrate only: it builds
//! declarations, checks grants against chain state, and wraps portable
//! claim bodies in opaque highway envelopes. It **never verifies a
//! claim** — verification is the court's act, and a kernel that wants
//! to verify runs a court.
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
//! ## The three calls
//!
//! - [`attach`] — say who you are and what you speak. Revisions
//!   compare for **equality, never order**: two peers on different
//!   revisions disagree about what a frame means and neither is wrong.
//! - [`grant`] — authorization is a **ledger fact**, not an allowlist:
//!   a kernel is authorized exactly when the chain shows a live deed
//!   for its holder covering the tag it writes.
//! - [`submit`] — a portable claim body becomes an opaque envelope no
//!   carrier can read and any court can open.
//!
//! ## What is deliberately absent
//!
//! `survey`, `settle`, and `credit` are the court's surface (`datum`),
//! not the kernel's. Until transport exists they are in-process calls
//! on a court you run; the example `join.rs` shows both sides in one
//! process. When transport lands, this crate grows the client half of
//! those calls without gaining the right to answer them.

pub mod attach;
pub mod grant;
pub mod submit;
