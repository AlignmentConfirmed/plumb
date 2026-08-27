//! # Isthmus — THE ISSUER (superhighway substrate)
//!
//! The up-level mesh wire: generalized frames for **all content, all
//! kernels, mesh-to-mesh connectivity**. Domain meshes (e.g. xylarium
//! `strand`, netstratum's chronicle mesh) are **tollways** — they carry
//! kernel-specific instructions and on-ramp into this highway.
//!
//! ```text
//! independent nodes ──► ISTHMUS (this crate) ──► nodes | tollways | kernels
//!                              ▲
//!                    datum is the authority (deeds, court)
//! ```
//!
//! A record rises through a kernel and optional local mesh, crosses
//! this substrate as opaque length-prefixed frames, and lands in
//! another mesh or a kernel directly. **An exit needs no mesh of its
//! own.** Nodes are decentralized: produce multi-axial work, verify by
//! re-derivation, or carry frames they do not own ([`node`]).
//!
//! ## Issuer, not authority
//!
//! This crate **executes**: frames, sessions, run-finding, act codecs,
//! opaque POW++ claim envelopes ([`work`]). It holds no state that
//! outlives a process and never answers who holds what.
//!
//! **datum is the authority.** The founding chain is stored `.tlv`
//! there; a deed is real when it is in that chain. Importing the issuer
//! must not fork the record — so the record does not live here.
//!
//! This crate **never verifies a proof payload** and never imports a
//! kernel or `assay`. Carriers length-skip unknown tags.
//!
//! **Dimensionality:** deed space is **n-axial polytopal** via
//! [`deed::Act::Open`] (not 2-D-capped; spheres/hyperspheres of estate
//! capacity). Multi-chain structure is [`sphere`] — frontiers and
//! vertical anchors on a sphere of chains.
//! Product law: datum the lab's `decide/linkage-estates.md`.
//!
//! ## What this crate is
//!
//! Everything in `IS-1` (the record, the exact rational, the refusals),
//! `IS-2` §7 (the session rule), `IS-3` (the registry), `IS-5` (the
//! declaration) and `IS-6` (the chain — [`deed::chain`], the acts as
//! stored bytes), and **nothing else**.
//!
//! `IS-6` is the odd one: the codec has been here since the founding
//! and the *document* did not exist until the vertical was added to it.
//! [`revisions`] is the list that is checked, and it now carries
//! `IS-6/1`.
//!
//! ## What this crate is not
//!
//! It is not a kernel and it names no kernel type.
//!
//! `IS-1` §7 specifies two frames — the relation, tag 1, and the
//! manifold, tag 5 — that cite `lith::Support` and `lith::Manifold`.
//! Those frames are **absent here on purpose**. A frame that names a
//! kernel type carries a dependency on that type, so implementing it
//! here would put a kernel dependency inside the crate whose entire
//! claim is that it has none.
//!
//! They belong to whoever holds the grant. This crate carries the
//! framing they travel in, and [`frame::Reader::frame`] hands you their
//! bytes without reading them.
//!
//! ## The minimum an integrator implements
//!
//! Four things, in this order.
//!
//! 1. **The record** — [`frame`]. `tag u8 ‖ LE32(length) ‖ value`.
//! 2. **Skip what you do not own** — [`frame::Reader::frame`] returns
//!    the tag and the value; a tag outside your grants is stepped over
//!    whole and forwarded. This is not an optimisation. It is the
//!    property that lets a mesh link to a mesh.
//! 3. **Four of the five verdicts** — [`Verdict`]. `accept`, `refuse`,
//!    `skip`, `wait`. The fifth, [`Verdict::Recognised`], is
//!    **optional**: it requires a court, and a reader without one
//!    lawfully degrades it to `skip` — forwarding instead of
//!    delivering, which loses economy and never correctness.
//! 4. **Tell *never* from *not yet*** — [`session::step`].
//!
//! A peer that implements only these four can connect. It will forward
//! what it cannot read, which is exactly what a linking mesh does.
//!
//! This list said "the four verdicts" while [`Verdict`] had five, from
//! `IS-1/3` until it was caught auditing `IS-6`'s claims. The count was
//! stale rather than the design: four really is the minimum, and
//! saying so is different from saying there are four.
//!
//! ## Checking yourself
//!
//! `tests/vectors.rs` asserts every byte string `IS-1` §9 publishes for
//! the frames this crate owns. `tests/refusals.rs` constructs one input
//! per row of the §4 refusal table, **and one that must be accepted for
//! each**, because a reader that refuses everything passes a refusal
//! table.

#![deny(missing_docs)]

pub mod deed;
pub mod frame;
pub mod hello;
pub mod sphere;
pub mod layout;
pub mod node;
pub mod ratio;
pub mod session;
pub mod work;

// There is no `registry` module. It held a `Band` enum of ten variants
// -- ten possible holders, EVER, an eleventh needing a new variant and a
// recompile -- over a `BANDS: [_; 12]` table fixed at compile time.
//
// That is the same defect as `grants_available() -> 6`, one level up:
// the count of grants was fixed, and so was the count of parties who
// could ever hold one. A holder is a `String` on a `Deed` now, and the
// table of who holds what is a `Ledger` built at runtime from whatever
// the edge actually knows.
//
// The specific assignments that table carried are one edge's facts, not
// the protocol's. `datum` builds them by reading both ancestors'
// registries, which is where a fact about the ancestors belongs.

pub use deed::{Binding, Deed, Ledger, Standing};
pub use frame::{Malformed, Reader};
pub use ratio::Exact;
pub use session::Step;

/// The revision strings this crate implements.
///
/// Sent in a [`hello::Hello`] and **compared for equality, never
/// ordered**. There is no newer-than: ordering would let a peer decide
/// it is ahead and act on the difference, which is authority this
/// substrate does not have. Two peers on different revisions disagree
/// about what a frame means and neither is wrong.
/// The revisions this build implements.
///
/// **A function, not a `[&str; 4]`.** The fixed arity meant adding a
/// document required editing the literal's length — a small thing, and
/// exactly the shape of every other capacity here: a number that has to
/// be maintained to let something grow.
///
/// `IS-1/2` and not `IS-1/1`: this crate refuses negative zero, a
/// refusal **neither ancestor produces**. It was found by writing this
/// crate — see [`frame::Malformed::NegativeZero`] — so `strand` still
/// implements `IS-1/1` and accepts a byte string this rejects. That
/// disagreement is visible in the declaration rather than hidden, which
/// is what revision strings are for.
///
/// Compared for equality and **never ordered**: ordering would let a
/// peer decide it is ahead and act on the difference, which is authority
/// this substrate does not have.
pub fn revisions() -> Vec<String> {
    // IS-1/3: §10 gains Recognised, the fifth verdict.
    // IS-1/4: §7.2's orbs carry extents, not scalars — capacity and
    // energy went multi-component in the kernel, because a scalar
    // capacity is one number met or refused and poles do not exchange.
    // The frame changed, so the revision does.
    // IS-5/2: the declaration gains the uplink block — a peer says
    // which chain it is and what it has seen. A chain's name is not in
    // its stored bytes, so without this an `Act::Anchor` naming a peer
    // has nothing to bind the name to, and the substrate is
    // downstream-only. Emitted ONLY by a chain that took a name, so a
    // peer that has not opted into being addressable is byte-identical
    // to IS-5/1 — the incompatibility is opt-in, by the side choosing
    // it, and declared here either way.
    // IS-6/1: the chain — this crate's `deed::chain` codec, which was
    // specified in no document until now. An outside implementation
    // could read IS-3's grant table and could not read or produce the
    // record that table is rendered from, so it could verify nothing
    // and append nothing.
    // IS-6/2: §8.1 replay. A repeated `Act::Open` folds to nothing —
    // `IS-2` §6.1's rule, applied where it had never been applied. A
    // BEHAVIOUR change: an IS-6/1 reader opens a second axis where this
    // one opens none, so the two disagree about the shape of the space
    // and must not be treated as compatible.
    // IS-3/2: §5.6 — a tag inside a grant is DERIVED from the record
    // kind and the holder's own deed, never declared as a constant.
    // A constant encodes an assumption about who else exists, and a
    // substrate cannot know that. Deed::tag_for is the derivation.
    // IS-6/3: tag 9 `Sublet` — estates within estates. Additive on the
    // wire; an older reader refuses tag 9 rather than misfolding. The
    // theorems are RESTATED, not weakened: H2 (live deeds disjoint)
    // becomes H2′ (disjoint at the same depth, and inside the parent),
    // which reduces to H2 where nothing is sublet.
    // IS-6/4: tag 10 `Bind` — a holder's presenting key on the record
    // (S3 of the signature layer): key x grants x epoch window, as a
    // chain fact rather than an allowlist. Additive on the wire; an
    // older reader refuses tag 10 rather than misfolding. A holder
    // with no bind is legacy/unbound — visible, and refusable by
    // courts that demand keys.
    // IS-6/5: tag 11 `Declare` — a domain definition published on the
    // record, bound to a tag by the resolver's read-time rule
    // (registration requires holding the grant; a definition lapses
    // with its deed). The bytes are opaque to the chain — a court's
    // evaluator interprets them, which is how a node learns a new
    // discipline from the chain alone, with no rebuild.
    // IS-2/2: §6.0 — the session challenge. One entropy record after
    // the court's declaration; under enforcement the first attestation
    // must answer it over exact frame bytes, or the session never goes
    // live. A replayed session's answer covers a dead token. A
    // BEHAVIOUR change at enforcing courts, so the revision moves.
    ["IS-1/4", "IS-2/2", "IS-3/2", "IS-5/2", "IS-6/5"]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// What a reader answers about a byte string.
///
/// `IS-1` §10. **The difference between [`Skip`](Verdict::Skip) and
/// [`Wait`](Verdict::Wait) is where most readers go wrong**: one is a
/// record you will never own, the other is a record that has not
/// finished arriving. Conflating them either drops data or stalls
/// forever.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Decodes to a whole record.
    Accept,
    /// Malformed. A reader that accepts this has a defect.
    Refuse(Malformed),
    /// Not a record this reader owns. Step over it whole and continue.
    Skip {
        /// The tag that was not owned.
        tag: layout::Tag,
        /// Total bytes of the record, header included.
        whole: usize,
    },
    /// **Between skip and accept**: the record's shape is confirmed —
    /// length-framed whole, its tag deeded on the court — and its
    /// payload is entirely opaque.
    ///
    /// This is what lets two differing kernels share a mesh without
    /// forcing mutual comprehension: the mesh delivers the unfractured
    /// record to its kernel, which applies the deed's law on receipt
    /// or discards the unreadable geometry — no payload branch was
    /// ever evaluated at the carrier seam.
    ///
    /// **Requires a court.** A reader without one lawfully degrades
    /// `Recognised` to `Skip` — forwarding instead of delivering,
    /// which loses economy and never correctness. A peer that speaks
    /// less is limited, never refused.
    Recognised {
        /// The tag, deeded to somebody on the court.
        tag: layout::Tag,
        /// Total bytes of the record, header included — the whole that
        /// is delivered without fracture.
        whole: usize,
    },
    /// A record has begun and not finished. Hold — neither accept nor
    /// refuse.
    Wait,
}

/// Read the head of a buffer the way a conforming reader does, given
/// which tags this reader owns.
///
/// This is the whole of §2 and §10 in one function, and it is the
/// shortest correct integration: hand it your buffer and your grants.
///
/// `owns` is asked about the tag, not consulted about the value. A
/// reader that owns a tag still has to decode it; this only decides
/// whether it is yours.
pub fn read(
    layout: &layout::Layout,
    bytes: &[u8],
    bound: usize,
    owns: impl Fn(layout::Tag) -> bool,
) -> Verdict {
    verdict(layout, bytes, bound, owns, None)
}

/// [`read`], with the court consulted: the five-verdict seam.
///
/// A tag this reader owns **accepts**; a tag deeded to somebody on the
/// court is **recognised** — shape confirmed, payload opaque, the
/// whole record delivered unfractured to the kernel behind the seam;
/// a tag nobody holds **skips** and is forwarded. No payload byte is
/// evaluated for any of the three — the only thing consulted beyond
/// the header is the court's fold.
///
/// Without a court (`None`) recognition degrades to skip, lawfully:
/// forwarding instead of delivering loses economy, never correctness.
pub fn verdict(
    layout: &layout::Layout,
    bytes: &[u8],
    bound: usize,
    owns: impl Fn(layout::Tag) -> bool,
    court: Option<&deed::Ledger>,
) -> Verdict {
    match session::step(layout, bytes, bound) {
        Step::Wait => Verdict::Wait,
        Step::Refuse(why) => Verdict::Refuse(Malformed::from(why)),
        Step::Take(whole) => match layout.take_tag(bytes) {
            // Unreachable in practice: `step` returned `Take`, so the
            // header is present. Answered rather than asserted, because
            // this crate does not panic on any input.
            None => Verdict::Wait,
            Some(tag) => {
                if owns(tag) {
                    Verdict::Accept
                } else if court.is_some_and(|c| c.holder_of(tag).is_some()) {
                    Verdict::Recognised { tag, whole }
                } else {
                    Verdict::Skip { tag, whole }
                }
            }
        },
    }
}
