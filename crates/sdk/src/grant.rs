//! Authorization as a ledger fact (IS-3).
//!
//! "Authorized kernel" means exactly one thing: the chain shows a live
//! deed for this holder covering the tag it writes. There is no
//! allowlist in source and no certificate — the registry deed **is**
//! the authorization. (Binding the presenter to the holder is the
//! signature layer, `decide/signatures.md`, designed and not built.)

use isthmus::deed::{Deed, Ledger};
use isthmus::layout::Tag;

/// Every live deed the chain shows for this holder.
///
/// Empty means unattached — a valid state, not an error: it is what a
/// kernel looks like before its grant matures.
pub fn holdings(ledger: &Ledger, holder: &str) -> Vec<Deed> {
    ledger
        .deeds()
        .into_iter()
        .filter(|d| d.live && d.holder == holder)
        .collect()
}

/// Whether the chain authorizes `holder` to write under `tag`.
///
/// True exactly when the live deed covering `tag` names this holder.
/// A retired deed answers false: a retired range is never reissued,
/// and never honored.
pub fn authorizes(ledger: &Ledger, holder: &str, tag: Tag) -> bool {
    ledger
        .holder_of(tag)
        .map(|d| d.live && d.holder == holder)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use isthmus::layout::Layout;

    /// An edge with the founding encumbrances and one issued deed —
    /// the same construction the substrate's own tests use.
    fn edge_with(holder: &str, width: u128) -> Ledger {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "both registries");
        ledger
            .issue(holder, width)
            .expect("room on a fresh edge");
        ledger
    }

    #[test]
    fn a_live_deed_authorizes_its_holder_and_nobody_else() {
        let ledger = edge_with("kernel-a", 16);
        let deed = holdings(&ledger, "kernel-a")
            .into_iter()
            .next()
            .expect("just issued");
        let inside = deed.low();
        assert!(authorizes(&ledger, "kernel-a", inside));
        assert!(!authorizes(&ledger, "kernel-b", inside), "not their range");
        assert!(
            !authorizes(&ledger, "kernel-a", deed.high().saturating_add(1)),
            "outside the range"
        );
    }

    #[test]
    fn holdings_reads_only_live_deeds_for_the_holder() {
        let ledger = edge_with("kernel-a", 16);
        assert_eq!(holdings(&ledger, "kernel-a").len(), 1);
        assert!(holdings(&ledger, "kernel-b").is_empty());
    }
}
