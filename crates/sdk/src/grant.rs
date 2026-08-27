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

/// The key `holder` presents under, if the chain bound one (IS-6/4).
///
/// `None` is legacy/unbound — a court may refuse it, and a caller
/// must be able to tell it from a key.
pub fn binding(ledger: &Ledger, holder: &str) -> Option<isthmus::Binding> {
    ledger.binding_of(holder)
}

/// Whether a presenter is who the chain says holds `tag`, in `epoch`.
///
/// The full S3 predicate: a live deed covering the tag names the
/// holder, the chain bound a key for the holder, the presenter's
/// scheme and key match it, and the epoch falls inside the binding's
/// window. An unbound holder answers **false** here — this is the
/// strict check; a court that admits legacy grants calls
/// [`authorizes`] and says so.
pub fn authorizes_presenter(
    ledger: &Ledger,
    holder: &str,
    tag: Tag,
    scheme: u8,
    key: &[u8],
    epoch: u64,
) -> bool {
    if !authorizes(ledger, holder, tag) {
        return false;
    }
    match ledger.binding_of(holder) {
        Some(bound) => {
            bound.scheme == scheme
                && bound.key == key
                && bound.from_epoch <= epoch
                && epoch <= bound.until_epoch
        }
        None => false,
    }
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

    #[test]
    fn a_presenter_is_held_to_key_scheme_and_window() {
        let mut ledger = edge_with("kernel-a", 16);
        let deed = holdings(&ledger, "kernel-a")
            .into_iter()
            .next()
            .expect("issued");
        let tag = deed.low();
        let key = [7u8; 32];

        // Unbound: the strict check refuses even the right holder.
        assert!(!authorizes_presenter(&ledger, "kernel-a", tag, 0x01, &key, 5));

        ledger.record(isthmus::deed::Act::Bind {
            holder: "kernel-a".into(),
            scheme: 0x01,
            key: key.to_vec(),
            from_epoch: 3,
            until_epoch: 9,
        });

        assert!(authorizes_presenter(&ledger, "kernel-a", tag, 0x01, &key, 5));
        assert!(
            !authorizes_presenter(&ledger, "kernel-a", tag, 0x01, &key, 10),
            "outside the window is stale"
        );
        assert!(
            !authorizes_presenter(&ledger, "kernel-a", tag, 0x01, &[8u8; 32], 5),
            "another key is another party"
        );
        assert!(
            !authorizes_presenter(&ledger, "kernel-a", tag, 0x02, &key, 5),
            "another scheme is another statement"
        );
    }
}
