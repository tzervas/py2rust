//! End-to-end: a source tree in, artifacts and both reports out.
//!
//! The unit tests in `batch.rs` render reports from hand-built `BatchSummary`
//! values, which is fast and precise but never exercises the actual pipeline —
//! discover, parse, emit, write sidecars, aggregate, render. A bug in the wiring
//! between those stages would pass every unit test.
//!
//! Deliberately kept declarative: the corpus is a fixture table, and the
//! assertions are about *properties of the output*, not string offsets. Adding a
//! case is a row, not a new function.

use py2rust_core::{render_priority_report, render_ranked_report, transpile_batch};
use std::path::{Path, PathBuf};

/// One file in the fixture corpus.
struct Fixture {
    path: &'static str,
    src: &'static str,
    /// Whether this file is expected to parse.
    parses: bool,
}

const CORPUS: &[Fixture] = &[
    Fixture {
        path: "pkg/simple.py",
        src: "def add(a: int, b: int) -> int:\n    return a + b\n",
        parses: true,
    },
    Fixture {
        path: "pkg/nested.py",
        src: "def outer(x: int) -> int:\n    if x:\n        try:\n            f = lambda y: y\n        except ValueError:\n            pass\n    return x\n",
        parses: true,
    },
    Fixture {
        path: "pkg/klass.py",
        src: "class C:\n    def m(self) -> int:\n        return 1\n",
        parses: true,
    },
    Fixture {
        path: "broken.py",
        src: "def (\n",
        parses: false,
    },
];

/// Build the fixture tree under a unique temp dir; caller removes it.
fn make_corpus(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("py2rust-e2e-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for f in CORPUS {
        let p = dir.join(f.path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, f.src).unwrap();
    }
    dir
}

fn expected_ok() -> usize {
    CORPUS.iter().filter(|f| f.parses).count()
}

#[test]
fn batch_writes_artifacts_and_accounts_for_every_file() {
    let dir = make_corpus("artifacts");
    let out = dir.join("out");
    let (summary, union) = transpile_batch(&dir, &out).expect("batch runs");

    assert_eq!(summary.total_files, CORPUS.len(), "every file discovered");
    assert_eq!(summary.ok_files, expected_ok(), "one fixture is unparseable");

    assert!(out.join("summary.json").exists(), "summary.json written");
    assert!(out.join("union.gap.json").exists(), "union.gap.json written");

    // Never-silent: every parsed file is accounted for by an emitted item or a
    // gap. A file that produced neither would be a silent drop.
    for f in summary.files.iter().filter(|f| f.error.is_none()) {
        assert!(
            f.emitted > 0 || f.gaps > 0,
            "{} produced neither emission nor gap — a silent drop",
            f.source
        );
    }

    // The unparsed fixture must be recorded as an error, not merely absent.
    let broken: Vec<_> = summary.files.iter().filter(|f| f.error.is_some()).collect();
    assert_eq!(broken.len(), 1, "the broken fixture is recorded, not skipped");
    assert!(broken[0].source.ends_with("broken.py"));

    assert!(union.file_count >= expected_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn l2_denominator_exceeds_l1_on_a_corpus_with_bodies() {
    let dir = make_corpus("l2");
    let (summary, _) = transpile_batch(&dir, &dir.join("out")).unwrap();

    assert!(
        summary.total_statements > summary.total_top_level,
        "fixtures contain function bodies, so L2's denominator must be larger: \
         statements={} top_level={}",
        summary.total_statements,
        summary.total_top_level
    );
    assert!(
        summary.lowered_statements <= summary.total_statements,
        "cannot lower more statements than exist"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nested_constructs_are_reported_not_swallowed() {
    // Regression guard: a lambda inside a try inside an if was invisible before
    // the recursive walk landed. This is the shape that regressed.
    let dir = make_corpus("nested");
    let (_, union) = transpile_batch(&dir, &dir.join("out")).unwrap();

    let cats: std::collections::BTreeSet<&str> =
        union.gaps.iter().map(|g| g.category.as_str()).collect();
    for expected in ["Exception", "Lambda"] {
        assert!(
            cats.contains(expected),
            "nested {expected} was swallowed; categories found: {cats:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_reports_render_with_their_required_sections() {
    let dir = make_corpus("reports");
    let (summary, union) = transpile_batch(&dir, &dir.join("out")).unwrap();

    let full = render_ranked_report(&dir, &summary, &union);
    for heading in [
        "# py2rust corpus report",
        "L2 statement coverage",
        "## Gap categories by measured frequency",
        "## Modules by expressible fraction",
        "## Files that did not parse",
    ] {
        assert!(full.contains(heading), "full report missing {heading:?}");
    }
    assert!(
        full.contains("broken.py"),
        "an unparsed file must be named, not silently dropped"
    );

    let prio = render_priority_report(&dir, &summary, &union, 2);
    assert!(prio.contains("# py2rust — high priority"));
    assert!(
        prio.contains("## Omitted"),
        "the priority report must always state what it left out"
    );
    assert!(
        prio.contains("not evidence of"),
        "and must disclaim completeness"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn priority_budget_is_actually_enforced() {
    let dir = make_corpus("budget");
    let (summary, union) = transpile_batch(&dir, &dir.join("out")).unwrap();

    // A budget of 1 must yield at most one listed item, whatever the corpus.
    let prio = render_priority_report(&dir, &summary, &union, 1);
    let listed = prio.matches("— id `").count();
    assert!(
        listed <= 1,
        "budget of 1 listed {listed} items — an unbounded priority report is \
         just the full report:\n{prio}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerunning_over_an_unchanged_corpus_is_deterministic() {
    // The corpus workflow commits only when output changes, so a re-run over
    // unchanged input must produce byte-identical reports. Nondeterminism here
    // would make every re-run look like progress.
    let dir = make_corpus("determinism");
    let (s1, u1) = transpile_batch(&dir, &dir.join("out1")).unwrap();
    let (s2, u2) = transpile_batch(&dir, &dir.join("out2")).unwrap();

    let root: &Path = &dir;
    assert_eq!(
        render_ranked_report(root, &s1, &u1),
        render_ranked_report(root, &s2, &u2),
        "full report differs between identical runs"
    );
    assert_eq!(
        render_priority_report(root, &s1, &u1, 10),
        render_priority_report(root, &s2, &u2, 10),
        "priority report differs between identical runs"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
