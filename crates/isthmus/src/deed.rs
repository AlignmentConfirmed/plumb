//! Deeds — the record of a negotiated attachment.
//!
//! ## What was wrong before this file
//!
//! The tag space was a compile-time constant with a fixed grant width
//! and a fixed grant count:
//!
//! ```text
//! GRANT_FLOOR  192      GRANT_CEILING 239      GRANT_WIDTH 8
//! grants_available() -> 6.  Always. Forever.
//! ```
//!
//! **The substrate could accept exactly six attachments**, and a seventh
//! required recompiling this crate and every peer linking through it. A
//! capacity that cannot grow, built into the thing whose only purpose is
//! that other meshes attach to it.
//!
//! It is the dual of a gate that cannot fail. That one refuses nothing;
//! this one admits a seventh peer never.
//!
//! ## What a deed is
//!
//! A deed is **produced by an act of attachment**, not read from a
//! table. Two peers negotiate, and the deed is the record that they did.
//!
//! ```text
//! a const table              a deed
//! ─────────────────          ──────────────────────────────
//! compile time               issued during negotiation
//! global                     scoped to ONE attachment
//! capacity fixed at 6        capacity is however many attached
//! "who holds 200?"           "who holds 200 ON THIS EDGE?"
//! ```
//!
//! ## Why the space is not actually scarce
//!
//! One byte is 256 values **per edge**, not 256 values in the universe.
//! Two peers that have never met do not need to agree about tag 200
//! globally; they need to agree about it across the edge they share, and
//! the deed is that agreement.
//!
//! This is the same split the rest of the substrate already runs on:
//!
//! ```text
//! the MEANING of a frame     gauge-invariant   S, T
//! the NUMBER carrying it     frame-dependent   G
//! ```
//!
//! A tag is a coordinate. A deed records which coordinate system one
//! edge chose. Treating tag identity as global was treating `G` as if it
//! were `S` — depending on a frame, which is the defect this substrate
//! keeps striking out. `Address::of` hashes the canonical representative
//! and not the frame, for the same reason.
//!
//! ## Forwarding is a change of coordinates
//!
//! A mesh-of-meshes carries a frame from edge A to edge B. If tag 200
//! means different things on the two edges, forwarding the number is a
//! mistranslation — which is the objection that pushed the first draft
//! into a global table.
//!
//! The answer is that forwarding crosses a deed boundary, so it **is** a
//! change of coordinates: carry the meaning, re-number under the
//! destination deed. See [`Ledger::translate`].
//!
//! ## The ledger is the authority, and it appends
//!
//! [`Ledger`] is a **sequence of acts**, and every reading of it is a
//! fold over that sequence. Nothing is edited in place and nothing is
//! erased: [`Act::Retire`] is an entry, not a deletion.
//!
//! An earlier version kept a mutable `standing: Vec<Standing>` and wrote
//! into it. That is a cache with no history — two ledgers holding the
//! same tags could not be told apart, and there was nothing to audit.
//!
//! **A document is never an input here.** `IS-3` §5's table is a
//! *rendering* of a ledger — read off [`Ledger::deeds`] — and if the
//! two disagree the document is stale. Parsing our own prose back in
//! would make an append-only history depend on a file anyone can edit
//! afterwards, which is the opposite of what the structure is for.
//!
//! What legitimately enters is an **observation of somebody else's
//! claim** — [`Act::Encumber`] carries `witnessed`, saying where the
//! claim was read. That is a fact arriving with provenance, not our own
//! output fed back.

use crate::layout::{Layout, Tag};

/// One entry. The ledger is these, in order, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// A run is claimed by a party outside this negotiation.
    ///
    /// An **observation**, and it carries where it was observed. Reading
    /// a neighbour's registry is a fact entering the chain; it is not
    /// this ledger deciding anything.
    Encumber {
        /// First tag, inclusive.
        low: Tag,
        /// Last tag, inclusive.
        high: Tag,
        /// Who claims it, as they name themselves.
        by: String,
        /// Where the claim was read. Provenance, so a later reader can
        /// check the observation rather than trust it.
        witnessed: String,
    },
    /// A deed was issued to a holder over a run.
    Issue {
        /// Who attached.
        holder: String,
        /// First tag, inclusive.
        low: Tag,
        /// Last tag, inclusive.
        high: Tag,
    },
    /// A holder gave up its deed. The tags do **not** return to the
    /// pool; this entry marks them spent.
    Retire {
        /// Who is retiring.
        holder: String,
    },

    // ===============================================================
    // THE AXES. Everything above acts on one line — an interval of
    // tags, issuance marching along it, exhaustion at the end of it.
    // That is linear progression, and it was the only shape the space
    // had. The acts below open more.
    // ===============================================================
    /// A new axis opens on this edge. **The space multiplies.**
    ///
    /// Identity on the edge stops being a scalar: with axes
    /// `tag × revision`, a point is `(64, 2)` and a deed is a box.
    /// Growth stops meaning *further along the line* and starts meaning
    /// *a new direction nothing yet occupies*.
    ///
    /// Every act recorded **before** an axis opened pins to coordinate
    /// 0 on it — the line the edge used to be is the zero slice of the
    /// space it becomes. So opening an axis grants nobody anything:
    /// the new coordinates are genuinely open, which is the point.
    Open {
        /// The axis's name, for reading the chain.
        axis: String,
        /// Largest coordinate, inclusive. Axis 0's extent comes from
        /// the layout; an opened axis declares its own.
        max: Tag,
    },
    /// An observation over a box rather than a run.
    EncumberBox {
        /// Per-axis inclusive ranges, in axis order.
        region: Vec<(Tag, Tag)>,
        /// Who claims it.
        by: String,
        /// Where the claim was read.
        witnessed: String,
    },
    /// A deed over a box rather than a run.
    IssueBox {
        /// Who attached.
        holder: String,
        /// Per-axis inclusive ranges, in axis order.
        region: Vec<(Tag, Tag)>,
    },
    /// Ground conveys from one holder to another, **continuously** — it
    /// is never open in between.
    ///
    /// Retired ground never reissues; that rule is about laundering
    /// space through the open pool. A cession is the other thing: the
    /// owner sells a slab of their estate and the buyer's deed begins
    /// where the owner's ends, with the chain recording the whole
    /// transfer. This is how a newcomer builds on a planet that is
    /// already relatively full — the space is bought, not conjured.
    ///
    /// The ceded region must be a **slab**: full extent on every axis
    /// but one, flush against an edge on that one — so the remainder is
    /// still a box, because deeds are boxes.
    Cede {
        /// The owner giving up the slab.
        from: String,
        /// The buyer. Must hold nothing (H1 survives conveyance).
        to: String,
        /// The slab, per-axis inclusive.
        region: Vec<(Tag, Tag)>,
    },

    /// An estate **inside** an estate. The moon.
    ///
    /// ## How this differs from [`Act::Cede`], which is the whole point
    ///
    /// A cession **transfers**: the slab leaves the owner's estate and
    /// the owner shrinks. A sublet **nests**: the sublessee gets a
    /// region *within* the owner's estate and **the owner keeps every
    /// point of it**. Two live deeds now cover the same ground on
    /// purpose, at different depths.
    ///
    /// That is a deliberate break of the disjointness the theorems were
    /// stated with, and it is why they are restated rather than
    /// weakened — see [`Ledger::well_formed`]:
    ///
    /// ```text
    /// H2   live deeds are pairwise disjoint
    /// H2'  live deeds AT THE SAME DEPTH are pairwise disjoint,
    ///      and every deed is strictly inside its parent
    /// ```
    ///
    /// `H2′` reduces to `H2` on a chain with no sublets, so nothing
    /// already proven is given up. What it buys is that the containment
    /// chain over any point is **totally ordered by depth**, so
    /// "deepest holder" is well defined and [`Ledger::holder_at`] is
    /// still a function — which is the property the cocycle theorem
    /// actually needed from `H2`, and which disjointness was only ever
    /// one way of getting.
    ///
    /// ## Why it is not just a smaller deed
    ///
    /// Because the owner is still there. A moon sits in a planet's
    /// estate, and if the planet is displaced the moon moves with it —
    /// so compensation owed to the planet is owed **through** it to
    /// everything it contains. [`Ledger::contained_in`] is that
    /// structure; pricing the cascade is the board's, not this crate's.
    Sublet {
        /// The owner, who keeps its estate.
        from: String,
        /// The sublessee. Must hold nothing (H1 survives nesting).
        to: String,
        /// The sub-estate, per-axis inclusive, strictly inside the
        /// owner's region.
        region: Vec<(Tag, Tag)>,
    },

    // ===============================================================
    // THE VERTICAL. Every act above is HORIZONTAL: it moves this
    // chain's own fold, and this chain's acts are totally ordered
    // among themselves because one party appends them.
    //
    // That was the whole shape of the chain, and it was a line. The
    // deed SPACE went multi-axial at `Open`; the chain carrying it
    // did not. Two substrates appending concurrently share no
    // instant, so a total order across them is a number nobody can
    // measure -- the same defect as a scalar in a negotiation, one
    // layer up, sitting in the ledger's spine.
    //
    // The act below is the other direction.
    // ===============================================================
    /// **Another chain exists, and I observed it at that state.**
    ///
    /// An *observation*, exactly like [`Act::Encumber`] — which is why
    /// it carries `witnessed` and why it grants nothing. Anchoring a
    /// chain does not adopt its deeds, submit to its authority, or
    /// merge its history. It records one fact with provenance: at this
    /// point in *my* history, *that* chain had that many acts and they
    /// digested to that value.
    ///
    /// ## What it buys
    ///
    /// An edge in the causal order. Acts in different chains are
    /// **unordered** — that is what having no shared instant means —
    /// *except* through anchors: an anchor at my height `h` naming
    /// chain `c` at height `k` orders every act of `c` below `k`
    /// before every act of mine at or above `h`. Chains that never
    /// anchor each other stay concurrent, and a conflict between two
    /// concurrent chains is arbitrated, never silently won.
    ///
    /// So the structure is a sphere of chains: horizontal within a chain,
    /// vertical between chains, and [`crate::sphere::Frontier`] is
    /// the join.
    ///
    /// ## Why the digest is opaque bytes
    ///
    /// This crate computes no digest and names no digest function. It
    /// is the issuer; **datum is the authority**, and which function
    /// hashes a chain is a fact about an edge, not about the wire.
    /// [`crate::sphere::confirms`] takes the function in.
    ///
    /// The width is not fixed either. A `[u8; 32]` here would be the
    /// same capacity-as-a-constant defect the rest of this file exists
    /// to have removed: it would pick one digest family forever, and
    /// the first peer using a wider one would need this crate
    /// recompiled to be observable at all.
    Anchor {
        /// The observed chain, as it names itself.
        chain: String,
        /// How many of its acts were seen. A **count**, so height 0 is
        /// the empty chain and there is no off-by-one to get wrong.
        height: u64,
        /// The digest of that prefix, uninterpreted here.
        digest: Vec<u8>,
        /// Where the chain was read. Provenance, so a later reader can
        /// check the observation rather than trust it.
        witnessed: String,
    },

    /// A holder's presenting key, bound **on the record** (`IS-6/4`).
    ///
    /// This is S3 of the signature layer: `key x the holder's grants x
    /// an epoch window`, as a chain fact rather than an allowlist. The
    /// key bytes are opaque here for the same reason an anchor's digest
    /// is — which scheme signs is an edge's decision, named by the
    /// scheme byte and interpreted by the signature leaf, never by
    /// this crate.
    ///
    /// A later bind for the same holder **supersedes** the earlier:
    /// rotation is an append, and the history of keys is the history —
    /// nothing is rewritten. A holder with no bind is **legacy /
    /// unbound**: readable, refusable by courts that demand keys, and
    /// visibly different from a holder whose key was recorded.
    ///
    /// Like an anchor, a bind covers no ground: it can never collide
    /// with a deed, which is what makes it safe to append to a live
    /// chain.
    Bind {
        /// Whose key this is, as the holder names itself.
        holder: String,
        /// The signature scheme byte (`0x01` = Ed25519/BLAKE3).
        scheme: u8,
        /// The public identity, uninterpreted here.
        key: Vec<u8>,
        /// First epoch this binding presents in, inclusive.
        from_epoch: u64,
        /// Last epoch this binding presents in, inclusive.
        until_epoch: u64,
    },

    /// A domain definition, published **on the record** (`IS-6/5`, UC4).
    ///
    /// The definition bytes are opaque here for the same reason a
    /// bind's key and an anchor's digest are: what an evaluation
    /// definition means is the court's leaf's business (the declared-
    /// complex codec today; whatever a fixed evaluator speaks
    /// tomorrow), never the chain's. The chain records that THIS
    /// holder published THESE bytes for THIS tag — and a court that
    /// resolves the tag learns the discipline from the chain alone,
    /// with no rebuild.
    ///
    /// A later declare for the same tag supersedes the earlier —
    /// definitions version by append, like keys. Whether the declarer
    /// actually holds the tag is the resolver's rule (registration
    /// requires holding the grant), applied at read time so a
    /// declaration by a since-retired holder visibly lapses.
    Declare {
        /// Who published, as the holder names itself.
        holder: String,
        /// The tag the definition governs.
        tag: Tag,
        /// The definition bytes, uninterpreted here.
        definition: Vec<u8>,
    },

    /// A holder's transport certificate, fingerprinted **on the
    /// record** (`IS-6/6`).
    ///
    /// TLS here buys confidentiality of the channel; it does not buy
    /// identity — that is what `Bind` and an attestation are for. A
    /// peer with no DNS name and no CA has nothing else to check a
    /// presented certificate against, so the chain carries the one
    /// fact that lets it: THIS holder's certificate hashes to THIS.
    /// A holder connecting to itself needs no round trip — it may
    /// record its own certify the same way genesis records a bind.
    ///
    /// A later certify for the same holder supersedes the earlier —
    /// certificate rotation is an append, exactly like key rotation.
    /// Like a bind, a certify covers no ground: safe to append to a
    /// live chain, and it can never collide with a deed.
    Certify {
        /// Whose certificate this is, as the holder names itself.
        holder: String,
        /// BLAKE3 of the certificate's exact DER bytes.
        fingerprint: [u8; 32],
    },

    /// A holder locked `amount` of its own earned balance as a
    /// refundable stake (`IS-6/7`, Phase 5 Fork B).
    ///
    /// Escrow buys **scheduling priority** at a court, nothing more — a
    /// court reads a holder's current locked amount ([`Ledger::escrow_of`])
    /// and weights its place in line by it, exactly as it reads a bind.
    /// The lock is voluntary and self-directed (a holder stakes its OWN
    /// balance, signed like any act), so it needs no more authority than
    /// a self-bind. What a court may LOCK is the court's rule, applied at
    /// admission (a lock may not exceed the holder's available balance) —
    /// not the chain's, which only records that the holder locked this.
    ///
    /// Additive, like every act since the founding: an older reader
    /// refuses the tag rather than misfolding. Like a bind, a lock covers
    /// no ground — safe to append to a live chain, never collides with a
    /// deed.
    Escrow {
        /// Whose stake this is, as the holder names itself.
        holder: String,
        /// How much balance is locked by this act, added to any already
        /// locked. **Never a settlement fact**: what a court weights this
        /// into is local scheduling policy, never a reward.
        amount: u128,
    },

    /// A holder unlocked its entire stake, returning it to spendable
    /// balance (`IS-6/7`). Refund, not extraction: releasing costs the
    /// holder only the priority the stake was buying.
    Release {
        /// Whose stake is released, as the holder names itself.
        holder: String,
    },

    /// A court destroyed `amount` of a holder's locked stake for a
    /// **consensus-verifiable** offence (`IS-6/7`, #56 ruling).
    ///
    /// The one involuntary act on a balance, and deliberately narrow: a
    /// slash may be triggered ONLY by something every court re-checks and
    /// agrees on — an attested-but-false proof, a double submission, a
    /// broken signed commitment — never by a scheduling or load judgment,
    /// which no federation can agree on. The chain records that the stake
    /// was destroyed; whether the trigger was legitimate is verified
    /// against the same chain by anyone, which is what keeps a slash a
    /// consensus act rather than one court's private punishment. Slashed
    /// value is burned, never redistributed (redistribution would pay a
    /// court to accuse). Covers no ground, like a bind.
    Slash {
        /// Whose stake is slashed, as the holder names itself.
        holder: String,
        /// How much locked stake is destroyed, subtracted from the
        /// holder's locked amount (saturating at zero).
        amount: u128,
    },
}

/// What [`Ledger::binding_of`] answers: the key a holder presents
/// under, and the window it presents in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The signature scheme byte.
    pub scheme: u8,
    /// The public identity bytes, opaque to this crate.
    pub key: Vec<u8>,
    /// First epoch, inclusive.
    pub from_epoch: u64,
    /// Last epoch, inclusive.
    pub until_epoch: u64,
}

/// If `region` is a slab of `parent`, the remainder box — `None` inside
/// the option-of-option meaning *full conveyance, nothing remains*.
fn slab_remainder(
    parent: &[(Tag, Tag)],
    region: &[(Tag, Tag)],
) -> Option<Option<Vec<(Tag, Tag)>>> {
    if parent.len() != region.len() || parent.is_empty() {
        return None;
    }
    // Inside on every axis.
    for ((plow, phigh), (rlow, rhigh)) in parent.iter().zip(region.iter()) {
        if rlow < plow || rhigh > phigh || rlow > rhigh {
            return None;
        }
    }
    // Axes where the region is not the parent's full extent.
    let partial: Vec<usize> = parent
        .iter()
        .zip(region.iter())
        .enumerate()
        .filter(|(_, (p, r))| p != r)
        .map(|(axis, _)| axis)
        .collect();

    match partial.as_slice() {
        [] => Some(None), // the whole estate conveys
        [axis] => {
            let (plow, phigh) = parent.get(*axis).copied()?;
            let (rlow, rhigh) = region.get(*axis).copied()?;
            let cut = if rlow == plow && rhigh < phigh {
                (rhigh.checked_add(1)?, phigh)
            } else if rhigh == phigh && rlow > plow {
                (plow, rlow.checked_sub(1)?)
            } else {
                return None; // an interior cut leaves two pieces
            };
            let mut remainder = parent.to_vec();
            *remainder.get_mut(*axis)? = cut;
            Some(Some(remainder))
        }
        _ => None, // partial on two axes: the remainder is not a box
    }
}

impl Act {
    /// The region an act covers, padded to `axes` dimensions.
    ///
    /// An act recorded before an axis opened carries fewer coordinates
    /// than the fold now has; it pins to `(0, 0)` on the axes it never
    /// knew — the zero slice. An act carrying **more** coordinates than
    /// were open when the fold reaches it covers nothing: that is a
    /// malformed history, and the fold stays total while the authority's
    /// validity checks name it.
    fn region(&self, axes: usize) -> Option<Vec<(Tag, Tag)>> {
        let mut region = match self {
            Act::Encumber { low, high, .. } | Act::Issue { low, high, .. } => {
                vec![(*low, *high)]
            }
            Act::EncumberBox { region, .. }
            | Act::IssueBox { region, .. }
            | Act::Cede { region, .. }
            | Act::Sublet { region, .. } => region.clone(),
            // An anchor covers no ground on *this* edge — it is a
            // fact about another chain, not a claim on this one. That
            // is what makes a vertical safe to append: it can never
            // collide with a horizontal.
            Act::Retire { .. }
            | Act::Open { .. }
            | Act::Anchor { .. }
            | Act::Bind { .. }
            | Act::Declare { .. }
            | Act::Certify { .. }
            // A stake covers no ground either: locking, releasing, or
            // slashing a balance is an economic fact, not a claim on the
            // tag line — safe to append to a live chain like a bind.
            | Act::Escrow { .. }
            | Act::Release { .. }
            | Act::Slash { .. } => return None,
        };
        if region.len() > axes {
            return None;
        }
        while region.len() < axes {
            region.push((0, 0));
        }
        Some(region)
    }
}

/// Two boxes intersect exactly when they intersect on every axis.
fn intersects(a: &[(Tag, Tag)], b: &[(Tag, Tag)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|((alow, ahigh), (blow, bhigh))| alow <= bhigh && blow <= ahigh)
}

/// Whether `inner` sits entirely inside `outer`, on every axis.
///
/// Containment, not intersection: a moon must be *within* the estate
/// that grants it, and a region that merely touches is a region that
/// covers ground the grantor does not hold.
fn contains(outer: &[(Tag, Tag)], inner: &[(Tag, Tag)]) -> bool {
    outer.len() == inner.len()
        && outer
            .iter()
            .zip(inner.iter())
            .all(|((olow, ohigh), (ilow, ihigh))| ilow >= olow && ihigh <= ohigh && ilow <= ihigh)
}

/// A point is in a box when every coordinate is in range.
fn covers_point(region: &[(Tag, Tag)], point: &[Tag]) -> bool {
    region.len() == point.len()
        && region
            .iter()
            .zip(point.iter())
            .all(|((low, high), at)| at >= low && at <= high)
}

/// What a run of tags is doing, from the point of view of one edge.
///
/// **Derived by folding [`Ledger::acts`]**, never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Never issued. A zero-filled buffer decodes to tag 0, so a frame
    /// arriving from nowhere must not name anything.
    Void,
    /// Taken before this edge existed, by a party that is not asking.
    ///
    /// **A fact, not a decision.** It is loaded from what the ancestors
    /// actually claim, and it is a property of the edge those ancestors
    /// share — not of the substrate. A third mesh that never carried
    /// those meanings inherits nothing.
    Encumbered {
        /// Who already holds it, as they name themselves.
        by: String,
    },
    /// Held by a deed this ledger issued.
    Deeded {
        /// Who holds it. **The holder, not an index** — an index into a
        /// derived list has to be kept aligned with the acts, and keeping
        /// two sequences aligned is a thing that can silently stop being
        /// true.
        holder: String,
    },
    /// Spent: a deed held it and retired. **Never reissued** — one tag,
    /// one meaning, for the life of the edge.
    Retired,
    /// Free to issue.
    Open,
}

/// One axis of an edge's identity space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Axis {
    /// The axis's name. Axis 0 is the layout's tag field; opened axes
    /// carry the name their [`Act::Open`] declared.
    pub name: String,
    /// Largest coordinate, inclusive.
    pub max: Tag,
}

/// The record of one negotiated attachment: a **box**, one range per
/// axis.
///
/// A deed issued while the edge was one line is a one-range box, and
/// reads as pinned to the zero slice of every axis opened later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deed {
    /// What the holder called itself when it attached. Not an identity
    /// claim — there is no issuer to check it against. It is a label for
    /// reading the ledger.
    pub holder: String,
    /// Per-axis inclusive ranges, padded to the edge's dimensionality.
    pub region: Vec<(Tag, Tag)>,
    /// Whether this deed is still in force.
    pub live: bool,
    /// The estate this one sits **inside**, if it is a sub-estate.
    ///
    /// `None` is ground held directly on the edge. `Some(holder)` is a
    /// moon: the named holder still holds every point of this region
    /// too, at one less depth. See [`Act::Sublet`].
    ///
    /// The parent is named rather than pointed at, for the same reason
    /// a holder is a `String`: a chain is a history, and a history that
    /// carried indices into itself would be a history that could not be
    /// read a record at a time.
    pub within: Option<String>,
}

/// Spread a record kind's name across a deed's width.
///
/// FNV-1a, 64-bit, specified here in full so an outside implementation
/// reproduces it exactly:
///
/// ```text
/// h = 0xcbf29ce484222325
/// for each byte b of the name (UTF-8):
///     h = (h XOR b) * 0x100000001b3   (mod 2^64)
/// ```
///
/// **Not a security function and not claimed as one.** It spreads names
/// over a small range; it hides nothing and authenticates nothing. That
/// is sufficient here because the offset is not a secret and because a
/// party choosing names that collide only collides *inside its own
/// deed* — see [`Deed::tag_for`]. A cryptographic hash would cost every
/// integrator a dependency to buy a property nothing here uses.
fn spread(kind: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in kind.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

impl Deed {
    /// **The tag this deed assigns to a named record kind — derived,
    /// never declared.**
    ///
    /// ## The defect this replaces
    ///
    /// `netstratum`'s mesh writes `MESH_HEAD_TAG: u8 = 64`. `IS-3` §5
    /// grants 64–79 to `isthmus`. Two substrate-layer protocols, each
    /// written without knowledge of the other, both reached for the
    /// same byte — and neither was wrong, because **each was choosing a
    /// global number from a private constant.**
    ///
    /// A constant cannot be right. It encodes an assumption about who
    /// else exists, and the set of who else exists is exactly the thing
    /// a substrate cannot know: a kernel that has not attached yet
    /// cannot have been consulted. Adding one is how the count of
    /// grants was fixed at six, one layer down, and it is the same
    /// defect wearing a number.
    ///
    /// ## The derivation
    ///
    /// ```text
    /// tag = deed.low() + (spread(kind) mod deed.width())
    /// ```
    ///
    /// Three properties follow, and they are what make it a standard
    /// rather than a convention:
    ///
    /// 1. **Deterministic.** A function of the kind and the deed alone.
    ///    Two parties that never meet compute the same answer for the
    ///    same edge, with nothing exchanged.
    /// 2. **Collision-free across holders, structurally.** Deeds are
    ///    disjoint (`H2′`), so a tag derived inside one deed cannot
    ///    land inside another. `netstratum`'s `head` and `isthmus`'s
    ///    `hello` cannot collide once both are derived, whatever they
    ///    are called.
    /// 3. **Stable under growth.** It does not depend on what *else*
    ///    the vocabulary contains, so declaring a tenth record kind
    ///    never moves the other nine. An assignment that probed for a
    ///    free slot would be denser and would move tags when the
    ///    vocabulary grew, which is a wire break dressed as an
    ///    optimisation.
    ///
    /// The cost of 3 is that two kinds in one vocabulary can land on
    /// one tag. That is **reported, not resolved** —
    /// [`Deed::collisions`] names the pair, and the author renames one.
    /// Resolving it silently would trade a visible refusal for a
    /// property nobody could rely on.
    ///
    /// `None` for a deed with no width.
    pub fn tag_for(&self, kind: &str) -> Option<Tag> {
        let offset = self.offset_for(kind)?;
        self.low().checked_add(offset)
    }

    /// The **offset** a kind takes inside its deed — the gauge-invariant
    /// half of [`Deed::tag_for`].
    ///
    /// This is what crosses an edge boundary. `potential_at` already
    /// says the offset within the box is the invariant and the box's
    /// origin is the frame; a record kind's identity is therefore its
    /// offset, and its absolute tag is a fact about one edge.
    ///
    /// So two edges that deed the same holder different ranges still
    /// agree, exactly, on what a `head` record *is*.
    pub fn offset_for(&self, kind: &str) -> Option<Tag> {
        // A deed with NO REGION is the case to refuse, and `width()`
        // does not report it: `low()` and `high()` both answer 0 for an
        // empty region, so the width reads as 1 and the derivation
        // lands on tag 0 — the void, which a zero-filled buffer decodes
        // to and which must name nothing.
        //
        // Measured, not reasoned about: `d4` caught it deriving
        // `Some(0)` from no region at all.
        if self.region.is_empty() {
            return None;
        }
        // `checked_rem` rather than `%`: this crate denies arithmetic
        // with side effects, and a remainder by zero is one. The guard
        // above already rules it out — carrying the check into the
        // operator means the total path is the only path.
        u128::from(spread(kind))
            .checked_rem(self.width())
            .and_then(|offset| Tag::try_from(offset).ok())
    }

    /// Pairs of kinds in one vocabulary that derive the same tag.
    ///
    /// Reported rather than resolved — see [`Deed::tag_for`]. An empty
    /// result is a vocabulary that fits; a non-empty one names exactly
    /// which two names to argue about.
    ///
    /// Deterministic and order-invariant: the input is sorted before
    /// comparison, so a vocabulary declared in a different order
    /// produces the same report.
    pub fn collisions(&self, kinds: &[&str]) -> Vec<(String, String, Tag)> {
        let mut sorted: Vec<&str> = kinds.to_vec();
        sorted.sort_unstable();
        sorted.dedup();

        let mut out = Vec::new();
        for (at, one) in sorted.iter().enumerate() {
            for other in sorted.iter().skip(at.saturating_add(1)) {
                match (self.tag_for(one), self.tag_for(other)) {
                    (Some(a), Some(b)) if a == b => {
                        out.push(((*one).to_owned(), (*other).to_owned(), a));
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// The axis-0 floor — the deed's first tag on the original line.
    pub fn low(&self) -> Tag {
        self.region.first().map_or(0, |(low, _)| *low)
    }

    /// The axis-0 ceiling.
    pub fn high(&self) -> Tag {
        self.region.first().map_or(0, |(_, high)| *high)
    }

    /// Tags covered on axis 0 — the reading a one-line edge calls the
    /// deed's width.
    pub fn width(&self) -> u128 {
        u128::from(self.high())
            .saturating_sub(u128::from(self.low()))
            .saturating_add(1)
    }

    // `volume()` was here: the product of this deed's extents across
    // every axis.
    //
    // A product calls `[2, 8]` and `[4, 4]` the same estate. They are
    // not the same estate and neither fits inside the other, so every
    // comparison built on it — pricing, containment, "a larger estate
    // never costs less" — was a total order over things that only have
    // a partial one.
    //
    // You fold a sheet of paper, not a sphere. The reading is per axis:
    // `region` carries it, `datum::extent::Extent` is the component
    // type, and containment is checked axis by axis.

    /// Whether this deed covers a tag on the zero slice — the one-line
    /// reading.
    pub fn covers(&self, tag: Tag) -> bool {
        let mut point = vec![tag];
        point.resize(self.region.len().max(1), 0);
        self.covers_at(&point)
    }

    /// Whether this deed covers a point.
    pub fn covers_at(&self, point: &[Tag]) -> bool {
        self.live && covers_point(&self.region, point)
    }

    /// The deed's boundary geometry: `2n` oriented facets, two per
    /// axis, orientation `+1` at the high face and `−1` at the low.
    ///
    /// **This is the multi-axial mapping law a closure proof reads:**
    /// a payload's extent components map to the deed's axes **in
    /// order**, components beyond the deed's dimensionality pin to the
    /// zero slice (the same padding rule every act obeys), and the
    /// boundary an assay closure proof walks is exactly these facets.
    /// The gauge-invariant reading over them is the per-axis potential
    /// — proven invariant under every re-gauge in `tests/theorem.rs` —
    /// so two kernels unfold the same bytes into the same spatial graph
    /// without sharing a frame.
    ///
    /// Opposite facets carry opposite orientation, which is what makes
    /// the boundary of the boundary vanish — the closure a closure
    /// proof is named for.
    pub fn facets(&self) -> Vec<Facet> {
        let mut out = Vec::new();
        for (axis, (low, high)) in self.region.iter().enumerate() {
            for (side, orientation) in [(*low, -1i8), (*high, 1i8)] {
                let mut face = self.region.clone();
                if let Some(slot) = face.get_mut(axis) {
                    *slot = (side, side);
                }
                out.push(Facet {
                    axis,
                    orientation,
                    region: face,
                });
            }
        }
        out
    }
}

/// One face of a deed's box: the axis it bounds, its orientation, and
/// the face itself — the deed's region flattened to a single
/// coordinate on that axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    /// The axis this facet bounds.
    pub axis: usize,
    /// `+1` at the high face, `−1` at the low — the alternation that
    /// makes ∂∂ vanish.
    pub orientation: i8,
    /// The face, as a region flat on `axis`.
    pub region: Vec<(Tag, Tag)>,
}

/// Why an attachment could not be granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// No free run of the requested width remains on this edge.
    ///
    /// Carries what *is* available, so a caller can ask for less rather
    /// than guess. A refusal that does not say how much is left forces
    /// the caller to probe.
    NoRun {
        /// Tags asked for.
        wanted: u128,
        /// The largest contiguous run that is open.
        largest_open: u128,
    },
    /// No open box of the requested shape remains.
    NoBox {
        /// Per-axis extents asked for.
        shape: Vec<u128>,
    },
    /// The shape does not match the edge's axes.
    WrongShape {
        /// Axes the edge has.
        axes: usize,
        /// Extents the request carried.
        asked: usize,
    },
    /// Maturation was asked over ground the holder has not claimed —
    /// open, retired, deeded, or encumbered by somebody else.
    NotYourClaim {
        /// Who asked.
        holder: String,
    },
    /// The named holder holds no live estate to cede from.
    NoSuchEstate {
        /// Who was named.
        holder: String,
    },
    /// The ceded region is not a slab of the owner's box — the
    /// remainder would not be a box, and deeds are boxes.
    NotASlab,
    /// The sub-estate is not inside the owner's estate.
    ///
    /// A sublet **nests**, so the region must be within what the owner
    /// actually holds. Granting a moon outside your own orbit is
    /// granting somebody else's ground.
    NotContained {
        /// Who tried to grant it.
        holder: String,
    },
    /// A holder cannot convey to itself.
    SelfDeal,
    /// The holder already holds a live deed on this edge.
    ///
    /// One attachment, one deed. This refusal is what turns the cocycle
    /// theorem's single-deed **hypothesis** into an **invariant**: a
    /// ledger evolved through the issuer cannot reach a state where one
    /// claim admits two points, by induction — the base ledger holds
    /// nothing, and this arm is the only way a second deed could have
    /// entered. A holder that needs more space retires and reattaches
    /// wider.
    AlreadyHeld {
        /// Who already holds.
        holder: String,
    },
    /// A width of zero is not an attachment.
    ZeroWidth,
}

/// Why a transcribed chain is not well-formed.
///
/// The issuer refuses these by construction; [`Ledger::record`]
/// transcribes without judging, so a chain that arrived from history is
/// **checked** instead — [`Ledger::well_formed`] is the decidable
/// predicate that discharges the theorem's hypotheses for chains the
/// issuer did not build.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Flaw {
    /// A holder was issued a second live deed.
    DoubleHold {
        /// Who, and the act index where the second arrived.
        holder: String,
        /// Position of the offending act in the chain.
        at: usize,
    },
    /// An issue landed on space already taken — encumbered, deeded, or
    /// retired.
    Overlap {
        /// Who was issued the overlapping deed.
        holder: String,
        /// Position of the offending act.
        at: usize,
    },
    /// An act carries more coordinates than axes open at its position.
    TooManyAxes {
        /// Position of the offending act.
        at: usize,
    },
    /// A cession from a holder with no estate, of a non-slab, to a
    /// holder that already holds, or to the ceder itself.
    BadCession {
        /// Position of the offending act.
        at: usize,
    },
    /// One axis name, two different extents.
    ///
    /// A repeated [`Act::Open`] with the *same* extent is a replay and
    /// folds to nothing — `IS-2` §6.1's rule, satisfied by the identity
    /// the act already carries. With a *different* extent it is not a
    /// replay but two irreconcilable statements about one direction,
    /// and the fold keeps the first. That is a lost effect rather than
    /// an idempotent one, so it is named.
    AxisRedeclared {
        /// The axis declared twice.
        axis: String,
        /// Position of the second declaration.
        at: usize,
    },
    /// A sublet from a holder with no estate, to a holder that already
    /// holds, to the granter itself, of a region not inside the
    /// granter's estate, or overlapping a sibling moon.
    ///
    /// The last two are `H2′`: a moon is strictly inside its planet,
    /// and moons of one planet are disjoint from each other. Together
    /// they are what keeps the containment chain over a point totally
    /// ordered, and therefore [`Ledger::holder_at`] a function.
    BadSublet {
        /// Position of the offending act.
        at: usize,
    },
}

/// The deeds one edge has issued, and the facts that constrain issuing.
///
/// **There is no maximum here.** The ledger issues until the edge's 256
/// tags are spent, and the substrate's capacity is the number of edges,
/// which is not a number this crate holds.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    layout: Layout,
    /// What this chain calls itself, so a stranger's [`Act::Anchor`]
    /// can name it. `None` is a real state, not a missing field: an
    /// **unnamed chain can anchor others but nobody can anchor it** —
    /// downstream works, upstream does not. That asymmetry is the
    /// honest default, because a name is not a fact a chain can
    /// establish about itself.
    ///
    /// A field and not an act, on the same footing as `layout`: both
    /// are the context the acts are read *in*, and neither is a thing
    /// that happened. Making it an act would also have meant rewriting
    /// the founding chain to insert a naming record at its head, which
    /// is editing history to record an identity.
    chain: Option<String>,
    acts: Vec<Act>,
}

impl Ledger {
    /// An edge with no acts on it, under a layout.
    ///
    /// **The layout comes in.** How many tags this edge can spell is a
    /// property of what its two peers negotiated, not of this type —
    /// which is why nothing here materialises a per-tag array. A
    /// four-byte tag field is `2^32` values and an array of that is not
    /// a data structure, it is a refusal to think about the question.
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            chain: None,
            acts: Vec::new(),
        }
    }

    /// Name this chain, so a stranger's [`Act::Anchor`] can address it.
    ///
    /// A builder rather than a second constructor: an unnamed chain is
    /// the base case and naming is the addition, which is also the
    /// order the substrate grew in.
    #[must_use]
    pub fn under(mut self, chain: &str) -> Self {
        self.chain = Some(chain.to_owned());
        self
    }

    /// What this chain calls itself, if anything. See [`Ledger::under`].
    pub fn name(&self) -> Option<&str> {
        self.chain.as_deref()
    }

    /// The layout this edge speaks.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The entries, in order. **This is the ledger** — everything else
    /// on this type is a fold over it.
    pub fn acts(&self) -> &[Act] {
        &self.acts
    }

    /// Rebuild a ledger from its entries.
    ///
    /// The only way to construct a non-empty one, which is what makes
    /// the acts the authority rather than a log kept alongside state.
    pub fn replay(layout: Layout, acts: Vec<Act>) -> Self {
        Self {
            layout,
            chain: None,
            acts,
        }
    }

    /// This chain as it stood after its first `height` acts.
    ///
    /// **The prefix is the unit of citation.** An [`Act::Anchor`] names
    /// a height, and what it observed is exactly this — so the fold a
    /// stranger anchored is reproducible, and the digest they recorded
    /// is a digest of something namable.
    ///
    /// A height past the end is the whole chain: you cannot observe
    /// more history than exists, and clamping says so without a
    /// refusal, because *asking* about a future height is not
    /// malformed — it is just early.
    #[must_use]
    pub fn at(&self, height: usize) -> Self {
        Self {
            layout: self.layout.clone(),
            chain: self.chain.clone(),
            acts: self
                .acts
                .get(..height.min(self.acts.len()))
                .unwrap_or(&self.acts)
                .to_vec(),
        }
    }

    /// This chain's own height — the number of acts on it.
    ///
    /// A **count**, matching [`Act::Anchor::height`](Act::Anchor), so
    /// `ledger.at(ledger.height())` is the whole chain and
    /// `ledger.at(0)` is the empty one.
    pub fn height(&self) -> u64 {
        self.acts.len() as u64
    }

    /// Record an observation of another chain: **the vertical**.
    ///
    /// Provenance-carrying and grant-free, exactly like
    /// [`Ledger::encumber`]. See [`Act::Anchor`] for what it does and
    /// does not buy.
    pub fn anchor(&mut self, chain: &str, height: u64, digest: &[u8], witnessed: &str) {
        self.acts.push(Act::Anchor {
            chain: chain.to_owned(),
            height,
            digest: digest.to_vec(),
            witnessed: witnessed.to_owned(),
        });
    }

    /// Record an observation that a run is claimed by a party outside
    /// this negotiation.
    ///
    /// `witnessed` says where the claim was read, so a later reader can
    /// check it rather than trust it. An observation without provenance
    /// is indistinguishable from this ledger having decided something.
    pub fn encumber(&mut self, low: Tag, high: Tag, by: &str, witnessed: &str) {
        self.acts.push(Act::Encumber {
            low,
            high,
            by: by.to_owned(),
            witnessed: witnessed.to_owned(),
        });
    }

    /// Issue a deed of the requested width.
    ///
    /// **The width is asked for, not fixed.** An attachment that needs
    /// three tags takes three; one that needs sixty takes sixty. Nothing
    /// here decides on the holder's behalf how much of a frame vocabulary
    /// it is going to need.
    pub fn issue(&mut self, holder: &str, width: u128) -> Result<Deed, Refused> {
        if width == 0 {
            return Err(Refused::ZeroWidth);
        }
        if self.holds_live(holder) {
            return Err(Refused::AlreadyHeld {
                holder: holder.to_owned(),
            });
        }
        let Some((low, high)) = self.first_run(width) else {
            return Err(Refused::NoRun {
                wanted: width,
                largest_open: self.largest_open(),
            });
        };
        self.acts.push(Act::Issue {
            holder: holder.to_owned(),
            low,
            high,
        });
        let mut region = vec![(low, high)];
        region.resize(self.axes().len(), (0, 0));
        Ok(Deed {
            holder: holder.to_owned(),
            region,
            live: true,
            within: None,
        })
    }

    /// Mature an observed claim into the claimant's deed.
    ///
    /// An encumbrance records that somebody *claims* a run; a deed is
    /// the run being *held*. When the claimant themselves asks, the
    /// claim matures: an [`Act::Issue`] lands on exactly the ground
    /// their encumbrance covers. This is the one lawful overlap — an
    /// issue may land on an encumbrance **by the same name**, because
    /// that is not a collision, it is a claim growing up.
    ///
    /// Refuses when any tag in the run is not `Encumbered` by exactly
    /// this holder, or when the holder already holds (H1 is not
    /// suspended for maturation).
    pub fn mature(&mut self, holder: &str, low: Tag, high: Tag) -> Result<Deed, Refused> {
        if self.holds_live(holder) {
            return Err(Refused::AlreadyHeld {
                holder: holder.to_owned(),
            });
        }
        for tag in low..=high {
            match self.standing_of(tag) {
                Standing::Encumbered { by } if by == holder => {}
                _ => {
                    return Err(Refused::NotYourClaim {
                        holder: holder.to_owned(),
                    })
                }
            }
        }
        self.acts.push(Act::Issue {
            holder: holder.to_owned(),
            low,
            high,
        });
        let mut region = vec![(low, high)];
        region.resize(self.axes().len(), (0, 0));
        Ok(Deed {
            holder: holder.to_owned(),
            region,
            live: true,
            within: None,
        })
    }

    /// Convey a slab of `from`'s estate to `to`, continuously.
    ///
    /// The purchase move: the ground is never open in between, so the
    /// never-reissue rule is not touched. H1 survives because `to`
    /// must hold nothing; H2 survives because the slab was `from`'s
    /// and the remainder plus the slab partition the original box.
    pub fn cede(
        &mut self,
        from: &str,
        to: &str,
        region: &[(Tag, Tag)],
    ) -> Result<Deed, Refused> {
        if from == to {
            return Err(Refused::SelfDeal);
        }
        if self.holds_live(to) {
            return Err(Refused::AlreadyHeld {
                holder: to.to_owned(),
            });
        }
        let owner = self
            .deeds()
            .into_iter()
            .find(|d| d.live && d.holder == from)
            .ok_or_else(|| Refused::NoSuchEstate {
                holder: from.to_owned(),
            })?;
        if slab_remainder(&owner.region, region).is_none() {
            return Err(Refused::NotASlab);
        }
        self.acts.push(Act::Cede {
            from: from.to_owned(),
            to: to.to_owned(),
            region: region.to_vec(),
        });
        Ok(Deed {
            holder: to.to_owned(),
            region: region.to_vec(),
            live: true,
            // A slab conveyed out of a moon is still in the planet.
            // Inheriting the seller's containment is what keeps a
            // cession from being a way to escape one: buying ground
            // from a sublessee must not lift it out of the estate the
            // sublessee itself sits in.
            within: owner.within.clone(),
        })
    }

    /// Grant a sub-estate **inside** an estate: the moon.
    ///
    /// Unlike [`Ledger::cede`], the owner keeps every point. See
    /// [`Act::Sublet`] for why that is the whole difference and what it
    /// costs in the theorems.
    ///
    /// H1 survives: `to` must hold nothing, so nesting adds a holder
    /// rather than a second deed for an existing one. H2′ survives by
    /// construction: the region is checked to be inside the owner's,
    /// and inside nothing else the owner has already sublet.
    pub fn sublet(
        &mut self,
        from: &str,
        to: &str,
        region: &[(Tag, Tag)],
    ) -> Result<Deed, Refused> {
        if from == to {
            return Err(Refused::SelfDeal);
        }
        if self.holds_live(to) {
            return Err(Refused::AlreadyHeld {
                holder: to.to_owned(),
            });
        }
        let owner = self
            .deeds()
            .into_iter()
            .find(|d| d.live && d.holder == from)
            .ok_or_else(|| Refused::NoSuchEstate {
                holder: from.to_owned(),
            })?;

        let width = owner.region.len().max(region.len());
        let mut asked = region.to_vec();
        asked.resize(width, (0, 0));
        let mut held = owner.region.clone();
        held.resize(width, (0, 0));
        if !contains(&held, &asked) {
            return Err(Refused::NotContained {
                holder: from.to_owned(),
            });
        }
        // Siblings do not overlap — H2′ at the child's depth. The
        // owner's OWN region is expected to overlap and is not
        // consulted here; that is the nesting.
        for moon in self.contained_in(from) {
            let mut theirs = moon.region.clone();
            theirs.resize(width, (0, 0));
            if intersects(&theirs, &asked) {
                return Err(Refused::NoBox {
                    shape: asked
                        .iter()
                        .map(|(low, high)| {
                            u128::from(*high)
                                .saturating_sub(u128::from(*low))
                                .saturating_add(1)
                        })
                        .collect(),
                });
            }
        }

        self.acts.push(Act::Sublet {
            from: from.to_owned(),
            to: to.to_owned(),
            region: asked.clone(),
        });
        Ok(Deed {
            holder: to.to_owned(),
            region: asked,
            live: true,
            within: Some(from.to_owned()),
        })
    }

    /// Open a new axis. **The space multiplies; nobody is displaced.**
    ///
    /// Every act already in the chain pins to coordinate 0 of the new
    /// axis — the line the edge was becomes the zero slice of the space
    /// it now is. Growth stops meaning *further along the line*.
    pub fn open_axis(&mut self, axis: &str, max: Tag) {
        self.acts.push(Act::Open {
            axis: axis.to_owned(),
            max,
        });
    }

    /// The edge's axes, in order. Axis 0 is the layout's tag field;
    /// the rest were opened by acts, and the chain says when.
    ///
    /// **An axis already open does not open again.** `IS-2` §6.1 rules
    /// that a frame with an effect must be idempotent under replay,
    /// *"either naturally or by carrying an identity the receiver
    /// dedups on"* — and [`Act::Open`] carries `axis`, which is that
    /// identity. Deduping on it is the ruled remedy, not a new one.
    ///
    /// Measured before the fix: `Open` was the **only** act that was
    /// both non-idempotent under replay and accepted by
    /// [`Ledger::well_formed`]. Issue, IssueBox and Cede are also
    /// non-idempotent, and all three are *refused* on the second
    /// application, so the effect never lands. Open replayed doubled
    /// the axis count and multiplied the volume, quietly.
    ///
    /// A second `Open` of the same name with a **different** `max` is
    /// not a replay — it is a contradiction — and
    /// [`Flaw::AxisRedeclared`] names it.
    pub fn axes(&self) -> Vec<Axis> {
        let mut axes = vec![Axis {
            name: crate::layout::TAG.to_owned(),
            max: self.layout.max_tag().unwrap_or(0),
        }];
        for act in &self.acts {
            if let Act::Open { axis, max } = act {
                if axes.iter().any(|open| &open.name == axis) {
                    continue;
                }
                axes.push(Axis {
                    name: axis.clone(),
                    max: *max,
                });
            }
        }
        axes
    }

    /// How many axes are open **at** each act's position.
    ///
    /// `out[i]` is the axis count the act at index `i` was recorded
    /// under — 1, plus one for each *distinct* axis opened strictly
    /// before it.
    ///
    /// **One place, four folds.** The dedup rule ([`Ledger::axes`])
    /// has to hold at every site that walks the acts, and four inline
    /// copies of a rule is how four sites drift apart. Before this,
    /// each fold counted `Open` acts with its own `axes_now` counter,
    /// so making a replayed `Open` inert in one place would have left
    /// the other three disagreeing with it about the shape of the
    /// space.
    fn axes_timeline(&self) -> Vec<usize> {
        let mut opened: std::collections::BTreeSet<&str> =
            std::collections::BTreeSet::new();
        let mut now = 1usize;
        let mut out = Vec::with_capacity(self.acts.len());
        for act in &self.acts {
            out.push(now);
            if let Act::Open { axis, .. } = act {
                if opened.insert(axis.as_str()) {
                    now = now.saturating_add(1);
                }
            }
        }
        out
    }

    // `volume()` was here: the product of the axes' extents.
    //
    // Struck with `Deed::volume` and for the same reason. The space a
    // court spans is `axes()` — one named extent per direction — and
    // multiplying them out is the flattening that makes two different
    // spaces read as one.

    /// Issue a deed over a box: one extent per axis.
    ///
    /// The one-line [`Ledger::issue`] is this with shape `[width]` on a
    /// one-axis edge. Finding is by corner candidates — an open box, if
    /// one exists, exists with its origin at 0/1 or flush against some
    /// taken region's boundary, so only those origins are tried rather
    /// than every point of a space that can be `2^64` per axis.
    pub fn issue_box(&mut self, holder: &str, shape: &[u128]) -> Result<Deed, Refused> {
        let axes = self.axes();
        if shape.len() != axes.len() {
            return Err(Refused::WrongShape {
                axes: axes.len(),
                asked: shape.len(),
            });
        }
        if shape.contains(&0) {
            return Err(Refused::ZeroWidth);
        }
        if self.holds_live(holder) {
            return Err(Refused::AlreadyHeld {
                holder: holder.to_owned(),
            });
        }

        let taken = self.taken_regions(axes.len());

        // Candidate origins per axis: the floor (1 on axis 0 — the void
        // — and 0 elsewhere), plus one past every taken region's high.
        let mut candidates: Vec<Vec<Tag>> = Vec::new();
        for (at, axis) in axes.iter().enumerate() {
            let mut this = vec![if at == 0 { 1 } else { 0 }];
            for region in &taken {
                if let Some((_, high)) = region.get(at) {
                    this.push(high.saturating_add(1));
                }
            }
            this.sort_unstable();
            this.dedup();
            this.retain(|origin| *origin <= axis.max);
            candidates.push(this);
        }

        // Odometer over the candidate origins.
        let mut at = vec![0usize; axes.len()];
        'search: loop {
            let origin: Vec<Tag> = at
                .iter()
                .enumerate()
                .filter_map(|(axis, index)| candidates.get(axis)?.get(*index).copied())
                .collect();

            if origin.len() == axes.len() {
                let mut fits = true;
                let mut region: Vec<(Tag, Tag)> = Vec::new();
                for (axis, (start, extent)) in origin.iter().zip(shape.iter()).enumerate() {
                    let span = Tag::try_from(extent.saturating_sub(1)).unwrap_or(Tag::MAX);
                    let Some(end) = start.checked_add(span) else {
                        fits = false;
                        break;
                    };
                    let Some(bound) = axes.get(axis) else {
                        fits = false;
                        break;
                    };
                    if end > bound.max {
                        fits = false;
                        break;
                    }
                    region.push((*start, end));
                }
                if fits && !taken.iter().any(|held| intersects(&region, held)) {
                    self.acts.push(Act::IssueBox {
                        holder: holder.to_owned(),
                        region: region.clone(),
                    });
                    return Ok(Deed {
                        holder: holder.to_owned(),
                        region,
                        live: true,
                        within: None,
                    });
                }
            }

            // Advance the odometer.
            for axis in (0..at.len()).rev() {
                let Some(digit) = at.get_mut(axis) else { break };
                *digit = digit.saturating_add(1);
                let width = candidates.get(axis).map_or(0, Vec::len);
                if *digit < width {
                    continue 'search;
                }
                *digit = 0;
            }
            break;
        }

        Err(Refused::NoBox {
            shape: shape.to_vec(),
        })
    }

    /// Retire a deed. Its tags become [`Standing::Retired`] and are
    /// **never reissued**.
    ///
    /// An entry, not a deletion. Reissuing would hand a newcomer a number
    /// an old peer still remembers the meaning of, and the newcomer would
    /// be right about the number and wrong about everything else.
    pub fn retire(&mut self, holder: &str) -> bool {
        let held = self.deeds().iter().any(|d| d.live && d.holder == holder);
        if held {
            self.acts.push(Act::Retire {
                holder: holder.to_owned(),
            });
        }
        held
    }

    /// What one tag is doing on the **zero slice** — the one-line
    /// reading, unchanged for one-axis edges.
    pub fn standing_of(&self, tag: Tag) -> Standing {
        let mut point = vec![tag];
        point.resize(self.axes().len(), 0);
        self.standing_at(&point)
    }

    /// What one point is doing on this edge, folded from
    /// [`Ledger::acts`].
    ///
    /// **Folded per point, not materialised.** An earlier version built
    /// a 256-entry `Vec<Standing>` — a cache with no history and a hard
    /// assumption that the space is one line of 256. A multi-axis space
    /// can be `2^64` per axis; materialising it is not a data structure.
    pub fn standing_at(&self, point: &[Tag]) -> Standing {
        let final_axes = self.axes().len();
        // Tag 0 on axis 0 is never issued: a zero-filled buffer decodes
        // to it, so a record arriving from nowhere must not name
        // anything. The void is a line's worth of points now — every
        // coordinate over tag 0.
        if point.first().is_none_or(|tag| *tag == 0) {
            return Standing::Void;
        }
        if point.len() != final_axes {
            return Standing::Void;
        }
        if let Some(tag) = point.first() {
            if !self.layout.holds(*tag) {
                return Standing::Void;
            }
        }

        let mut standing = Standing::Open;
        let timeline = self.axes_timeline();
        for (at, act) in self.acts.iter().enumerate() {
            let axes_now = timeline.get(at).copied().unwrap_or(1);
            match act {
                // Opening changes nothing here — the timeline carries
                // it, and a replayed open carries nothing at all.
                Act::Open { .. } => {}
                Act::Retire { holder } => {
                    if standing
                        == (Standing::Deeded {
                            holder: holder.clone(),
                        })
                    {
                        standing = Standing::Retired;
                    }
                }
                other => {
                    // Validate against the axes open when the act
                    // happened, then read in the space's final shape.
                    let Some(mut region) = other.region(axes_now) else {
                        continue;
                    };
                    region.resize(final_axes, (0, 0));
                    if !covers_point(&region, point) {
                        continue;
                    }
                    match other {
                        Act::Encumber { by, .. } | Act::EncumberBox { by, .. } => {
                            if matches!(standing, Standing::Open) {
                                standing = Standing::Encumbered { by: by.clone() };
                            }
                        }
                        Act::Issue { holder, .. } | Act::IssueBox { holder, .. } => {
                            standing = Standing::Deeded {
                                holder: holder.clone(),
                            };
                        }
                        // Ground conveys only from its actual holder — a
                        // cession recorded against someone else's point
                        // moves nothing there.
                        Act::Cede { from, to, .. }
                            if standing
                                == (Standing::Deeded {
                                    holder: from.clone(),
                                }) =>
                        {
                            standing = Standing::Deeded { holder: to.clone() };
                        }
                        // A moon answers for its own points. The owner
                        // still holds them — `contained_in` says so —
                        // but a standing is one name, and the useful
                        // one is the deepest.
                        Act::Sublet { from, to, .. }
                            if standing
                                == (Standing::Deeded {
                                    holder: from.clone(),
                                }) =>
                        {
                            standing = Standing::Deeded { holder: to.clone() };
                        }
                        _ => {}
                    }
                }
            }
        }
        standing
    }

    /// How deep an estate sits. `0` is ground held on the edge itself,
    /// `1` a moon, `2` a moon of a moon.
    ///
    /// Walks the `within` chain, and **stops** if it ever revisits a
    /// name. A cycle cannot be built through the issuer — a sublessee
    /// must hold nothing, so it cannot already be somebody's parent —
    /// but a transcribed chain can carry one, and a fold that looped on
    /// it would hang rather than answer. Totality first; the checker
    /// names the cycle.
    pub fn depth_of(&self, holder: &str) -> usize {
        let deeds = self.deeds();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut at = holder.to_owned();
        let mut depth = 0usize;
        while seen.insert(at.clone()) {
            let Some(parent) = deeds
                .iter()
                .find(|d| d.live && d.holder == at)
                .and_then(|d| d.within.clone())
            else {
                break;
            };
            depth = depth.saturating_add(1);
            at = parent;
        }
        depth
    }

    /// The estates held **directly inside** this one — its moons.
    ///
    /// One level, not the whole subtree: the compensation cascade is
    /// paid *through* each level, so a caller that wants the subtree
    /// walks it and can price each step. Flattening here would hand the
    /// board a list with the containment discarded, which is the one
    /// thing it needs.
    pub fn contained_in(&self, holder: &str) -> Vec<Deed> {
        self.deeds()
            .into_iter()
            .filter(|d| d.live && d.within.as_deref() == Some(holder))
            .collect()
    }

    /// The key `holder` presents under, if one was ever bound.
    ///
    /// The **last** bind wins: rotation is an append, and the answer
    /// is derived from the acts on demand like every other property of
    /// this ledger — a cached key would be a second place identity
    /// lives. `None` is legacy/unbound, which a court may refuse and a
    /// reader must be able to see.
    #[must_use]
    pub fn binding_of(&self, holder: &str) -> Option<Binding> {
        self.acts.iter().rev().find_map(|act| match act {
            Act::Bind {
                holder: bound,
                scheme,
                key,
                from_epoch,
                until_epoch,
            } if bound == holder => Some(Binding {
                scheme: *scheme,
                key: key.clone(),
                from_epoch: *from_epoch,
                until_epoch: *until_epoch,
            }),
            _ => None,
        })
    }

    /// The certificate fingerprint `holder` last recorded, if any
    /// (`IS-6/6`). The **last** certify wins, the same rotation rule
    /// as [`Ledger::binding_of`] — a holder with no certify has
    /// published no transport certificate at all, which a connecting
    /// peer must treat as "nothing to check against," never as "skip
    /// the check."
    #[must_use]
    pub fn fingerprint_of(&self, holder: &str) -> Option<[u8; 32]> {
        self.acts.iter().rev().find_map(|act| match act {
            Act::Certify {
                holder: certified,
                fingerprint,
            } if certified == holder => Some(*fingerprint),
            _ => None,
        })
    }

    /// How much `holder` currently has locked as a stake (`IS-6/7`,
    /// Phase 5). A FORWARD fold, not last-wins: each [`Act::Escrow`]
    /// adds, a [`Act::Release`] returns the whole stake to zero, and a
    /// [`Act::Slash`] subtracts (saturating). The number a court reads to
    /// weight this holder's place in line — an escrow amount is a shared,
    /// verifiable chain fact, but the WEIGHT a court derives from it is
    /// local scheduling policy and never re-enters consensus.
    #[must_use]
    pub fn escrow_of(&self, holder: &str) -> u128 {
        let mut locked: u128 = 0;
        for act in &self.acts {
            match act {
                Act::Escrow { holder: h, amount } if h == holder => {
                    locked = locked.saturating_add(*amount);
                }
                Act::Release { holder: h } if h == holder => locked = 0,
                Act::Slash { holder: h, amount } if h == holder => {
                    locked = locked.saturating_sub(*amount);
                }
                _ => {}
            }
        }
        locked
    }

    /// How much of `holder`'s stake has been destroyed by slashing over
    /// all time (`IS-6/7`). Unlike [`Ledger::escrow_of`], a release does
    /// not reset this — slashed value is gone for good, so it is
    /// subtracted from earned balance permanently (see the court's
    /// `balance_of`).
    #[must_use]
    pub fn slashed_of(&self, holder: &str) -> u128 {
        self.acts.iter().fold(0u128, |sum, act| match act {
            Act::Slash { holder: h, amount } if h == holder => sum.saturating_add(*amount),
            _ => sum,
        })
    }

    /// The definition governing `tag`, if a **current holder** of the
    /// tag ever published one (UC4).
    ///
    /// The resolver's rule, applied at read time: the last `Declare`
    /// for the tag whose declarer holds the tag's live deed NOW.
    /// A definition published by a since-retired holder lapses with
    /// the deed — a vocabulary does not outlive its grant.
    #[must_use]
    pub fn declaration_of(&self, tag: Tag) -> Option<Vec<u8>> {
        let holder = self.holder_of(tag).filter(|d| d.live)?.holder;
        self.acts.iter().rev().find_map(|act| match act {
            Act::Declare {
                holder: declared_by,
                tag: declared_tag,
                definition,
            } if *declared_tag == tag && *declared_by == holder => {
                Some(definition.clone())
            }
            _ => None,
        })
    }

    /// Which deed holds a tag on the zero slice, if a live one does.
    pub fn holder_of(&self, tag: Tag) -> Option<Deed> {
        self.deepest(|d| d.covers(tag))
    }

    /// Which deed holds a point, if a live one does.
    ///
    /// **The deepest one.** A point inside a moon is inside its planet
    /// too, so "who holds it" has more than one true answer and only
    /// one useful one. `H2′` is what makes the choice well defined:
    /// deeds at the same depth are disjoint, so the containment chain
    /// over a point is totally ordered and has a unique maximum.
    pub fn holder_at(&self, point: &[Tag]) -> Option<Deed> {
        self.deepest(|d| d.covers_at(point))
    }

    fn deepest(&self, covers: impl Fn(&Deed) -> bool) -> Option<Deed> {
        self.deeds()
            .into_iter()
            .filter(|d| covers(d))
            .max_by_key(|d| self.depth_of(&d.holder))
    }

    /// Every region that is not open, padded to the space's final
    /// shape. A retired region counts as taken — never reissued.
    fn taken_regions(&self, final_axes: usize) -> Vec<Vec<(Tag, Tag)>> {
        let mut out = Vec::new();
        let timeline = self.axes_timeline();
        for (at, act) in self.acts.iter().enumerate() {
            let axes_now = timeline.get(at).copied().unwrap_or(1);
            match act {
                Act::Open { .. } => {}
                Act::Retire { .. } => {}
                other => {
                    if let Some(mut region) = other.region(axes_now) {
                        region.resize(final_axes, (0, 0));
                        out.push(region);
                    }
                }
            }
        }
        out
    }

    /// Every axis-0 run that is not open **on the zero slice**, merged
    /// and in order.
    ///
    /// The sparse form the one-line gap arithmetic works over. A box
    /// that never touches the zero slice does not appear here — it
    /// occupies the space, not the line.
    fn taken(&self) -> Vec<(Tag, Tag)> {
        let final_axes = self.axes().len();
        let mut runs: Vec<(Tag, Tag)> = vec![(0, 0)]; // the void at tag 0
        for region in self.taken_regions(final_axes) {
            let touches_zero_slice = region
                .iter()
                .skip(1)
                .all(|(low, _)| *low == 0);
            if touches_zero_slice {
                if let Some((low, high)) = region.first() {
                    runs.push((*low, *high));
                }
            }
        }
        runs.sort_unstable();

        let mut merged: Vec<(Tag, Tag)> = Vec::new();
        for (low, high) in runs {
            match merged.last_mut() {
                Some((_, end)) if low <= end.saturating_add(1) => *end = (*end).max(high),
                _ => merged.push((low, high)),
            }
        }
        merged
    }

    /// Open runs, in order, as `(low, high)` inclusive.
    ///
    /// The complement of the taken runs inside the layout's tag space.
    /// Everything about capacity is read off this rather than off a
    /// count somebody wrote down.
    pub fn gaps(&self) -> Vec<(Tag, Tag)> {
        let Some(max) = self.layout.max_tag() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut at: Tag = 0;
        for (low, high) in self.taken() {
            if low > at {
                out.push((at, low.saturating_sub(1)));
            }
            at = at.max(high.saturating_add(1));
            if at == 0 {
                // high was Tag::MAX and wrapped; nothing is left.
                return out;
            }
        }
        if at <= max {
            out.push((at, max));
        }
        out
    }

    /// Every deed ever issued on this edge, retired ones included, in
    /// the order they were issued.
    ///
    /// Folded from the acts. There is no stored list to fall out of step
    /// with them.
    pub fn deeds(&self) -> Vec<Deed> {
        let final_axes = self.axes().len();
        let mut out: Vec<Deed> = Vec::new();
        let timeline = self.axes_timeline();
        for (at, act) in self.acts.iter().enumerate() {
            let axes_now = timeline.get(at).copied().unwrap_or(1);
            match act {
                Act::Open { .. } => {}
                // A vertical issues nothing. Anchoring a chain does
                // not adopt its deeds — if it did, observing a
                // stranger would enlarge my estate, and every party
                // could grow by looking at things.
                Act::Anchor { .. } => {}
                // A key issues nothing either: binding is identity,
                // not ground.
                Act::Bind { .. } => {}
                // A definition issues nothing: publishing is speech,
                // not ground.
                Act::Declare { .. } => {}
                // A certificate fingerprint issues nothing either —
                // transport hygiene, not ground.
                Act::Certify { .. } => {}
                // A stake issues nothing: locking, releasing, or slashing
                // a balance is economics, not ground.
                Act::Escrow { .. } | Act::Release { .. } | Act::Slash { .. } => {}
                Act::Issue { holder, .. } | Act::IssueBox { holder, .. } => {
                    let Some(mut region) = act.region(axes_now) else {
                        continue;
                    };
                    region.resize(final_axes, (0, 0));
                    out.push(Deed {
                        holder: holder.clone(),
                        region,
                        live: true,
                        within: None,
                    });
                }
                Act::Retire { holder } => {
                    for deed in out.iter_mut().filter(|d| &d.holder == holder) {
                        deed.live = false;
                    }
                }
                Act::Cede { from, to, region } => {
                    let Some(mut slab) = act.region(axes_now) else {
                        continue;
                    };
                    slab.resize(final_axes, (0, 0));
                    let Some(owner) = out
                        .iter_mut()
                        .find(|d| d.live && &d.holder == from)
                    else {
                        continue; // transcribed garbage; the checker names it
                    };
                    // Read before the borrow ends: the buyer inherits
                    // the seller's containment, so buying out of a moon
                    // does not lift the ground out of the planet.
                    let inherited = owner.within.clone();
                    match slab_remainder(&owner.region, &slab) {
                        Some(Some(remainder)) => owner.region = remainder,
                        Some(None) => owner.live = false, // fully conveyed
                        None => continue,
                    }
                    let _ = region; // the padded slab is authoritative
                    out.push(Deed {
                        holder: to.clone(),
                        region: slab,
                        live: true,
                        within: inherited,
                    });
                }
                // The moon. The owner keeps every point; the sublessee
                // gets a deed one level down, inside it.
                Act::Sublet { from, to, .. } => {
                    let Some(mut region) = act.region(axes_now) else {
                        continue;
                    };
                    region.resize(final_axes, (0, 0));
                    if !out.iter().any(|d| d.live && &d.holder == from) {
                        continue; // no such estate; the checker names it
                    }
                    out.push(Deed {
                        holder: to.clone(),
                        region,
                        live: true,
                        within: Some(from.clone()),
                    });
                }
                Act::Encumber { .. } | Act::EncumberBox { .. } => {}
            }
        }
        out
    }

    /// How many tags remain issuable.
    ///
    /// `u128` for the same reason [`Layout::tag_space`] is: on a wide
    /// layout the answer does not fit a [`Tag`].
    pub fn open(&self) -> u128 {
        self.gaps().iter().fold(0u128, |sum, (low, high)| {
            sum.saturating_add(
                u128::from(*high)
                    .saturating_sub(u128::from(*low))
                    .saturating_add(1),
            )
        })
    }

    /// The largest contiguous run still open.
    ///
    /// The number that actually answers *can another mesh attach* — the
    /// total open count does not, because a deed is contiguous so a
    /// grantee can claim inside its own range without another's landing
    /// on top.
    pub fn largest_open(&self) -> u128 {
        self.gaps()
            .iter()
            .map(|(low, high)| {
                u128::from(*high)
                    .saturating_sub(u128::from(*low))
                    .saturating_add(1)
            })
            .max()
            .unwrap_or(0)
    }

    /// Move a frame from this edge's numbering to another's.
    ///
    /// Forwarding crosses a deed boundary, so it is a change of
    /// coordinates. The **meaning** travels; the number does not.
    ///
    /// Returns the tag to send under on `onto`, or `None` when the
    /// destination has no deed for that holder — in which case the frame
    /// is not forwardable *as that holder's frame*, and a substrate that
    /// forwarded the raw number would be mistranslating rather than
    /// carrying.
    pub fn translate(&self, tag: Tag, onto: &Ledger) -> Option<Tag> {
        let mut point = vec![tag];
        point.resize(self.axes().len(), 0);
        let mapped = self.translate_at(&point, onto)?;
        mapped.first().copied()
    }

    /// Move a point across a deed boundary.
    ///
    /// The far deed must belong to the same holder and have the same
    /// per-axis extents — the box is a coordinate patch, and a patch
    /// maps only onto a patch of its own shape. Offsets are preserved
    /// per axis.
    pub fn translate_at(&self, point: &[Tag], onto: &Ledger) -> Option<Vec<Tag>> {
        let here = self.holder_at(point)?;
        let there = onto.deeds().into_iter().find(|d| {
            d.live
                && d.holder == here.holder
                && d.region.len() == here.region.len()
                && d.region
                    .iter()
                    .zip(here.region.iter())
                    .all(|((tlow, thigh), (hlow, hhigh))| {
                        thigh.saturating_sub(*tlow) == hhigh.saturating_sub(*hlow)
                    })
        })?;

        let mut mapped = Vec::new();
        for ((at, (hlow, _)), (tlow, thigh)) in point
            .iter()
            .zip(here.region.iter())
            .zip(there.region.iter())
        {
            let offset = at.checked_sub(*hlow)?;
            let landed = tlow.checked_add(offset)?;
            if landed > *thigh {
                return None;
            }
            mapped.push(landed);
        }
        Some(mapped)
    }

    /// Whether a holder has a live deed on this edge.
    fn holds_live(&self, holder: &str) -> bool {
        self.deeds().iter().any(|d| d.live && d.holder == holder)
    }

    /// The decidable well-formedness predicate: the theorems'
    /// hypotheses, checked over a chain the issuer did not build.
    ///
    /// The issuer refuses double-holds and overlaps **by construction**
    /// (induction: the empty ledger satisfies both, and every issuing
    /// arm preserves them). [`Ledger::record`] transcribes history
    /// without judging, so for transcribed chains the hypotheses are
    /// discharged by running this instead:
    ///
    /// - **no double hold** — at every prefix, each holder has at most
    ///   one live deed. This is what makes the admitted relation a
    ///   *function* (one claim, one point).
    /// - **no overlap** — an issue never lands on space already taken.
    ///   This is what makes [`Ledger::holder_at`] single-valued.
    /// - **no future axes** — an act never carries more coordinates
    ///   than axes open at its position. A history that references a
    ///   direction that did not exist yet is not a history.
    ///
    /// Encumbrances may overlap each other: both ancestors claim the
    /// frozen band, and two observations of a collision are two true
    /// observations.
    pub fn well_formed(&self) -> Result<(), Flaw> {
        let timeline = self.axes_timeline();
        let mut declared: std::collections::BTreeMap<&str, Tag> =
            std::collections::BTreeMap::new();
        let mut live: std::collections::BTreeMap<String, Vec<(Tag, Tag)>> =
            std::collections::BTreeMap::new();
        // Who sits inside whom. Carried alongside `live` rather than
        // derived, because H2′ is checked at the moment of the act and
        // the fold has not finished yet.
        let mut within: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        // Taken ground with provenance: `Some(by)` for an encumbrance,
        // `None` for issued or retired ground. Provenance is what makes
        // maturation lawful — an issue may land on an encumbrance by
        // the same name, and on nothing else.
        type TakenGround = Vec<(Vec<(Tag, Tag)>, Option<String>)>;
        let mut taken: TakenGround = Vec::new();

        let pad = |mut region: Vec<(Tag, Tag)>, to: usize| -> Vec<(Tag, Tag)> {
            region.resize(to, (0, 0));
            region
        };

        for (at, act) in self.acts.iter().enumerate() {
            let axes_now = timeline.get(at).copied().unwrap_or(1);
            match act {
                // A repeated `Open` is a replay and folds to nothing
                // ([`Ledger::axes`]), so it is not a flaw. The same
                // name with a DIFFERENT extent is not a replay — it is
                // two irreconcilable statements about one direction,
                // and the fold silently keeps the first. Naming it is
                // the difference between an idempotent effect and a
                // lost one.
                Act::Open { axis, max } => {
                    match declared.insert(axis.as_str(), *max) {
                        Some(before) if before != *max => {
                            return Err(Flaw::AxisRedeclared {
                                axis: axis.clone(),
                                at,
                            })
                        }
                        _ => {}
                    }
                }
                // A vertical constrains nothing about this edge's
                // ground, so well-formedness of the horizontal is
                // blind to it — and must be, or my history would
                // become ill-formed because of what a stranger
                // appended to theirs. Whether an anchor's digest is
                // *true* is `sphere::confirms` against the foreign
                // chain: a different question from whether this
                // history is internally consistent, and answerable
                // only by someone holding both.
                Act::Anchor { .. } => {}
                // A bind covers no ground and names no ground, so no
                // horizontal rule can trip on it. Whether its window is
                // honored is the court's enforcement, not chain shape.
                Act::Bind { .. } => {}
                // Likewise a declaration: whether the declarer holds
                // the tag is the resolver's read-time rule.
                Act::Declare { .. } => {}
                // Likewise a certify: it names no ground either.
                Act::Certify { .. } => {}
                // A stake names no ground either — locking, releasing, or
                // slashing a balance cannot trip a horizontal rule.
                Act::Escrow { .. } | Act::Release { .. } | Act::Slash { .. } => {}
                Act::Retire { holder } => {
                    live.remove(holder);
                }
                Act::Encumber { by, .. } | Act::EncumberBox { by, .. } => {
                    let Some(region) = act.region(axes_now) else {
                        return Err(Flaw::TooManyAxes { at });
                    };
                    taken.push((region, Some(by.clone())));
                }
                Act::Issue { holder, .. } | Act::IssueBox { holder, .. } => {
                    let Some(region) = act.region(axes_now) else {
                        return Err(Flaw::TooManyAxes { at });
                    };
                    if live.contains_key(holder) {
                        return Err(Flaw::DoubleHold {
                            holder: holder.clone(),
                            at,
                        });
                    }
                    let width = region.len();
                    for (held, provenance) in &taken {
                        // Maturation: the holder's own observed claim is
                        // theirs to grow into. Everything else refuses.
                        if provenance.as_deref() == Some(holder.as_str()) {
                            continue;
                        }
                        let to = width.max(held.len());
                        if intersects(&pad(region.clone(), to), &pad(held.clone(), to)) {
                            return Err(Flaw::Overlap {
                                holder: holder.clone(),
                                at,
                            });
                        }
                    }
                    live.insert(holder.clone(), region.clone());
                    taken.push((region, None));
                }
                // H2′ at the child's depth. The owner's own region is
                // EXPECTED to contain this one — that is the nesting —
                // so the taken-set is not consulted. What is checked is
                // that the moon is inside its planet and clear of its
                // siblings.
                Act::Sublet { from, to, .. } => {
                    let Some(region) = act.region(axes_now) else {
                        return Err(Flaw::TooManyAxes { at });
                    };
                    if from == to || live.contains_key(to) {
                        return Err(Flaw::BadSublet { at });
                    }
                    let Some(owner) = live.get(from).cloned() else {
                        return Err(Flaw::BadSublet { at });
                    };
                    let width = owner.len().max(region.len());
                    if !contains(&pad(owner, width), &pad(region.clone(), width)) {
                        return Err(Flaw::BadSublet { at });
                    }
                    for (sibling, parent) in &within {
                        if parent != from {
                            continue;
                        }
                        let Some(theirs) = live.get(sibling.as_str()) else {
                            continue;
                        };
                        let to_width = width.max(theirs.len());
                        if intersects(
                            &pad(theirs.clone(), to_width),
                            &pad(region.clone(), to_width),
                        ) {
                            return Err(Flaw::BadSublet { at });
                        }
                    }
                    within.insert(to.clone(), from.clone());
                    live.insert(to.clone(), region);
                    // No ground is added to `taken`: the owner's entry
                    // already covers it, and adding it again would make
                    // a later maturation over the parent's own claim
                    // read as a collision with itself.
                }
                Act::Cede { from, to, .. } => {
                    let Some(slab) = act.region(axes_now) else {
                        return Err(Flaw::TooManyAxes { at });
                    };
                    if from == to || live.contains_key(to) {
                        return Err(Flaw::BadCession { at });
                    }
                    let Some(owner) = live.get(from).cloned() else {
                        return Err(Flaw::BadCession { at });
                    };
                    // Pad the older estate to the slab's dimensionality
                    // before the slab test — an estate from before an
                    // axis pins to its zero slice.
                    let width = slab.len().max(owner.len());
                    match slab_remainder(&pad(owner, width), &pad(slab.clone(), width)) {
                        Some(Some(remainder)) => {
                            live.insert(from.clone(), remainder);
                        }
                        Some(None) => {
                            live.remove(from);
                        }
                        None => return Err(Flaw::BadCession { at }),
                    }
                    live.insert(to.clone(), slab);
                    // The ground was already in `taken` under the
                    // original issue; a conveyance adds no new ground.
                }
            }
        }
        Ok(())
    }

    /// The node's potential at a point: whose box it is in, and the
    /// per-axis offset **within** that box.
    ///
    /// **Local knowledge only.** A node computes this from its own
    /// deeds; it needs no other node's numbering and no shared ground
    /// truth. The offset is the gauge-invariant part of a coordinate —
    /// the box origin is the frame, and this subtracts the frame off.
    pub fn potential_at(&self, point: &[Tag]) -> Option<(String, Vec<Tag>)> {
        let deed = self.holder_at(point)?;
        let mut offsets = Vec::new();
        for (at, (low, _)) in point.iter().zip(deed.region.iter()) {
            offsets.push(at.checked_sub(*low)?);
        }
        (offsets.len() == point.len()).then_some((deed.holder, offsets))
    }

    /// Append an act that already happened somewhere else.
    ///
    /// The issuer's `issue()` chooses a run for a **new** attachment. A
    /// historical act arrives with its numbers already fixed — recording
    /// it is transcription, not issuance, and the authority holding the
    /// chain validates it after the fold rather than trusting this call.
    pub fn record(&mut self, act: Act) {
        self.acts.push(act);
    }

    /// First open run of at least `width`, as `(low, high)` covering
    /// exactly `width` tags.
    ///
    /// One arm, no special case: the run is closed and measured at the
    /// same point whatever its length. An earlier version tested the
    /// length only on the byte *after* a run opened, so a width of one
    /// fell through and needed a patch — which is the shape of a bug
    /// rather than the shape of a rule.
    fn first_run(&self, width: u128) -> Option<(Tag, Tag)> {
        for (low, high) in self.gaps() {
            let span = u128::from(high)
                .saturating_sub(u128::from(low))
                .saturating_add(1);
            if span >= width {
                let high = low.checked_add(Tag::try_from(width.saturating_sub(1)).ok()?)?;
                return Some((low, high));
            }
        }
        None
    }
}

/// Verify one transduction as a **cocycle condition, per edge**.
///
/// A crossing `p` on node `a` → `q` on node `b` is sound exactly when
/// both nodes' potentials agree: same holder, same per-axis offset
/// within the holder's box. Each side computes its half from **its own
/// deeds alone** — the comparison is the only thing exchanged, so two
/// strangers verify a crossing without trusting either's numbering.
///
/// ## Why per edge, and not around the loop
///
/// A loop law (`B(A(p)) = p`) checks the *total* around a cycle, and a
/// distortion that cancels — a mirror, any involution — leaves the
/// total clean while every crossing is wrong. Measured: the mirror
/// mutation passed the round trip and failed the anchored law.
///
/// The cocycle condition checks **each edge against the potentials**,
/// so cancellation has nowhere to happen: the mirrored crossing moves
/// the offset on its own edge and fails there, no loop required. And
/// when every edge of a cycle satisfies the condition, the loop
/// identity follows — **cycles are run in cocycles**: the loop law is
/// derived, never separately trusted.
///
/// The verifier is deliberately not the transducer. [`Ledger::translate_at`]
/// maps; this checks; a defect in the map cannot hide in a check that
/// re-derives nothing through it — the same separation `lith`'s verify
/// keeps from its prover's reduction.
pub fn cocycle(a: &Ledger, p: &[Tag], b: &Ledger, q: &[Tag]) -> bool {
    match (a.potential_at(p), b.potential_at(q)) {
        (Some(here), Some(there)) => here == there,
        _ => false,
    }
}

// ===================================================================
// THE CHAIN — acts as stored bytes.
//
// The issuer executes; the AUTHORITY is whoever holds the chain, and a
// chain that cannot be stored is not a chain. Acts serialize as TLV
// records so the authority's ground truth is the same `.tlv` everything
// else in the hierarchy grounds in:
//
//     .tlv -> kernel -> local mesh -> substrate -> local mesh | kernel
// ===================================================================

/// The chain's storage form.
pub mod chain {
    use super::Act;
    use crate::frame::{put_frame, Malformed, Reader};
    use crate::layout::Layout;

    /// Chain-record tag for an [`Act::Encumber`].
    pub const ENCUMBER: u64 = 1;
    /// Chain-record tag for an [`Act::Issue`].
    pub const ISSUE: u64 = 2;
    /// Chain-record tag for an [`Act::Retire`].
    pub const RETIRE: u64 = 3;
    /// Chain-record tag for an [`Act::Open`]. **Additive**: the minted
    /// founding chain carries tags 1–3 only and keeps decoding
    /// unchanged; a chain carrying 4–6 refuses on an old reader rather
    /// than misfolding, which is D1d doing its job across revisions.
    pub const OPEN: u64 = 4;
    /// Chain-record tag for an [`Act::EncumberBox`].
    pub const ENCUMBER_BOX: u64 = 5;
    /// Chain-record tag for an [`Act::IssueBox`].
    pub const ISSUE_BOX: u64 = 6;
    /// Chain-record tag for an [`Act::Cede`] — the purchase, on the
    /// record like everything else.
    pub const CEDE: u64 = 7;
    /// Chain-record tag for an [`Act::Sublet`] — the moon.
    pub const SUBLET: u64 = 9;
    /// Chain-record tag for an [`Act::Bind`] — the presenting key
    /// (`IS-6/4`). Additive: an older reader refuses tag 10 rather
    /// than misfolding, as with every act added since the founding.
    pub const BIND: u64 = 10;
    /// Chain-record tag for an [`Act::Declare`] — the published
    /// domain definition (`IS-6/5`). Additive, same rule.
    pub const DECLARE: u64 = 11;
    /// Chain-record tag for an [`Act::Anchor`] — **the vertical**.
    ///
    /// Tags 1–7 all move this chain's own fold. This one names another
    /// chain, and it is the first record in the format that does.
    pub const ANCHOR: u64 = 8;
    /// Chain-record tag for an [`Act::Certify`] — the transport
    /// certificate fingerprint (`IS-6/6`). Additive, same rule.
    pub const CERTIFY: u64 = 12;
    /// Chain-record tag for an [`Act::Escrow`] — a balance locked as a
    /// stake (`IS-6/7`). Additive: an older reader refuses tag 13 rather
    /// than misfolding, as with every act added since the founding.
    pub const ESCROW: u64 = 13;
    /// Chain-record tag for an [`Act::Release`] — a stake unlocked
    /// (`IS-6/7`). Additive, same rule.
    pub const RELEASE: u64 = 14;
    /// Chain-record tag for an [`Act::Slash`] — a stake destroyed for a
    /// verifiable offence (`IS-6/7`). Additive, same rule.
    pub const SLASH: u64 = 15;

    /// A length-framed opaque blob: `LE32(len) ‖ bytes`.
    ///
    /// Deliberately wider than the `u16` the text fields use. A digest
    /// is the one field here whose width is somebody *else's* choice,
    /// and a narrow length would be this crate putting a ceiling on a
    /// stranger's decision — which is the capacity-as-a-constant
    /// defect wearing a length prefix.
    fn put_blob(bytes: &[u8], out: &mut Vec<u8>) {
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(bytes.get(..len as usize).unwrap_or(bytes));
    }

    fn take_blob(reader: &mut Reader<'_>) -> Result<Vec<u8>, Malformed> {
        let len = reader.u32()? as usize;
        Ok(reader.take(len)?.to_vec())
    }

    fn put_region(region: &[(super::Tag, super::Tag)], out: &mut Vec<u8>) {
        let axes = u16::try_from(region.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&axes.to_le_bytes());
        for (low, high) in region.iter().take(usize::from(axes)) {
            out.extend_from_slice(&low.to_le_bytes());
            out.extend_from_slice(&high.to_le_bytes());
        }
    }

    fn take_region(reader: &mut Reader<'_>) -> Result<Vec<(super::Tag, super::Tag)>, Malformed> {
        let axes = usize::from(reader.u16()?);
        let mut region = Vec::new();
        for _ in 0..axes {
            let low = reader.u64()?;
            let high = reader.u64()?;
            region.push((low, high));
        }
        Ok(region)
    }

    fn put_text(text: &str, out: &mut Vec<u8>) {
        let bytes = text.as_bytes();
        let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(bytes.get(..usize::from(len)).unwrap_or(bytes));
    }

    fn take_text(reader: &mut Reader<'_>) -> Result<String, Malformed> {
        let len = usize::from(reader.u16()?);
        let bytes = reader.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Malformed::TrailingBytes { left: len })
    }

    /// Encode a sequence of acts, in order, under the founding layout.
    pub fn encode(acts: &[Act]) -> Vec<u8> {
        let layout = Layout::founding();
        let mut out = Vec::new();
        for act in acts {
            let mut value = Vec::new();
            let tag = match act {
                Act::Encumber {
                    low,
                    high,
                    by,
                    witnessed,
                } => {
                    value.extend_from_slice(&low.to_le_bytes());
                    value.extend_from_slice(&high.to_le_bytes());
                    put_text(by, &mut value);
                    put_text(witnessed, &mut value);
                    ENCUMBER
                }
                Act::Issue { holder, low, high } => {
                    put_text(holder, &mut value);
                    value.extend_from_slice(&low.to_le_bytes());
                    value.extend_from_slice(&high.to_le_bytes());
                    ISSUE
                }
                Act::Retire { holder } => {
                    put_text(holder, &mut value);
                    RETIRE
                }
                Act::Open { axis, max } => {
                    put_text(axis, &mut value);
                    value.extend_from_slice(&max.to_le_bytes());
                    OPEN
                }
                Act::EncumberBox {
                    region,
                    by,
                    witnessed,
                } => {
                    put_region(region, &mut value);
                    put_text(by, &mut value);
                    put_text(witnessed, &mut value);
                    ENCUMBER_BOX
                }
                Act::IssueBox { holder, region } => {
                    put_text(holder, &mut value);
                    put_region(region, &mut value);
                    ISSUE_BOX
                }
                Act::Cede { from, to, region } => {
                    put_text(from, &mut value);
                    put_text(to, &mut value);
                    put_region(region, &mut value);
                    CEDE
                }
                Act::Sublet { from, to, region } => {
                    put_text(from, &mut value);
                    put_text(to, &mut value);
                    put_region(region, &mut value);
                    SUBLET
                }
                Act::Anchor {
                    chain,
                    height,
                    digest,
                    witnessed,
                } => {
                    put_text(chain, &mut value);
                    value.extend_from_slice(&height.to_le_bytes());
                    put_blob(digest, &mut value);
                    put_text(witnessed, &mut value);
                    ANCHOR
                }
                Act::Bind {
                    holder,
                    scheme,
                    key,
                    from_epoch,
                    until_epoch,
                } => {
                    put_text(holder, &mut value);
                    value.push(*scheme);
                    put_blob(key, &mut value);
                    value.extend_from_slice(&from_epoch.to_le_bytes());
                    value.extend_from_slice(&until_epoch.to_le_bytes());
                    BIND
                }
                Act::Declare {
                    holder,
                    tag: declared,
                    definition,
                } => {
                    put_text(holder, &mut value);
                    value.extend_from_slice(&declared.to_le_bytes());
                    put_blob(definition, &mut value);
                    DECLARE
                }
                Act::Certify {
                    holder,
                    fingerprint,
                } => {
                    put_text(holder, &mut value);
                    value.extend_from_slice(fingerprint);
                    CERTIFY
                }
                Act::Escrow { holder, amount } => {
                    put_text(holder, &mut value);
                    value.extend_from_slice(&amount.to_le_bytes());
                    ESCROW
                }
                Act::Release { holder } => {
                    put_text(holder, &mut value);
                    RELEASE
                }
                Act::Slash { holder, amount } => {
                    put_text(holder, &mut value);
                    value.extend_from_slice(&amount.to_le_bytes());
                    SLASH
                }
            };
            // The founding layout holds tags 1..=3 and every value here
            // fits its length field; ignoring the Ok is the total path.
            let _ = put_frame(&layout, tag, &value, &mut out);
        }
        out
    }

    /// Decode a stored chain back to its acts.
    ///
    /// **An unknown act refuses. It does not skip.** On the mesh an
    /// unknown tag steps over whole, because a frame not yours is still
    /// someone's. In a chain every act moves the fold, so a reader that
    /// skipped one would fold a *different history* and report it as
    /// this one — the worst available outcome, delivered quietly.
    pub fn decode(bytes: &[u8]) -> Result<Vec<Act>, Malformed> {
        let layout = Layout::founding();
        let mut outer = Reader::new(bytes);
        let mut acts = Vec::new();

        while !outer.is_done() {
            let (tag, value) = outer.frame(&layout)?;
            let mut reader = Reader::new(value);
            let act = match tag {
                ENCUMBER => {
                    let low = reader.u64()?;
                    let high = reader.u64()?;
                    let by = take_text(&mut reader)?;
                    let witnessed = take_text(&mut reader)?;
                    Act::Encumber {
                        low,
                        high,
                        by,
                        witnessed,
                    }
                }
                ISSUE => {
                    let holder = take_text(&mut reader)?;
                    let low = reader.u64()?;
                    let high = reader.u64()?;
                    Act::Issue { holder, low, high }
                }
                RETIRE => Act::Retire {
                    holder: take_text(&mut reader)?,
                },
                OPEN => {
                    let axis = take_text(&mut reader)?;
                    let max = reader.u64()?;
                    Act::Open { axis, max }
                }
                ENCUMBER_BOX => {
                    let region = take_region(&mut reader)?;
                    let by = take_text(&mut reader)?;
                    let witnessed = take_text(&mut reader)?;
                    Act::EncumberBox {
                        region,
                        by,
                        witnessed,
                    }
                }
                ISSUE_BOX => {
                    let holder = take_text(&mut reader)?;
                    let region = take_region(&mut reader)?;
                    Act::IssueBox { holder, region }
                }
                CEDE => {
                    let from = take_text(&mut reader)?;
                    let to = take_text(&mut reader)?;
                    let region = take_region(&mut reader)?;
                    Act::Cede { from, to, region }
                }
                SUBLET => {
                    let from = take_text(&mut reader)?;
                    let to = take_text(&mut reader)?;
                    let region = take_region(&mut reader)?;
                    Act::Sublet { from, to, region }
                }
                ANCHOR => {
                    let chain = take_text(&mut reader)?;
                    let height = reader.u64()?;
                    let digest = take_blob(&mut reader)?;
                    let witnessed = take_text(&mut reader)?;
                    Act::Anchor {
                        chain,
                        height,
                        digest,
                        witnessed,
                    }
                }
                BIND => {
                    let holder = take_text(&mut reader)?;
                    let scheme = *reader
                        .take(1)?
                        .first()
                        .ok_or(Malformed::TrailingBytes { left: 0 })?;
                    let key = take_blob(&mut reader)?;
                    let from_epoch = reader.u64()?;
                    let until_epoch = reader.u64()?;
                    Act::Bind {
                        holder,
                        scheme,
                        key,
                        from_epoch,
                        until_epoch,
                    }
                }
                DECLARE => {
                    let holder = take_text(&mut reader)?;
                    let tag = reader.u64()?;
                    let definition = take_blob(&mut reader)?;
                    Act::Declare {
                        holder,
                        tag,
                        definition,
                    }
                }
                CERTIFY => {
                    let holder = take_text(&mut reader)?;
                    let bytes = reader.take(32)?;
                    let mut fingerprint = [0u8; 32];
                    fingerprint.copy_from_slice(bytes);
                    Act::Certify {
                        holder,
                        fingerprint,
                    }
                }
                ESCROW => {
                    let holder = take_text(&mut reader)?;
                    let amount = reader.u128()?;
                    Act::Escrow { holder, amount }
                }
                RELEASE => {
                    let holder = take_text(&mut reader)?;
                    Act::Release { holder }
                }
                SLASH => {
                    let holder = take_text(&mut reader)?;
                    let amount = reader.u128()?;
                    Act::Slash { holder, amount }
                }
                found => {
                    return Err(Malformed::UnexpectedTag {
                        expected: ENCUMBER,
                        found,
                    })
                }
            };
            reader.finish()?;
            acts.push(act);
        }
        Ok(acts)
    }
}
