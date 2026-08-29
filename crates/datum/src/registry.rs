//! The tag registry, counted.
//!
//! Records are framed as `tag u8 | LE32(len) | value`, so the tag space
//! is 256 and is shared by everything that speaks the wire.
//!
//! A registry is a prose table, so this is the one place in `datum` that
//! reads text rather than calling code. The defect to avoid: a row's
//! first cell is a number and its second says what the number is for, and
//! a reader that takes the first without the second expands
//! `32-255 | unclaimed` into 224 false claims.

use std::collections::BTreeSet;
use std::path::Path;

/// What a registry row says about its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// In use, with a meaning.
    Claimed,
    /// Set aside for a named holder, not yet used.
    Held,
    /// Free.
    Unclaimed,
}

/// Every tag a registry names, with what the row said about it.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    pub claimed: BTreeSet<u16>,
    pub held: BTreeSet<u16>,
}

impl Registry {
    /// Parse one file. Rows may be bare (`| 12 | ...`) or inside a Rust
    /// doc comment (`//! | 12 | ...`).
    pub fn read(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut out = Self::default();

        for line in text.lines() {
            let line = line.trim_start();
            let line = line
                .strip_prefix("//!")
                .or_else(|| line.strip_prefix("//"))
                .unwrap_or(line)
                .trim_start();

            let Some(row) = line.strip_prefix('|') else {
                continue;
            };
            let mut cells = row.split('|');
            let (Some(number), Some(meaning)) = (cells.next(), cells.next()) else {
                continue;
            };

            let Some((low, high)) = span(number.trim()) else {
                continue;
            };
            let state = classify(meaning);
            for tag in low..=high {
                match state {
                    State::Claimed => out.claimed.insert(tag),
                    State::Held => out.held.insert(tag),
                    State::Unclaimed => false,
                };
            }
        }
        Ok(out)
    }

    /// Fold another registry's rows in.
    pub fn merge(&mut self, other: &Self) {
        self.claimed.extend(other.claimed.iter().copied());
        self.held.extend(other.held.iter().copied());
    }

    /// Tags claimed by both, which therefore mean two things.
    pub fn collisions(&self, other: &Self) -> BTreeSet<u16> {
        self.claimed.intersection(&other.claimed).copied().collect()
    }
}

/// `12` or `19–31`. Either dash. Anything else is not a registry row.
fn span(cell: &str) -> Option<(u16, u16)> {
    let cell = cell.trim();
    if cell.is_empty() {
        return None;
    }
    let (low, high) = match cell.split_once(['–', '-']) {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (cell, cell),
    };
    let low: u16 = low.parse().ok()?;
    let high: u16 = high.parse().ok()?;
    (low <= high && high < 256).then_some((low, high))
}

/// **From the meaning cell, never from the number.**
fn classify(meaning: &str) -> State {
    let meaning = meaning.to_ascii_lowercase();
    let free = meaning.contains("unclaimed");
    let spoken_for = meaning.contains("held") || meaning.contains("reserved");
    match (free, spoken_for) {
        (true, true) => State::Held,
        (true, false) => State::Unclaimed,
        (false, true) => State::Held,
        (false, false) => State::Claimed,
    }
}

/// The whole space, one byte wide.
pub const SPACE: usize = 256;
