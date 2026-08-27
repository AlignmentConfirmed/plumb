//! The record header as a **structure**, not a number.
//!
//! ## What was wrong
//!
//! ```rust,ignore
//! pub const HEADER: usize = size_of::<u8>() + size_of::<u32>();
//! ```
//!
//! Two defects, and the second is worse.
//!
//! **`size_of::<u8>()` pins the tag at one byte.** That is a capacity —
//! 256 values, permanently, in the same crate that had just finished
//! removing a capacity of six attachments and a capacity of ten holders.
//! One level further down and the same shape: a limit expressed as a
//! type, so raising it means editing the type everywhere it appears.
//!
//! **The sum collapses the header to a scalar.** `5` is all that
//! survives. You cannot ask it where the length begins, how wide the tag
//! is, what the fields are called, or which of them is which. Every one
//! of those questions then gets answered somewhere else by hand — a `1`
//! here for the tag, a `1..5` there for the length — and those hand
//! answers are what has to stay in step when the layout moves.
//!
//! Adding widths is a *reading* of a layout. It is not the layout.
//!
//! ## What a layout is
//!
//! An ordered list of [`Field`]s. Every scalar question — width, offset,
//! tag space, header size — is derived by walking it, so there is one
//! place a layout is stated and no place it is restated.
//!
//! ```text
//! founding()      tag 1 byte, length 4 bytes      header 5
//! wide()          tag 4 bytes, length 4 bytes     header 8
//! ```
//!
//! A layout is **per-edge**, negotiated like a deed. Two peers that want
//! a wider tag space agree a wider layout; nothing recompiles, and an
//! edge that never needs more than 256 tags pays one byte.

/// A tag, in memory.
///
/// **Not `u8`.** How many bytes it occupies on the wire is
/// [`Layout::width_of`], which is a property of the negotiated layout
/// and not of this type. An edge carrying one-byte tags and an edge
/// carrying four-byte tags hold the same `Tag` values in memory and
/// spell them differently.
pub type Tag = u64;

/// How many bytes a field occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// A fixed number of bytes.
    Fixed(usize),
}

impl Width {
    /// Bytes this field occupies.
    pub fn bytes(self) -> usize {
        match self {
            Width::Fixed(n) => n,
        }
    }
}

/// One field of a record header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// What it is called. Offsets are asked for by name, so a field
    /// moving does not silently change what a caller reads.
    pub name: &'static str,
    /// How wide it is.
    pub width: Width,
}

/// The field a record's tag lives in.
pub const TAG: &str = "tag";
/// The field a record's value length lives in.
pub const LENGTH: &str = "length";

/// The shape of a record header on one edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    fields: Vec<Field>,
}

impl Default for Layout {
    fn default() -> Self {
        Self::founding()
    }
}

impl Layout {
    /// Build a layout from fields, in order.
    pub fn of(fields: Vec<Field>) -> Self {
        Self { fields }
    }

    /// The layout both ancestors already speak.
    ///
    /// `tag u8 ‖ LE32(length)` — measured byte-identical in netstratum
    /// and xylarium before it was written down. It is the founding
    /// edge's layout, not the protocol's only one.
    pub fn founding() -> Self {
        Self::with_tag_width(1)
    }

    /// The founding layout with a tag field of `bytes` bytes.
    ///
    /// This is the whole of *raising the tag space*: it takes an
    /// argument. Nothing recompiles, and no type changes.
    pub fn with_tag_width(bytes: usize) -> Self {
        Self::of(vec![
            Field {
                name: TAG,
                width: Width::Fixed(bytes),
            },
            Field {
                name: LENGTH,
                width: Width::Fixed(4),
            },
        ])
    }

    /// The fields, in order.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// A field by name.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Where a field begins, in bytes from the start of the record.
    ///
    /// The question a scalar `HEADER` could not answer, and which was
    /// therefore being answered by hand at every call site.
    pub fn offset_of(&self, name: &str) -> Option<usize> {
        let mut at = 0usize;
        for field in &self.fields {
            if field.name == name {
                return Some(at);
            }
            at = at.checked_add(field.width.bytes())?;
        }
        None
    }

    /// How wide a field is.
    pub fn width_of(&self, name: &str) -> Option<usize> {
        self.field(name).map(|f| f.width.bytes())
    }

    /// Bytes before the value begins.
    ///
    /// **Derived by folding the fields**, so it moves when they do. This
    /// is a reading of the layout, and the layout is the thing.
    pub fn header(&self) -> usize {
        self.fields
            .iter()
            .fold(0usize, |at, f| at.saturating_add(f.width.bytes()))
    }

    /// How many distinct tags this layout can spell.
    ///
    /// `u128` because a four-byte tag is already `2^32` and an eight-byte
    /// one is `2^64` — a count that does not fit the thing it counts.
    pub fn tag_space(&self) -> u128 {
        match self.width_of(TAG) {
            None => 0,
            Some(bytes) if bytes >= 16 => u128::MAX,
            Some(bytes) => 1u128 << (bytes.saturating_mul(8)),
        }
    }

    /// The largest tag this layout can spell, or `None` when the tag
    /// field is wider than a [`Tag`] can hold.
    pub fn max_tag(&self) -> Option<Tag> {
        let bytes = self.width_of(TAG)?;
        if bytes == 0 {
            return Some(0);
        }
        if bytes >= core::mem::size_of::<Tag>() {
            return Some(Tag::MAX);
        }
        Some(
            Tag::MAX
                .checked_shr(u32::try_from(
                    core::mem::size_of::<Tag>()
                        .saturating_sub(bytes)
                        .saturating_mul(8),
                )
                .unwrap_or(0))
                .unwrap_or(0),
        )
    }

    /// Whether a tag can be spelled under this layout.
    pub fn holds(&self, tag: Tag) -> bool {
        self.max_tag().is_some_and(|max| tag <= max)
    }

    /// Write a tag, little-endian, in this layout's width.
    pub fn put_tag(&self, tag: Tag, out: &mut Vec<u8>) -> bool {
        let Some(bytes) = self.width_of(TAG) else {
            return false;
        };
        if !self.holds(tag) {
            return false;
        }
        let le = tag.to_le_bytes();
        for at in 0..bytes {
            out.push(le.get(at).copied().unwrap_or(0));
        }
        true
    }

    /// Read a tag from the front of a slice, in this layout's width.
    pub fn take_tag(&self, bytes: &[u8]) -> Option<Tag> {
        let width = self.width_of(TAG)?;
        let slice = bytes.get(..width)?;
        let mut value: Tag = 0;
        for (at, byte) in slice.iter().enumerate() {
            let shift = u32::try_from(at.checked_mul(8)?).ok()?;
            value |= Tag::from(*byte).checked_shl(shift)?;
        }
        Some(value)
    }

    /// Read the declared value length from a record header.
    pub fn take_length(&self, bytes: &[u8]) -> Option<usize> {
        let at = self.offset_of(LENGTH)?;
        let width = self.width_of(LENGTH)?;
        let end = at.checked_add(width)?;
        let slice = bytes.get(at..end)?;
        let mut value: u64 = 0;
        for (n, byte) in slice.iter().enumerate() {
            let shift = u32::try_from(n.checked_mul(8)?).ok()?;
            value |= u64::from(*byte).checked_shl(shift)?;
        }
        usize::try_from(value).ok()
    }

    /// Write a value length in this layout's width.
    pub fn put_length(&self, len: usize, out: &mut Vec<u8>) -> bool {
        let Some(width) = self.width_of(LENGTH) else {
            return false;
        };
        let Ok(value) = u64::try_from(len) else {
            return false;
        };
        // A length that does not fit its field would produce a
        // well-formed record meaning something else, which is worse than
        // a refusal.
        if width < core::mem::size_of::<u64>() {
            let ceiling = 1u64.checked_shl(u32::try_from(width.saturating_mul(8)).unwrap_or(64));
            if let Some(ceiling) = ceiling {
                if value >= ceiling {
                    return false;
                }
            }
        }
        let le = value.to_le_bytes();
        for at in 0..width {
            out.push(le.get(at).copied().unwrap_or(0));
        }
        true
    }
}
