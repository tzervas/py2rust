//! Keeps `tests/corpus/` (the expressibility-map corpus) honest: every snippet
//! must at least parse and produce a well-formed `GapReport` — never a crash —
//! so `scripts/expressibility_report.py` always has something to measure.
//!
//! This does not duplicate the Python harness (which drives the CLI and
//! regenerates `docs/EXPRESSIBILITY.md`); it just guards the corpus itself
//! from bit-rotting under `cargo test` in this crate, without a Python
//! interpreter or a `cargo run` subprocess in the loop.

use py2rust_core::analyze_source;
use std::fs;
use std::path::Path;

fn corpus_root() -> std::path::PathBuf {
    // crates/py2rust-core -> repo root -> tests/corpus
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is two levels under repo root")
        .join("tests")
        .join("corpus")
}

fn collect_py_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}")) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_py_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("py") {
            out.push(path);
        }
    }
}

#[test]
fn corpus_exists_and_covers_many_families() {
    let root = corpus_root();
    assert!(root.is_dir(), "expected {root:?} to exist");

    let mut families: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    families.sort();

    // Guards against silent shrinkage of the corpus; bump the count when
    // deliberately adding families, never lower it to make a test pass.
    assert!(
        families.len() >= 15,
        "expected at least 15 construct families in tests/corpus, found {}: {families:?}",
        families.len()
    );
}

#[test]
fn every_corpus_file_parses_and_never_silently_drops() {
    let root = corpus_root();
    let mut files = Vec::new();
    collect_py_files(&root, &mut files);
    assert!(!files.is_empty(), "no .py files found under {root:?}");

    for path in &files {
        let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let label = path.to_string_lossy();
        let report = analyze_source(&src, &label)
            .unwrap_or_else(|e| panic!("corpus file must parse cleanly: {path:?}: {e}"));

        // Never-silent invariant (G2): every top-level item is emitted, gapped,
        // or both — the corpus itself must not violate the property it exists
        // to measure.
        assert!(
            report.emitted_items.len() + report.gaps.len() >= report.total_top_level_items,
            "{path:?}: emitted ({}) + gaps ({}) < total_top_level_items ({}) — a statement was silently dropped",
            report.emitted_items.len(),
            report.gaps.len(),
            report.total_top_level_items
        );
    }
}
