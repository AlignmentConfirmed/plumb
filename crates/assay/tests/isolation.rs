//! THE ISOLATION GATE — rules 1 and 2, enforced rather than declared.
//!
//! `assay` is a leaf. It imports nothing from the mesh (`isthmus`) or
//! any kernel, and it contains no floating point.
//! Both are properties of the *source*, so both are checked by reading
//! the source — a manifest comment saying so is a comment.
//!
//! This is the same instrument `isthmus/tests/no_path_dependencies.rs`
//! uses, and for the same reason: a dependency that would violate the
//! architecture must fail a test rather than a code review.
//!
//! Tests may panic. A test that cannot reach its subject must say so
//! loudly rather than pass quietly.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `src/`.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Strip `//`-comments so a rule's own *explanation* does not trip it.
///
/// The first version of this test read raw lines and failed on
/// `lib.rs`, which says "assay imports nothing from `isthmus` or any
/// kernel" in its own header. A gate that cannot survive being
/// documented is a gate that will be deleted.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => line.get(..at).unwrap_or(""),
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// **Rule 1.** No mesh, no kernel, no path dependency — in the manifest.
#[test]
fn the_manifest_names_no_mesh_no_kernel_and_no_path() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml"))
        .expect("assay has no Cargo.toml");
    let code = code_only(&manifest.replace('#', "//"));

    for forbidden in ["isthmus", "path ="] {
        assert!(
            !code.contains(forbidden),
            "Cargo.toml names `{forbidden}` — assay is a leaf and the \
             convergence engine must not be able to reach the network \
             or a kernel",
        );
    }

    // And the gate is not vacuous: the arithmetic crates ARE named, so
    // the check is reading a real dependency table.
    assert!(
        code.contains("num-rational"),
        "the manifest has no dependency table to check",
    );
}

/// **Rule 1, in the source.** Nothing `use`s a mesh or a kernel.
///
/// The manifest could be clean while a file reached for a crate through
/// some other route, so the imports are read too.
#[test]
fn no_source_file_imports_a_mesh_or_a_kernel() {
    let mut files = Vec::new();
    sources(&root().join("src"), &mut files);
    assert!(!files.is_empty(), "no sources found — this test measured nothing");

    let mut offences = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("unreadable source");
        let code = code_only(&text);
        for forbidden in ["isthmus"] {
            if code.contains(forbidden) {
                offences.push(format!("{} names {forbidden}", path.display()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the convergence engine reached for the mesh or a kernel:\n{}",
        offences.join("\n"),
    );
}

/// **Rule 2.** There is no floating point in this crate.
///
/// Checked by reading the source rather than by a lint, because a lint
/// can be `#[allow]`ed at the one site that needed it and a rounded
/// flux is a closure nobody can check.
#[test]
fn no_source_file_contains_a_floating_point_type() {
    let mut files = Vec::new();
    sources(&root().join("src"), &mut files);
    assert!(!files.is_empty(), "no sources found — this test measured nothing");

    let mut offences = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("unreadable source");
        let code = code_only(&text);
        for forbidden in ["f32", "f64", "as f", "EPSILON", "abs()"] {
            if code.contains(forbidden) {
                offences.push(format!("{} contains `{forbidden}`", path.display()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "floating point or a tolerance reached the convergence engine:\n{}",
        offences.join("\n"),
    );
}

/// **The gates fire.** Each check above is run against text that must
/// trip it, so none of them is passing because its pattern never
/// matches anything.
#[test]
fn the_isolation_gates_are_not_vacuous() {
    // A manifest that carries a path dependency.
    let bad_manifest = code_only(&"[dependencies]\nfoo = { path = \"../foo\" }\n".replace('#', "//"));
    assert!(bad_manifest.contains("path ="), "the manifest check cannot fire");

    // A source that imports the mesh.
    let bad_source = code_only("use isthmus::deed::Ledger;\nfn main() {}");
    assert!(bad_source.contains("isthmus"), "the import check cannot fire");

    // A source with a float.
    let bad_float = code_only("fn area(x: f64) -> f64 { x * 2.0 }");
    assert!(bad_float.contains("f64"), "the float check cannot fire");

    // And a comment mentioning them all does NOT trip, or documenting
    // the rule would break the rule.
    let documented = code_only("// assay imports nothing from isthmus or any kernel, and no f64");
    assert!(
        !documented.contains("isthmus") && !documented.contains("f64"),
        "a comment tripped the gate — the rule cannot be explained",
    );
}
