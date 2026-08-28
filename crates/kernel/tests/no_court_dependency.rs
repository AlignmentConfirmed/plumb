//! K1's whole point, held by a test rather than by a comment: a
//! kernel that could only join a court by linking the court's own
//! crate would not be a kernel a stranger could write. This crate
//! must build against sdk and the leaves alone.
//!
//! A text read over the manifest, the weaker kind of pin, said so
//! plainly (it would not catch a dependency added under a
//! target-specific table this parser does not walk) — what it does
//! catch is the way it would actually happen: someone reaching for a
//! court type and adding the obvious line.

#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

const MANIFEST: &str = include_str!("../Cargo.toml");

fn dependency_lines() -> Vec<String> {
    let mut inside = false;
    let mut lines = Vec::new();
    for raw in MANIFEST.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            inside = line.contains("dependencies");
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('#') {
            continue;
        }
        lines.push(line.to_string());
    }
    lines
}

#[test]
fn the_manifest_never_names_the_court() {
    for line in dependency_lines() {
        assert!(
            !line.contains("plumb-datum") && !line.starts_with("datum"),
            "the kernel grew a dependency on the court: {line}\n\
             a kernel that must link datum to join a court is not a \
             kernel a stranger could write — see K1, IMPLEMENTATION.md §6c"
        );
    }
}

/// The gate can fail: the parser finds dependency lines at all,
/// naming exactly the leaves + sdk this crate is scoped to.
#[test]
fn the_parser_actually_reads_the_dependency_table() {
    let lines = dependency_lines();
    assert!(!lines.is_empty(), "found no dependency lines — the parser is broken");
    for name in ["isthmus", "assay", "sig", "sdk"] {
        assert!(
            lines.iter().any(|l| l.starts_with(name)),
            "expected a {name} dependency; got {lines:?}"
        );
    }
}
