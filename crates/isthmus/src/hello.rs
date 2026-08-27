//! The declaration. `IS-5`.
//!
//! ```text
//! hello = LE32(revisions) ‖ (LE16(len) ‖ utf8)…
//!       ‖ LE32(ranges)    ‖ (LE32(low) ‖ LE32(high))…
//!       ‖ LE32(max_record)
//!       ‖ [uplink]                              IS-5/2, optional
//!
//! uplink = LE16(len) ‖ utf8                     the sender's chain
//!        ‖ LE32(len) ‖ digest
//!        ‖ LE32(count) ‖ (LE16(len) ‖ utf8 ‖ LE64(height))…
//! ```
//!
//! Tag 64, the first value `IS-3` grants this crate.
//!
//! ## A declaration, not a negotiation
//!
//! Each side states what it holds. Neither agrees to anything, there is
//! no round trip, and **there is no failure to agree** — which is the
//! whole reason it is shaped this way. A negotiation has a state where
//! both peers are waiting for the other to concede, and that state is
//! indistinguishable from a stall.
//!
//! A peer that speaks less is **limited, never refused**. It forwards
//! what it cannot read, which is exactly what a linking mesh does.

use crate::frame::{Malformed, Reader};
use crate::sphere::Frontier;
use crate::layout::Tag;

/// **Who this peer is, as a chain, and what it has seen.**
///
/// A [`crate::deed::Ledger`]'s name is not in its stored bytes — it is
/// context the acts are read in, like the layout. So a party reading
/// `founding.tlv` off disk learns the history and not who kept it, and
/// an [`Act::Anchor`](crate::deed::Act::Anchor) that names a chain has
/// nothing to bind the name to. This is where the name gets said.
///
/// Without it the uplink is one-directional: a peer can attach, be
/// deeded, and have its frames recognised — all downstream — and no
/// chain can record having observed it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Uplink {
    /// What the sender's chain calls itself.
    pub chain: String,
    /// The sender's own chain, at its own height, digested.
    ///
    /// Which function produced it is the edge's business — see
    /// [`crate::sphere::confirms`], which takes the function in. The
    /// height it is a digest *of* is `frontier.height_of(chain)`, so
    /// there is one place a height is stated and no pair to disagree.
    pub digest: Vec<u8>,
    /// Everything the sender has observed, its own chain included.
    pub frontier: Frontier,
}

impl Uplink {
    /// What a ledger declares about itself, if it is anybody.
    ///
    /// `None` for an unnamed chain — which is not a failure, it is the
    /// downstream-only peer saying nothing rather than saying it is
    /// nobody. The digest function comes in because this crate names
    /// none; see [`crate::sphere::confirms`].
    pub fn of(ledger: &crate::deed::Ledger, digest: impl Fn(&[u8]) -> Vec<u8>) -> Option<Self> {
        Some(Self {
            chain: ledger.name()?.to_owned(),
            digest: digest(&crate::deed::chain::encode(ledger.acts())),
            frontier: ledger.frontier(),
        })
    }

    /// The sender's own height, per its frontier.
    pub fn height(&self) -> u64 {
        self.frontier.height_of(&self.chain)
    }

    /// The vertical this declaration justifies: **one anchor, over the
    /// sender's own chain, and nothing else.**
    ///
    /// The frontier names other chains too, and those are *hearsay* —
    /// the sender's anchors, not this receiver's observations. Turning
    /// them into acts on this chain would launder provenance: it would
    /// record "I observed chain X" on the strength of somebody saying
    /// they did, and [`Act::Encumber`](crate::deed::Act::Encumber)
    /// exists in the shape it does precisely so that never happens.
    ///
    /// The frontier is still worth carrying — it orders the two peers
    /// against each other, which is what
    /// [`Frontier::compare`](crate::sphere::Frontier::compare) is for.
    /// Observation and ordering are different powers.
    pub fn anchor(&self, witnessed: &str) -> crate::deed::Act {
        crate::deed::Act::Anchor {
            chain: self.chain.clone(),
            height: self.height(),
            digest: self.digest.clone(),
            witnessed: witnessed.to_owned(),
        }
    }
}

// `HELLO_TAG: u8 = 64` was here, and it was the bootstrap problem
// answered by pretending it did not exist.
//
// If every tag is per-edge and issued by deed, there is no number the
// first frame can arrive on -- no deed exists yet, so no numbering does.
// Reserving 64 globally was carving one tag out of every edge forever so
// that the negotiation could refer to itself.
//
// THE FIRST RECORD ON AN EDGE IS THE DECLARATION.
//
// Position, not number. It is structural, costs no tag on any edge, and
// composes with the deed model: after the declaration every frame is
// deeded, including any later re-declaration, which travels under the
// deed the first one established.
//
// A first record that does not decode as a declaration is refused. There
// is nothing to misread, because nothing else is admissible there.

/// `LE16(len) ‖ utf8` — the same string framing the revision list uses.
fn put_text(text: &str, out: &mut Vec<u8>) {
    let bytes = text.as_bytes();
    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes.get(..usize::from(len)).unwrap_or(bytes));
}

fn take_text(reader: &mut Reader<'_>) -> Result<String, Malformed> {
    let len = usize::from(reader.u16()?);
    let bytes = reader.take(len)?;
    // Not utf8 is not a name. Refuse rather than lossy-convert: a
    // mangled chain name would compare unequal to every real one and
    // silently read as a stranger.
    String::from_utf8(bytes.to_vec()).map_err(|_| Malformed::TrailingBytes { left: len })
}

/// Whether a record is the one that must be a declaration.
///
/// The whole of the bootstrap rule. An edge that has read nothing yet
/// expects a declaration; an edge that has read anything does not.
pub fn expects_declaration(records_read: usize) -> bool {
    records_read == 0
}

/// What a peer states about itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hello {
    /// Protocol documents this peer implements, as revision strings.
    ///
    /// **Compared for equality and never ordered.** There is no
    /// newer-than: ordering would let a peer decide it is ahead and act
    /// on the difference, which is authority this substrate does not
    /// have.
    pub revisions: Vec<String>,
    /// Deeds held on this edge. Inclusive on both ends.
    ///
    /// `Tag`-wide, not `u32`, and **not validated against 255**. An
    /// earlier version refused any range above 255 — the one-byte tag
    /// space asserted in a third place, where it would have refused a
    /// perfectly good declaration from a peer on a wider layout.
    pub ranges: Vec<(Tag, Tag)>,
    /// The largest record value this peer will accept.
    pub max_record: u32,
    /// **Who this peer is as a chain**, if it is anybody. `IS-5/2`.
    ///
    /// `None` is the pre-`IS-5/2` declaration, byte for byte: an
    /// unnamed chain emits nothing here, so a peer that has not opted
    /// into being addressable is indistinguishable on the wire from
    /// one that predates the field.
    ///
    /// That is what makes the extension safe to add to a live wire. An
    /// `IS-5/1` reader refuses trailing bytes — correctly; a truncated
    /// or over-long declaration is not this declaration — so an
    /// `IS-5/2` declaration *would* be refused by an old peer. But it
    /// is only ever sent by a chain that took a name, and taking a name
    /// is a deliberate act by the party that wants upstream. The
    /// incompatibility is opt-in, by the side that chose it, and
    /// visible in the revision strings either way.
    ///
    /// ## The one downgrade, stated
    ///
    /// Being optional **and last**, this field can be cut off: a
    /// declaration truncated at the byte where the uplink block begins
    /// decodes as a complete, valid declaration by an anonymous peer.
    /// Every other cut refuses. No arrangement of an optional trailing
    /// field avoids this, and `l7` pins down what it costs — the only
    /// surviving cut is that one, and what survives is strictly less:
    /// never a *different* uplink, only an absent one.
    ///
    /// It is not a framing hazard, because the value is length-framed
    /// by the record around it; a party that can shorten the value can
    /// rewrite it entirely, and no codec defends against that. It
    /// **is** a downgrade an active party can perform, turning an
    /// addressable peer into an anonymous one. The observable
    /// consequence is that [`Hello::against`] answers `None`, which is
    /// exactly why that answer is kept distinct from *concurrent*: a
    /// downgraded peer looks unaddressable, and unaddressable is a
    /// state worth being able to notice.
    pub uplink: Option<Uplink>,
}

impl Hello {
    /// What this build declares, given the deeds it holds on this edge
    /// and the bound its deployment measured.
    ///
    /// **Both are arguments.** An earlier version was `of_isthmus()`
    /// with the range and the bound written in, which declared the same
    /// thing on every edge and made a per-edge protocol behave like a
    /// global one.
    /// **Declares no uplink.** Downstream only, and byte-identical to
    /// `IS-5/1` — pair it with [`Hello::declaring`] to go upstream.
    ///
    /// The opt-in is at the call site on purpose. An `of()` that
    /// emitted the uplink block whenever the ledger happened to have a
    /// name would make a peer incompatible with `IS-5/1` readers as a
    /// side effect of naming its chain, which is a decision the caller
    /// should have to write down.
    pub fn of(ledger: &crate::deed::Ledger, holder: &str, bound: u32) -> Self {
        Self {
            revisions: crate::revisions(),
            ranges: ledger
                .deeds()
                .iter()
                .filter(|d| d.live && d.holder == holder)
                .map(|d| (d.low(), d.high()))
                .collect(),
            max_record: bound,
            uplink: None,
        }
    }

    /// Say who this chain is: **the upstream half**.
    ///
    /// Takes the `Option` [`Uplink::of`] returns, so an unnamed chain
    /// flows through unchanged rather than needing a branch at every
    /// call site.
    #[must_use]
    pub fn declaring(mut self, uplink: Option<Uplink>) -> Self {
        self.uplink = uplink;
        self
    }

    /// Write the declaration's *value*. Wrap it with
    /// [`crate::frame::put_frame`].
    ///
    /// **Under no particular tag.** This said "under `HELLO_TAG`",
    /// naming a constant deleted from this module for the reason its
    /// own comment gives: reserving 64 globally carved one tag out of
    /// every edge forever so the negotiation could refer to itself.
    /// The first record on an edge *is* the declaration — position,
    /// not number. See [`expects_declaration`].
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(u32::try_from(self.revisions.len()).unwrap_or(u32::MAX)).to_le_bytes());
        for revision in &self.revisions {
            let bytes = revision.as_bytes();
            out.extend_from_slice(&(u16::try_from(bytes.len()).unwrap_or(u16::MAX)).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        out.extend_from_slice(&(u32::try_from(self.ranges.len()).unwrap_or(u32::MAX)).to_le_bytes());
        for (low, high) in &self.ranges {
            out.extend_from_slice(&low.to_le_bytes());
            out.extend_from_slice(&high.to_le_bytes());
        }
        out.extend_from_slice(&self.max_record.to_le_bytes());

        // Absent, not empty. A named chain with an empty frontier is a
        // real declaration; a chain with no name emits no bytes at all,
        // and the two must not encode the same.
        if let Some(uplink) = &self.uplink {
            put_text(&uplink.chain, &mut out);
            out.extend_from_slice(
                &(u32::try_from(uplink.digest.len()).unwrap_or(u32::MAX)).to_le_bytes(),
            );
            out.extend_from_slice(&uplink.digest);
            let chains = uplink.frontier.chains();
            out.extend_from_slice(&(u32::try_from(chains.len()).unwrap_or(u32::MAX)).to_le_bytes());
            for name in chains {
                put_text(name, &mut out);
                out.extend_from_slice(&uplink.frontier.height_of(name).to_le_bytes());
            }
        }
        out
    }

    /// Read a declaration from a frame's value.
    ///
    /// **Refuses rather than guessing.** A truncated declaration is not
    /// a partial one: a peer acting on half a declaration acts on terms
    /// the sender did not state.
    pub fn decode(value: &[u8]) -> Result<Self, Malformed> {
        let mut reader = Reader::new(value);

        let count = reader.u32()? as usize;
        let mut revisions = Vec::new();
        for _ in 0..count {
            let len = reader.u16()? as usize;
            let bytes = reader.take(len)?;
            // Not utf8 is not a revision string. Refuse rather than
            // lossy-convert: a mangled revision compares unequal to
            // every real one and reads as a peer speaking nothing.
            let text = String::from_utf8(bytes.to_vec())
                .map_err(|_| Malformed::TrailingBytes { left: len })?;
            revisions.push(text);
        }

        let range_count = reader.u32()? as usize;
        let mut ranges = Vec::new();
        for _ in 0..range_count {
            let low = reader.u64()?;
            let high = reader.u64()?;
            // Inverted is not a range. There is deliberately NO ceiling
            // check here: an earlier version refused any range above
            // 255, which asserted the one-byte tag space in a third
            // place and would have refused a good declaration from a
            // peer on a wider layout.
            if low > high {
                return Err(Malformed::UnexpectedTag {
                    expected: low,
                    found: high,
                });
            }
            ranges.push((low, high));
        }

        let max_record = reader.u32()?;

        // `IS-5/2`. Nothing left is a declaration from a chain with no
        // name — not a truncated one, because the field it would have
        // carried is the last thing in the record and every field
        // before it was read whole.
        let uplink = if reader.is_done() {
            None
        } else {
            let chain = take_text(&mut reader)?;
            let len = reader.u32()? as usize;
            let digest = reader.take(len)?.to_vec();
            let count = reader.u32()? as usize;
            let mut frontier = Frontier::new();
            for _ in 0..count {
                let name = take_text(&mut reader)?;
                let height = reader.u64()?;
                frontier.observe(&name, height);
            }
            Some(Uplink {
                chain,
                digest,
                frontier,
            })
        };

        // A declaration with bytes left over is not this declaration.
        reader.finish()?;

        Ok(Self {
            revisions,
            ranges,
            max_record,
            uplink,
        })
    }

    /// The bound to send under, given what a peer declared.
    ///
    /// `fallback` is the caller's own — there is no crate-wide default,
    /// because a default here would be one deployment's measurement
    /// imposed on every other.
    pub fn bound_for(declared: Option<&Hello>, fallback: usize) -> usize {
        match declared {
            None => fallback,
            Some(hello) => hello.max_record as usize,
        }
    }

    /// Whether this peer claims to read a tag.
    pub fn reads(&self, tag: Tag) -> bool {
        self.ranges
            .iter()
            .any(|(low, high)| tag >= *low && tag <= *high)
    }

    /// How this peer stands against another in the causal order.
    ///
    /// `None` when either side declared no uplink — an unnamed chain
    /// has no position in the order, which is a different thing from
    /// being concurrent with everybody. [`Frontier::compare`] answering
    /// `None` means *concurrent*; this answering `None` means *not
    /// comparable at all, because one of you is anonymous*, and
    /// collapsing the two would report an unaddressable peer as a
    /// simultaneous one.
    pub fn against(&self, other: &Hello) -> Option<Option<std::cmp::Ordering>> {
        let (here, there) = (self.uplink.as_ref()?, other.uplink.as_ref()?);
        Some(here.frontier.compare(&there.frontier))
    }

    /// Revision strings both peers hold.
    ///
    /// Equality only. The result may be empty, and an empty result is
    /// **not** an error: two peers sharing no revision still exchange
    /// frames, they just each forward what the other owns.
    pub fn shared_revisions(&self, other: &Hello) -> Vec<String> {
        self.revisions
            .iter()
            .filter(|r| other.revisions.contains(r))
            .cloned()
            .collect()
    }
}
