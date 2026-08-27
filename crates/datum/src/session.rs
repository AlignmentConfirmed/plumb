//! THE SESSION RULE — telling *not yet arrived* from *never will*.
//!
//! `IS-2` §7 recorded a defect: `whole_records` stops at the first
//! record it cannot take and `feed` keeps the remainder, so a header
//! declaring more value than can ever arrive sits at the head of the
//! buffer forever. Every later feed re-parses from the same offset,
//! fails identically, and returns nothing. **The session stalls and
//! reports nothing — neither accepting nor refusing.**
//!
//! Both projects otherwise hold the line *refuse, never guess*. Stalling
//! is neither.
//!
//! ## The rule
//!
//! A frame header declares its own length, so the two cases are
//! separable from the header alone:
//!
//! ```text
//! len > MAX_RECORD           REFUSE   no arrival can satisfy this
//! buffer < 5                 WAIT     the header is incomplete
//! buffer < 5 + len           WAIT     the value is incomplete
//! otherwise                  TAKE
//! ```
//!
//! The bound is what makes the first line decidable. Without one,
//! *unsatisfiable* and *not yet arrived* are the same observation, and a
//! reader can only wait.
//!
//! ## Why the buffer is then bounded
//!
//! A header declaring more than `MAX_RECORD` refuses **at the header**,
//! before any value is held. So a session's held bytes never exceed one
//! maximal record, and `pending()` cannot grow without bound — which was
//! the other half of the defect.

/// The largest value a record may declare, in bytes.
///
/// **Measured, not picked.** The largest record across 4127 stored
/// netstratum chronicle records is 585 bytes of value; this clears it by
/// a factor of roughly 1790. See `measure/record-bound.md`.
///
/// A peer may agree a larger bound in the handshake. It may not agree a
/// smaller one silently: a reader refusing at a bound its sender does
/// not know about is the silent-misread failure one level up.
pub const MAX_RECORD: usize = 1 << 20;

/// The frame header: a tag byte and a little-endian 32-bit length.
pub const HEADER: usize = 5;

/// What a reader must do with the bytes it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A whole record is available, this many bytes long including its
    /// header.
    Take(usize),
    /// A record has begun and not finished. Hold, and neither accept nor
    /// refuse.
    Wait,
    /// The head record can never be satisfied. Refuse and say why.
    Refuse(Unsatisfiable),
}

/// Why a record at the head of the buffer will never complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsatisfiable {
    /// The declared length exceeds [`MAX_RECORD`].
    Overlong { declared: usize, bound: usize },
}

/// Read the head of a buffer under the rule.
///
/// Pure and total. Decides from the header alone, so it costs the same
/// whatever the buffer holds.
pub fn step(bytes: &[u8], bound: usize) -> Step {
    let Some(len_bytes) = bytes.get(1..HEADER) else {
        return Step::Wait;
    };
    let Ok(array) = <[u8; 4]>::try_from(len_bytes) else {
        return Step::Wait;
    };
    let declared = u32::from_le_bytes(array) as usize;

    if declared > bound {
        return Step::Refuse(Unsatisfiable::Overlong {
            declared,
            bound,
        });
    }

    match HEADER.checked_add(declared) {
        None => Step::Refuse(Unsatisfiable::Overlong { declared, bound }),
        Some(whole) if bytes.len() < whole => Step::Wait,
        Some(whole) => Step::Take(whole),
    }
}

/// How many bytes of whole records are available, and whether what
/// remains is a wait or a refusal.
///
/// This is `whole_records` with the third answer it was missing.
pub fn whole_records(bytes: &[u8], bound: usize) -> (usize, Step) {
    let mut consumed = 0usize;
    loop {
        let Some(rest) = bytes.get(consumed..) else {
            return (consumed, Step::Wait);
        };
        if rest.is_empty() {
            return (consumed, Step::Wait);
        }
        match step(rest, bound) {
            Step::Take(whole) => consumed = consumed.saturating_add(whole),
            other => return (consumed, other),
        }
    }
}

/// The most a conforming session ever holds: one maximal record.
pub fn max_held(bound: usize) -> usize {
    HEADER.saturating_add(bound)
}

// ===================================================================
// THE HELLO — IS-5, a declaration and not a negotiation
// ===================================================================

/// Tag 64, the first value `IS-3` §5 grants `isthmus`.
pub const HELLO_TAG: u8 = 64;

/// What a peer states about itself. It agrees to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    /// Protocol documents this peer implements. Compared for equality
    /// and never ordered — there is no newer-than, because ordering
    /// would let a peer decide it is ahead and act on the difference.
    pub revisions: Vec<String>,
    /// Registry grants held, from `IS-3` §5. Inclusive.
    pub ranges: Vec<(u32, u32)>,
    /// The largest record value this peer will accept.
    pub max_record: u32,
}

impl Hello {
    /// `LE32(revisions) ‖ (LE16(len) ‖ utf8)… ‖ LE32(ranges) ‖
    /// (LE32(low) ‖ LE32(high))… ‖ LE32(max_record)`
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.revisions.len() as u32).to_le_bytes());
        for revision in &self.revisions {
            let bytes = revision.as_bytes();
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        out.extend_from_slice(&(self.ranges.len() as u32).to_le_bytes());
        for (low, high) in &self.ranges {
            out.extend_from_slice(&low.to_le_bytes());
            out.extend_from_slice(&high.to_le_bytes());
        }
        out.extend_from_slice(&self.max_record.to_le_bytes());
        out
    }

    /// Read a hello from a frame's *value*.
    ///
    /// Refuses rather than guessing: a truncated declaration is not a
    /// partial one, because a peer acting on half a declaration would
    /// act on terms the sender did not state.
    pub fn decode(value: &[u8]) -> Option<Self> {
        let le32 = |at: usize| -> Option<u32> {
            let slice = value.get(at..at + 4)?;
            Some(u32::from_le_bytes(<[u8; 4]>::try_from(slice).ok()?))
        };

        let count = le32(0)? as usize;
        let mut at = 4usize;
        let mut revisions = Vec::with_capacity(count);
        for _ in 0..count {
            let len_bytes = value.get(at..at + 2)?;
            let len = u16::from_le_bytes(<[u8; 2]>::try_from(len_bytes).ok()?) as usize;
            at += 2;
            let text = value.get(at..at + len)?;
            revisions.push(String::from_utf8(text.to_vec()).ok()?);
            at += len;
        }

        let range_count = le32(at)? as usize;
        at += 4;
        let mut ranges = Vec::with_capacity(range_count);
        for _ in 0..range_count {
            let low = le32(at)?;
            let high = le32(at + 4)?;
            at += 8;
            if low > high || high > 255 {
                return None;
            }
            ranges.push((low, high));
        }

        let max_record = le32(at)?;
        at += 4;
        // A declaration with bytes left over is not this declaration.
        (at == value.len()).then_some(Self {
            revisions,
            ranges,
            max_record,
        })
    }

    /// The bound to send under, given what a peer declared.
    ///
    /// `None` means no declaration was heard, and the default applies.
    /// A peer may not enforce a bound **smaller** than the default
    /// without having declared it — a reader refusing at a ceiling its
    /// sender does not know about is the silent-misread failure one
    /// level up.
    pub fn bound_for(declared: Option<&Hello>) -> usize {
        match declared {
            None => MAX_RECORD,
            Some(hello) => hello.max_record as usize,
        }
    }

    /// Whether this peer claims to read a tag.
    ///
    /// A peer that does not is **limited, never refused** — it forwards
    /// what it does not own, which is what makes a mesh of meshes
    /// possible.
    pub fn reads(&self, tag: u8) -> bool {
        self.ranges
            .iter()
            .any(|(low, high)| u32::from(tag) >= *low && u32::from(tag) <= *high)
    }
}
