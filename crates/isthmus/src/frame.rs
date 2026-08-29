//! The record, and every refusal a conforming reader may produce.
//!
//! ```text
//! tag u8 ‖ LE32(length) ‖ value
//! ```
//!
//! `IS-1` §1. That is the whole framing, and the length prefix is
//! load-bearing: it is what lets a carrier move a frame it cannot read.
//!
//! > *"The length prefix is what makes this possible without
//! > understanding the value."*
//!
//! Measured byte-identical in both ancestors before it was written down
//! here.

use crate::layout::{self, Layout, Tag};
use crate::session::Unsatisfiable;

// `HEADER` was a const here. Twice.
//
// First `= 5`, then `= size_of::<u8>() + size_of::<u32>()`, which was
// worse for being plausible. Both defects survived the rewrite:
//
//   size_of::<u8>() PINS THE TAG AT ONE BYTE. 256 values, permanently,
//   in the crate that had just removed a capacity of six attachments
//   and a capacity of ten holders. Same shape, one level down: a limit
//   expressed as a type, so raising it means editing the type wherever
//   it appears.
//
//   THE SUM COLLAPSES THE HEADER TO A SCALAR. `5` is all that survives.
//   It cannot say where the length begins, how wide the tag is, or what
//   the fields are called -- so every one of those got answered by hand
//   somewhere else, a `1` here and a `1..5` there, and those hand
//   answers were the thing that had to stay in step.
//
// Adding widths is a READING of a layout, not the layout.
//
// `crate::layout::Layout` is the structure. `Layout::header()` folds the
// fields and is one reading among several -- `offset_of`, `width_of`,
// `tag_space`, `max_tag` are the ones a scalar could not give.

/// Why a reader refused.
///
/// **Every refusal `IS-1` §4 permits is a variant here**, and no
/// variant exists that §4 does not permit. An implementer who handles
/// only some of these has a reader that accepts what this one refuses,
/// which is the failure the table exists to prevent.
///
/// Refusals are *named*, never repaired. A decoder that silently fixes
/// a non-canonical input makes two byte strings mean one value, and the
/// address of a value is taken over its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Malformed {
    /// A fixed-width field ran off the end of the buffer.
    Truncated {
        /// Bytes the field needed.
        want: usize,
        /// Bytes that remained.
        have: usize,
    },
    /// A declared length exceeds what the record holds. `IS-2` §7.4.
    LengthExceedsRecord {
        /// The length the header declared.
        declared: usize,
        /// The bytes actually available to it.
        available: usize,
    },
    /// A declared length exceeds the session bound, so no arrival can
    /// ever satisfy it. This is the refusal that separates *never* from
    /// *not yet*; without it a reader can only wait.
    Overlong {
        /// The length the header declared.
        declared: usize,
        /// The bound in force, from the handshake or the default.
        bound: usize,
    },
    /// Bytes were left over after a value was read.
    ///
    /// Not pedantry: trailing bytes mean the reader and the writer
    /// disagree about the layout, and the reader got lucky about where
    /// the fields landed.
    TrailingBytes {
        /// How many bytes remained unread.
        left: usize,
    },
    /// A nested record carried a tag the enclosing layout does not
    /// permit there.
    UnexpectedTag {
        /// The tag the layout requires.
        expected: Tag,
        /// The tag that arrived.
        found: Tag,
    },
    /// The sign byte of an exact rational was neither 0 nor 1.
    SignByte(u8),
    /// A tag does not fit the layout's tag field.
    ///
    /// An edge carrying one-byte tags cannot spell tag 300. That is a
    /// property of the negotiated layout, not of the protocol — the same
    /// tag crosses an edge with a wider tag field without complaint.
    TagTooWide {
        /// The tag that could not be spelled.
        tag: Tag,
        /// Bytes the layout's tag field holds.
        width: usize,
    },
    /// A magnitude began with a zero byte.
    ///
    /// `IS-1` §3 says magnitudes *carry* no leading zeros, which states
    /// what an encoder does. **It does not follow that a decoder refuses
    /// them** — and a second implementation read §3 exactly that way and
    /// accepted `01/2`. §4 now states both, and this is the refusal.
    LeadingZero,
    /// The denominator of an exact rational was zero.
    ZeroDenominator,
    /// Numerator and denominator share a factor. Refuse; **do not
    /// silently reduce.**
    NotReduced,
    /// Zero arrived over a denominator other than 1.
    ///
    /// Zero's canonical denominator is 1, so `0/5` is a second byte
    /// string for a value that already has one. Same defect class as
    /// [`NotReduced`](Malformed::NotReduced), and found the same way.
    NonCanonicalZero,
    /// Zero arrived carrying a negative sign.
    ///
    /// **This row is not in `IS-1/1` §4.** It was found by writing this
    /// crate — a third implementation — and is owed as `IS-1/2`. See
    /// `datum/measure/third-implementation.md`.
    ///
    /// The reasoning is §4's own: `01 ‖ 0 ‖ 1` and `00 ‖ 0 ‖ 1` are two
    /// byte strings for one value, and every other such pair in the
    /// table refuses.
    NegativeZero,
}

impl From<Unsatisfiable> for Malformed {
    fn from(why: Unsatisfiable) -> Self {
        match why {
            Unsatisfiable::Overlong { declared, bound } => {
                Malformed::Overlong { declared, bound }
            }
        }
    }
}

impl core::fmt::Display for Malformed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated { want, have } => {
                write!(f, "truncated: wanted {want} bytes, had {have}")
            }
            Self::LengthExceedsRecord {
                declared,
                available,
            } => write!(
                f,
                "declared length {declared} exceeds the {available} bytes the record holds"
            ),
            Self::Overlong { declared, bound } => {
                write!(f, "declared length {declared} exceeds the bound {bound}")
            }
            Self::TrailingBytes { left } => {
                write!(f, "{left} bytes left over after the value was read")
            }
            Self::UnexpectedTag { expected, found } => {
                write!(f, "expected tag {expected}, found {found}")
            }
            Self::TagTooWide { tag, width } => {
                write!(f, "tag {tag} does not fit a {width}-byte tag field")
            }
            Self::SignByte(byte) => write!(f, "sign byte {byte} is neither 0 nor 1"),
            Self::LeadingZero => write!(f, "magnitude begins with a zero byte"),
            Self::ZeroDenominator => write!(f, "denominator is zero"),
            Self::NotReduced => write!(f, "numerator and denominator share a factor"),
            Self::NonCanonicalZero => write!(f, "zero over a denominator other than 1"),
            Self::NegativeZero => write!(f, "zero carrying a negative sign"),
        }
    }
}

impl std::error::Error for Malformed {}

/// Write a record.
///
/// The only encoder in this crate that takes a tag: everything else
/// writes a *value*, and a value becomes a record exactly here.
///
/// Refuses a value larger than `u32::MAX` rather than truncating the
/// length — a truncated length produces a well-formed record that means
/// something else, which is worse than a refusal.
pub fn put_frame(
    layout: &Layout,
    tag: Tag,
    value: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), Malformed> {
    let before = out.len();
    if !layout.put_tag(tag, out) {
        out.truncate(before);
        return Err(Malformed::TagTooWide {
            tag,
            width: layout.width_of(layout::TAG).unwrap_or(0),
        });
    }
    if !layout.put_length(value.len(), out) {
        out.truncate(before);
        return Err(Malformed::LengthExceedsRecord {
            declared: value.len(),
            available: layout.width_of(layout::LENGTH).unwrap_or(0),
        });
    }
    out.extend_from_slice(value);
    Ok(())
}

/// A cursor over a byte string.
///
/// Every method either advances by exactly what it read or leaves the
/// cursor where it was and names a refusal. There is no partial read.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// Start at the front of a byte string.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Bytes not yet read.
    fn rest(&self) -> &'a [u8] {
        self.bytes.get(self.at..).unwrap_or(&[])
    }

    /// How many bytes remain.
    pub fn remaining(&self) -> usize {
        self.rest().len()
    }

    /// Whether every byte has been read.
    pub fn is_done(&self) -> bool {
        self.remaining() == 0
    }

    /// Assert every byte was read.
    ///
    /// Call this at the end of a value. `IS-1` §4 refuses leftover
    /// bytes, and this is where that refusal is produced.
    pub fn finish(&self) -> Result<(), Malformed> {
        match self.remaining() {
            0 => Ok(()),
            left => Err(Malformed::TrailingBytes { left }),
        }
    }

    /// Take `n` bytes.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let end = self.at.checked_add(n).ok_or(Malformed::Truncated {
            want: n,
            have: self.remaining(),
        })?;
        let slice = self.bytes.get(self.at..end).ok_or(Malformed::Truncated {
            want: n,
            have: self.remaining(),
        })?;
        self.at = end;
        Ok(slice)
    }

    /// Take one byte.
    pub fn u8(&mut self) -> Result<u8, Malformed> {
        let slice = self.take(1)?;
        slice.first().copied().ok_or(Malformed::Truncated {
            want: 1,
            have: 0,
        })
    }

    /// Take a little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16, Malformed> {
        let slice = self.take(2)?;
        let array = <[u8; 2]>::try_from(slice).map_err(|_| Malformed::Truncated {
            want: 2,
            have: slice.len(),
        })?;
        Ok(u16::from_le_bytes(array))
    }

    /// Take a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32, Malformed> {
        let slice = self.take(4)?;
        let array = <[u8; 4]>::try_from(slice).map_err(|_| Malformed::Truncated {
            want: 4,
            have: slice.len(),
        })?;
        Ok(u32::from_le_bytes(array))
    }

    /// Take a little-endian `u64`.
    ///
    /// This was deleted once. A mutation flipped it to big-endian and
    /// failed none of 43 tests, because nothing called it — it existed
    /// because a caller was *anticipated*.
    ///
    /// It is back because a caller *arrived*: a declaration's tag ranges
    /// are [`Tag`]-wide now, so they need eight bytes, and
    /// `hello::Hello::decode` reads them. That is the whole difference
    /// between surface and dead surface, and the endianness is covered
    /// by `hello.rs`'s round-trip test rather than by nothing.
    pub fn u64(&mut self) -> Result<u64, Malformed> {
        let slice = self.take(8)?;
        let array = <[u8; 8]>::try_from(slice).map_err(|_| Malformed::Truncated {
            want: 8,
            have: slice.len(),
        })?;
        Ok(u64::from_le_bytes(array))
    }

    /// Take a little-endian `u128` — a chain amount (escrow, IS-6/7).
    pub fn u128(&mut self) -> Result<u128, Malformed> {
        let slice = self.take(16)?;
        let array = <[u8; 16]>::try_from(slice).map_err(|_| Malformed::Truncated {
            want: 16,
            have: slice.len(),
        })?;
        Ok(u128::from_le_bytes(array))
    }

    /// Take a `LE32`-prefixed byte string.
    ///
    /// Distinguishes *the length is longer than this record* from *the
    /// buffer ran out*, because the first is a malformed record and the
    /// second may just be a short read.
    pub fn sized(&mut self) -> Result<&'a [u8], Malformed> {
        let len = self.u32()? as usize;
        let available = self.remaining();
        if len > available {
            return Err(Malformed::LengthExceedsRecord {
                declared: len,
                available,
            });
        }
        self.take(len)
    }

    /// Take one whole record and return its tag and its value.
    ///
    /// **This is the skip.** A reader that does not own the tag has
    /// already stepped over the record by calling this, and the value it
    /// did not read is right there to forward. No knowledge of the
    /// value's layout is needed or available.
    pub fn frame(&mut self, layout: &Layout) -> Result<(Tag, &'a [u8]), Malformed> {
        let width = layout
            .width_of(layout::TAG)
            .ok_or(Malformed::Truncated { want: 1, have: 0 })?;
        let bytes = self.take(width)?;
        let tag = layout.take_tag(bytes).ok_or(Malformed::Truncated {
            want: width,
            have: bytes.len(),
        })?;
        let value = self.sized_under(layout)?;
        Ok((tag, value))
    }

    /// Take a record that must carry a particular tag.
    ///
    /// For nested records, where the enclosing layout fixes the tag.
    /// `IS-1` §4 refuses any other, rather than skipping it: inside a
    /// known layout an unexpected tag is not an unknown frame, it is a
    /// disagreement about the layout.
    pub fn nested(&mut self, layout: &Layout, expected: Tag) -> Result<&'a [u8], Malformed> {
        let (found, value) = self.frame(layout)?;
        if found == expected {
            Ok(value)
        } else {
            Err(Malformed::UnexpectedTag { expected, found })
        }
    }

    /// A length-prefixed byte string, in the layout's length width.
    ///
    /// [`Reader::sized`] reads a bare `LE32` for fields inside a value,
    /// where the width is the field's own business. This reads a
    /// *record's* length, which is the layout's.
    fn sized_under(&mut self, layout: &Layout) -> Result<&'a [u8], Malformed> {
        let width = layout
            .width_of(layout::LENGTH)
            .ok_or(Malformed::Truncated { want: 4, have: 0 })?;
        let header = self.take(width)?;
        // `take_length` reads from a whole header, so offset the slice
        // back to where the length field sits.
        let mut whole = vec![0u8; layout.offset_of(layout::LENGTH).unwrap_or(0)];
        whole.extend_from_slice(header);
        let len = layout.take_length(&whole).ok_or(Malformed::Truncated {
            want: width,
            have: header.len(),
        })?;
        let available = self.remaining();
        if len > available {
            return Err(Malformed::LengthExceedsRecord {
                declared: len,
                available,
            });
        }
        self.take(len)
    }
}
