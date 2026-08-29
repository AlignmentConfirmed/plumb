//! Telling *not yet arrived* from *never will*.
//!
//! `IS-2` §7. A stream has no message boundaries and a file does, so a
//! socket delivery can stop mid-record. The partial tail is held by the
//! session, and where a record ends is answered by the frame format —
//! the carrier still parses nothing.
//!
//! ## The defect this rule closes
//!
//! A reader that stops at the first record it cannot take, and keeps
//! the remainder, will sit on a header declaring more value than can
//! ever arrive. Every later feed re-parses from the same offset, fails
//! identically, and returns nothing. **The session stalls and reports
//! nothing — neither accepting nor refusing.**
//!
//! Both ancestors otherwise hold the line *refuse, never guess*.
//! Stalling is neither.
//!
//! ## The rule
//!
//! A header declares its own length, so the two cases are separable
//! from the header alone:
//!
//! ```text
//! len > bound            REFUSE   no arrival can satisfy this
//! buffer < 5             WAIT     the header is incomplete
//! buffer < 5 + len       WAIT     the value is incomplete
//! otherwise              TAKE
//! ```
//!
//! **The bound is what makes the first line decidable.** Without one,
//! *unsatisfiable* and *not yet arrived* are the same observation, and a
//! reader can only wait.
//!
//! ## And the buffer is then bounded
//!
//! An overlong header refuses **at the header**, before any value is
//! held. So a session's held bytes never exceed one maximal record —
//! which was the other half of the defect, and it falls out rather than
//! needing its own rule.

use crate::layout::Layout;

// `MAX_RECORD: usize = 1 << 20` was here, and it did not belong in a
// protocol crate at all.
//
// It was a measurement of one deployment's corpus -- the largest record
// observed was 585 bytes -- baked into the crate every integrator
// imports. A title with larger records than that inherited the ceiling,
// and the only way to raise it was to edit this line.
//
// The protocol says a bound EXISTS and is declared. What the number is
// belongs to the deployment that measured it. `datum` supplies its own
// from `measure/record-bound.md`, and a peer declares its own in the
// opening declaration.
//
// A peer may declare a LARGER bound than its neighbour. It may not
// enforce a smaller one silently: a reader refusing at a ceiling its
// sender does not know about loses the record with nobody at fault.

/// What a reader must do with the bytes it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A whole record is available, this many bytes long including its
    /// header.
    Take(usize),
    /// A record has begun and not finished. Hold, and neither accept nor
    /// refuse.
    Wait,
    /// The head record can never be satisfied. Refuse, and say why.
    Refuse(Unsatisfiable),
}

/// Why a record at the head of the buffer will never complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unsatisfiable {
    /// The declared length exceeds the bound in force.
    Overlong {
        /// The length the header declared.
        declared: usize,
        /// The bound in force — from a peer's declaration, or the
        /// caller's own fallback.
        ///
        /// This said "or `MAX_RECORD`", naming a constant deleted from
        /// this module: a number one deployment measured does not
        /// belong in the crate every deployment imports. See
        /// [`crate::hello::Hello::bound_for`], which takes the
        /// fallback in.
        bound: usize,
    },
}

/// Read the head of a buffer under the rule.
///
/// Pure and total, and it decides from the header alone — so it costs
/// the same whatever the buffer holds. A reader cannot be made to do
/// work by sending it a large declared length; that is the refusal, not
/// the work.
pub fn step(layout: &Layout, bytes: &[u8], bound: usize) -> Step {
    let header = layout.header();
    // The header is not complete, so nothing about it can be read. The
    // offsets come from the layout rather than from a `1..5` written
    // here, which is the hand answer a scalar HEADER used to force.
    if bytes.len() < header {
        return Step::Wait;
    }
    let Some(declared) = layout.take_length(bytes) else {
        return Step::Wait;
    };

    if declared > bound {
        return Step::Refuse(Unsatisfiable::Overlong { declared, bound });
    }

    match header.checked_add(declared) {
        None => Step::Refuse(Unsatisfiable::Overlong { declared, bound }),
        Some(whole) if bytes.len() < whole => Step::Wait,
        Some(whole) => Step::Take(whole),
    }
}

/// How many bytes of whole records are available, and whether what
/// remains is a wait or a refusal.
///
/// The third answer is the point. A reader that returns only *how many
/// bytes are ready* cannot distinguish a buffer that is waiting from one
/// that is stuck.
pub fn whole_records(layout: &Layout, bytes: &[u8], bound: usize) -> (usize, Step) {
    let mut consumed = 0usize;
    loop {
        let Some(rest) = bytes.get(consumed..) else {
            return (consumed, Step::Wait);
        };
        if rest.is_empty() {
            return (consumed, Step::Wait);
        }
        match step(layout, rest, bound) {
            Step::Take(whole) => consumed = consumed.saturating_add(whole),
            other => return (consumed, other),
        }
    }
}

/// The most a conforming session ever holds: one maximal record.
///
/// A reading of the layout plus the bound, so an edge with a wider tag
/// field holds correspondingly more — which a scalar header could not
/// have expressed.
pub fn max_held(layout: &Layout, bound: usize) -> usize {
    layout.header().saturating_add(bound)
}

// ===================================================================
// THE SESSION — the stateful half of the rule above.
//
// `step` is a pure function and until now nothing drove it: every
// buffer it ever judged was built whole by the test asserting on it.
// A stream does not arrive whole. It arrives in chunks cut wherever
// the carrier felt like cutting, and the thing that survives that is
// this fold.
// ===================================================================

use crate::layout::Tag;

/// One whole record, off the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The tag, read in the edge's layout.
    pub tag: Tag,
    /// The value, exactly as it arrived. The session does not read it.
    pub value: Vec<u8>,
}

/// What a session knows after a feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum After {
    /// Whatever remains held is an incomplete record. More may arrive.
    Waiting,
    /// The head record can never complete. The edge is dead — see
    /// [`Session::feed`] for why this is terminal.
    Refused(Unsatisfiable),
}

/// Everything one feed produced.
///
/// Records and status **together**, because a refusal can arrive midway
/// through a chunk that already completed records — and an API that
/// returns `Err` there silently drops what was delivered before the
/// poison. The records a peer sent whole are theirs to have read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// Whole records completed by this feed, in arrival order.
    pub records: Vec<Record>,
    /// What the session is doing now.
    pub after: After,
}

/// One edge's receive state: a buffer, a bound, and the fold.
///
/// Feed it chunks of any size — one byte, a megabyte, a chunk that ends
/// mid-header — and it emits whole records and holds the tail. The
/// chunking is the carrier's accident; the records are the sender's
/// intent; this type is where the accident is removed.
#[derive(Debug, Clone)]
pub struct Session {
    layout: Layout,
    bound: usize,
    held: Vec<u8>,
    taken: u64,
    dead: Option<Unsatisfiable>,
}

impl Session {
    /// A session on an edge speaking `layout`, refusing any record
    /// declaring more than `bound` bytes of value.
    ///
    /// The bound is the caller's — from the peer's declaration or the
    /// deployment's own measurement. There is no default here for the
    /// same reason there is no `MAX_RECORD` any more: a number measured
    /// on one deployment's corpus is not the protocol's.
    pub fn new(layout: Layout, bound: usize) -> Self {
        Self {
            layout,
            bound,
            held: Vec::new(),
            taken: 0,
            dead: None,
        }
    }

    /// Feed a chunk. Returns every record it completed and the state
    /// after.
    ///
    /// ## Refusal is terminal, and here is why
    ///
    /// An unsatisfiable header could only be stepped over by trusting
    /// its declared length — the very claim just refused. There is no
    /// resynchronisation point in a length-prefixed stream that does not
    /// come from a length, so after one bad header every later byte is
    /// unframed noise. The session says so once and repeats it; it does
    /// not guess.
    ///
    /// Records completed **before** the poison are still delivered.
    /// They were whole and well-formed; the refusal is about what
    /// follows them, not about them.
    pub fn feed(&mut self, chunk: &[u8]) -> Delivery {
        if let Some(why) = self.dead {
            return Delivery {
                records: Vec::new(),
                after: After::Refused(why),
            };
        }

        self.held.extend_from_slice(chunk);
        let mut records = Vec::new();

        loop {
            match step(&self.layout, &self.held, self.bound) {
                Step::Take(whole) => {
                    let header = self.layout.header();
                    let Some(tag) = self.layout.take_tag(&self.held) else {
                        // step returned Take, so the header is present;
                        // answered rather than asserted because this
                        // crate does not panic on any input.
                        break;
                    };
                    let value = self
                        .held
                        .get(header..whole)
                        .map(<[u8]>::to_vec)
                        .unwrap_or_default();
                    self.held.drain(..whole.min(self.held.len()));
                    self.taken = self.taken.saturating_add(1);
                    records.push(Record { tag, value });
                }
                Step::Wait => {
                    return Delivery {
                        records,
                        after: After::Waiting,
                    };
                }
                Step::Refuse(why) => {
                    self.dead = Some(why);
                    // Nothing after a bad header can ever be framed, so
                    // holding it would hold garbage at full bound
                    // forever.
                    self.held.clear();
                    return Delivery {
                        records,
                        after: After::Refused(why),
                    };
                }
            }
        }

        Delivery {
            records,
            after: After::Waiting,
        }
    }

    /// Bytes held: the tail of an incomplete record.
    ///
    /// Never exceeds [`max_held`] once `feed` returns — an overlong
    /// header refuses at the header, before any value is buffered.
    pub fn pending(&self) -> usize {
        self.held.len()
    }

    /// Whole records emitted over this session's lifetime.
    ///
    /// This is the position the bootstrap rule reads:
    /// `hello::expects_declaration(session.records_read())` — the first
    /// record on an edge is the declaration, by position and not by a
    /// reserved number.
    pub fn records_read(&self) -> u64 {
        self.taken
    }

    /// The refusal that killed this edge, if one has.
    pub fn refused(&self) -> Option<Unsatisfiable> {
        self.dead
    }
}
