//! The tag registries, counted.
//!
//! Both projects frame records as `tag u8 | LE32(len) | value`, so the
//! space is 256 and it is shared by everything that speaks the wire.
//!
//! The registries are prose tables, so this is the one place in `datum`
//! that reads text rather than calling code. The defect to avoid is
//! recorded: a row's first cell is a number and its second says what the
//! number is FOR, and a reader that takes the first without the second
//! expanded `32-255 | *unclaimed*` into 224 claims and reported strand
//! as holding 255 of 256.

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
    /// **Who a held range is held FOR**, when the row names them.
    ///
    /// A row like `| 64–79 | *unclaimed* — held for datum-lane |`
    /// reserves ground for a party under *that registry's* name for
    /// them. `IS-3` §5 grants the same range under a different name —
    /// `isthmus` — and until this field existed nothing connected the
    /// two.
    ///
    /// **Two registries naming different grantees for one range is how
    /// the 32–47 reissue happened** (`IS-3` §5.4). The names being
    /// different is not the defect; nobody having written down whether
    /// they are the same party is. See [`reconcile`].
    pub held_for: std::collections::BTreeMap<u16, String>,
}

impl Registry {
    /// Parse one file. Rows may be bare (`| 12 | ...`) or inside a Rust
    /// doc comment (`//! | 12 | ...`), which is how strand carries its
    /// registry and what an earlier reader missed entirely.
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
            let beneficiary = held_for(meaning);
            for tag in low..=high {
                match state {
                    State::Claimed => out.claimed.insert(tag),
                    State::Held => out.held.insert(tag),
                    State::Unclaimed => false,
                };
                if let (State::Held, Some(who)) = (state, beneficiary.as_deref()) {
                    out.held_for.insert(tag, who.to_owned());
                }
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

/// Who a row says a range is **held for**, if it says.
///
/// Reads the text after `held for` up to the first punctuation that
/// ends a name. Markdown decoration is stripped, because a registry is
/// prose and `**held for datum-lane**` names the same party as
/// `held for datum-lane`.
///
/// `None` when the row reserves ground without naming a beneficiary —
/// which is a different and lesser statement, and is not turned into
/// one by guessing.
fn held_for(meaning: &str) -> Option<String> {
    let plain = meaning.replace(['*', '`'], "");
    let at = plain.to_ascii_lowercase().find("held for")?;
    let tail = plain.get(at.checked_add("held for".len())?..)?;
    let name: String = tail
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// **The reconciliation: which registry's name is whose.**
///
/// One row per party that appears under more than one name. `IS-3` §5
/// grants to a **crate**; another project's registry reserves for a
/// **lane**. Neither is wrong and the two must be stated to be one, or
/// the pair reads as two grantees for one range — which is exactly the
/// shape of the `32–47` reissue (`IS-3` §5.4), where a range was
/// granted twice because nobody compared the tables.
///
/// **The point is not that this list is correct.** It is that adding a
/// row is a deliberate act somebody performs, and that an unreconciled
/// name fails a test rather than passing quietly. A guess about
/// identity is the one thing this must not do.
pub const RECONCILED: [(&str, &str); 1] = [
    // strand's registry: `| 64–79 | *unclaimed* — held for datum-lane |`
    // IS-3 §5:           `| 64–79 | isthmus |`
    // The lane produces the crate. One grant, two names for one party.
    ("datum-lane", "isthmus"),
];

/// The `IS-3` §5 grantee a foreign registry's name refers to.
pub fn reconcile(name: &str) -> Option<&'static str> {
    RECONCILED
        .iter()
        .find(|(theirs, _)| *theirs == name)
        .map(|(_, ours)| *ours)
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

