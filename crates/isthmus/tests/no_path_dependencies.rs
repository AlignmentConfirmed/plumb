//! The crate's whole claim, held by a test rather than by a comment.
//!
//! An outside title adds one line to its manifest. If a `path = `
//! dependency is ever added here, that title has to clone a tree it
//! never asked for — so this reads the manifest and refuses.
//!
//! It is a text read, which is the weaker kind of pin and is said so
//! plainly: it would not catch a dependency added under a target-
//! specific table this parser does not walk. What it does catch is the
//! way it would actually happen — someone reaching for a kernel type and
//! adding the obvious line.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

const MANIFEST: &str = include_str!("../Cargo.toml");

/// Lines inside a dependency table, comments and blanks removed.
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
fn no_dependency_is_a_path_dependency() {
    for line in dependency_lines() {
        assert!(
            !line.contains("path"),
            "isthmus grew a path dependency: {line}\n\
             The crate an integrator imports must not require a tree on disk."
        );
    }
}

/// And the gate can fail: the parser finds the dependency table at all.
///
/// Without this, the test above passes on an empty list — which is what
/// a broken parser returns, and it would read as *no path dependencies*.
#[test]
fn the_parser_actually_reads_the_dependency_table() {
    let lines = dependency_lines();
    assert!(
        !lines.is_empty(),
        "found no dependency lines at all — the parser is broken, and a \
         broken parser reports success"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("num-")),
        "expected the num-* dependencies; got {lines:?}"
    );

    // The same parser, over a manifest that DOES carry a path
    // dependency, must find it.
    let poisoned = "[dependencies]\nother = { path = \"../other\" }\n";
    let mut inside = false;
    let mut found = false;
    for raw in poisoned.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            inside = line.contains("dependencies");
            continue;
        }
        if inside && line.contains("path") {
            found = true;
        }
    }
    assert!(found, "the parser cannot see a path dependency it is given");
}
