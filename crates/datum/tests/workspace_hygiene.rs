//! The workspace's whole claim, held by a test rather than by a README.
//!
//! "Edge-free by construction" means: no crate in this workspace names
//! a path outside it. `isthmus` holds the stricter law for itself (no
//! path dependencies at all — `no_path_dependencies.rs`); this test
//! holds the workspace law for **every** member: a `path =` dependency
//! may point at a sibling crate, and may point nowhere else.
//!
//! It lives in datum because datum is the crate that is allowed to
//! know about everything — the same reason the measurement bench lived
//! here before the split.
//!
//! A text read, which is the weaker kind of pin, said plainly: it
//! walks `crates/*/Cargo.toml` and would not catch a target-specific
//! table. What it catches is how it would actually happen — someone
//! repointing a dependency at a working tree on their machine.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

/// The workspace root, from this crate's own manifest dir.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/datum has a workspace above it")
        .to_path_buf()
}

/// Every member manifest, found on disk rather than assumed.
fn member_manifests() -> Vec<PathBuf> {
    let crates = workspace_root().join("crates");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("crates/ exists") {
        let dir = entry.expect("readable entry").path();
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            found.push(manifest);
        }
    }
    found.sort();
    found
}

/// `path = "…"` values inside dependency tables, with their manifest.
fn path_values(manifest: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(manifest).expect("manifest reads");
    let mut inside = false;
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            inside = line.contains("dependencies");
            continue;
        }
        if !inside || line.starts_with('#') {
            continue;
        }
        let mut rest = line;
        while let Some(at) = rest.find("path = \"") {
            let tail = &rest[at + "path = \"".len()..];
            if let Some(end) = tail.find('"') {
                out.push(tail[..end].to_string());
                rest = &tail[end..];
            } else {
                break;
            }
        }
    }
    out
}

#[test]
fn every_path_dependency_stays_inside_the_workspace() {
    let root = workspace_root()
        .canonicalize()
        .expect("workspace root resolves");
    for manifest in member_manifests() {
        let dir = manifest.parent().expect("manifest has a dir");
        for value in path_values(&manifest) {
            assert!(
                !value.starts_with('/'),
                "{} names an absolute path: {value}\n\
                 A path outside the workspace is a tree an outsider \
                 does not have.",
                manifest.display()
            );
            let resolved = dir
                .join(&value)
                .canonicalize()
                .unwrap_or_else(|_| panic!(
                    "{} names a path that does not exist: {value}",
                    manifest.display()
                ));
            assert!(
                resolved.starts_with(&root),
                "{} escapes the workspace: {value} -> {}",
                manifest.display(),
                resolved.display()
            );
        }
    }
}

/// And the gate can fail: the walk finds manifests, and finds the
/// sibling paths that are supposed to exist. Without this, a broken
/// walker returns empty lists — which would read as *all clean*.
#[test]
fn the_walker_actually_reads_the_workspace() {
    let manifests = member_manifests();
    assert!(
        manifests.len() >= 4,
        "expected at least four member crates, found {}",
        manifests.len()
    );
    let total: usize = manifests.iter().map(|m| path_values(m).len()).sum();
    assert!(
        total >= 3,
        "expected at least the datum->isthmus/assay and sdk->isthmus \
         path deps, found {total} — the parser is broken, and a broken \
         parser reports success"
    );
}
