//! Never-silent invariant over the fixed Python fixture corpus (G2).
//!
//! Guarantee: **Empirical/Declared** — checked over hand-written fixtures, not
//! proptest-generated arbitrary Python (Stmt exhaustiveness rests on a catch-all arm).

use py2rust_core::{analyze_source, transpile_source, Category, GAP_SCHEMA_VERSION};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn fixture(name: &str) -> (String, String) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("fixtures");
    path.push(name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    (path.display().to_string(), source)
}

fn assert_never_silent(label: &str, source: &str) {
    let (report, _rust) = transpile_source(source, label, None)
        .unwrap_or_else(|e| panic!("fixture failed to parse ({label}): {e}"));
    assert!(
        report.never_silent_holds(),
        "never-silent violated for {label}: top_level={} emitted={} gaps={} emitted_items={:?}",
        report.total_top_level_items,
        report.emitted_items.len(),
        report.gaps.len(),
        report.emitted_items
    );
    assert_eq!(report.schema_version, GAP_SCHEMA_VERSION);
    for g in &report.gaps {
        assert!(g.line >= 1, "gap lines are 1-based, got {}", g.line);
        assert_eq!(g.python_construct, g.category.as_str());
    }
    for name in &report.emitted_items {
        assert!(!name.is_empty(), "empty emitted name in {label}");
    }
    let unique: BTreeSet<_> = report.emitted_items.iter().collect();
    assert_eq!(
        unique.len(),
        report.emitted_items.len(),
        "duplicate emitted names in {label}"
    );
}

const FIXTURES: &[&str] = &[
    "simple_fn.py",
    "class_only.py",
    "try_except.py",
    "lambda_mod.py",
    "mixed.py",
    "meta_exec.py",
];

#[test]
fn never_silent_over_fixture_corpus() {
    for name in FIXTURES {
        let (label, source) = fixture(name);
        assert_never_silent(&label, &source);
    }
}

#[test]
fn empty_module_vacuous() {
    assert_never_silent("empty.py", "");
}

#[test]
fn simple_fn_emits_typed_functions() {
    let (label, source) = fixture("simple_fn.py");
    let (report, rust) = transpile_source(&source, &label, Some("simple_fn")).unwrap();
    assert!(report.emitted_items.contains(&"add".into()));
    assert!(report.emitted_items.contains(&"unit".into()));
    assert_eq!(report.total_top_level_items, 2);
    assert!(rust.contains("fn add"));
    // Fully lowered simple bodies should not invent silent FunctionBody gaps.
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody && g.item_name.as_deref() == Some("add")),
        "typed add should lower without FunctionBody gap; gaps={:?}",
        report.gaps
    );
}

#[test]
fn test_comparison_boolean_ternary_lowering() {
    let src = r#"
def is_equal(x: int, y: int) -> bool:
    return x == y

def logical_or(a: bool, b: bool) -> bool:
    return a or b

def ternary_expr(x: int) -> int:
    return 1 if x > 0 else 0
"#;
    let (report, rust) = transpile_source(src, "expr_tests.py", None).unwrap();
    assert_eq!(report.emitted_items.len(), 3);
    assert!(report.gaps.is_empty(), "expected no gaps for supported logical and comparison expressions, got: {:?}", report.gaps);

    assert!(rust.contains("fn is_equal"), "Expected is_equal: {}", rust);
    assert!(rust.contains("(x == y)"), "Expected == comparison: {}", rust);

    assert!(rust.contains("fn logical_or"), "Expected logical_or: {}", rust);
    assert!(rust.contains("(a || b)"), "Expected logical or: {}", rust);

    assert!(rust.contains("fn ternary_expr"), "Expected ternary_expr: {}", rust);
    assert!(rust.contains("(if (x > 0) { 1 } else { 0 })"), "Expected ternary conditional: {}", rust);
}

#[test]
fn class_only_produces_class_gaps() {
    let (label, source) = fixture("class_only.py");
    let report = analyze_source(&source, &label).unwrap();
    assert!(report.emitted_items.is_empty());
    assert_eq!(report.gaps.len(), report.total_top_level_items);
    assert!(report.gaps.iter().all(|g| g.category == Category::Class));
    let names: BTreeSet<_> = report
        .gaps
        .iter()
        .filter_map(|g| g.item_name.clone())
        .collect();
    assert!(names.contains("Animal"));
    assert!(names.contains("Dog"));
}

#[test]
fn try_except_produces_exception_gaps() {
    let (label, source) = fixture("try_except.py");
    let (report, _rust) = transpile_source(&source, &label, None).unwrap();
    // may_fail is emitted but should carry Exception sub-gap from try inside body;
    // top-level raise is a hard Exception gap.
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::Exception),
        "expected Exception gaps: {:?}",
        report.category_counts()
    );
    assert!(report.never_silent_holds());
}

#[test]
fn lambda_mod_flags_lambda_and_dynamic() {
    let (label, source) = fixture("lambda_mod.py");
    let report = analyze_source(&source, &label).unwrap();
    assert!(
        report.gaps.iter().any(|g| g.category == Category::Lambda),
        "expected Lambda gap: {:?}",
        report.category_counts()
    );
    // untyped apply should emit with DynamicTyping sub-gaps or similar honesty
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::DynamicTyping)
            || report.emitted_items.iter().any(|n| n == "apply"),
        "expected DynamicTyping and/or emitted apply: emitted={:?} gaps={:?}",
        report.emitted_items,
        report.category_counts()
    );
}

#[test]
fn mixed_covers_readme_categories() {
    let (label, source) = fixture("mixed.py");
    let (report, rust) = transpile_source(&source, &label, Some("mixed")).unwrap();
    assert!(report.never_silent_holds());
    let cats = report.category_counts();
    // README limitations closed-as-flags:
    assert!(cats.contains_key("Class"), "Class: {cats:?}");
    assert!(
        cats.contains_key("Exception")
            || report
                .gaps
                .iter()
                .any(|g| g.category == Category::Exception),
        "Exception: {cats:?}"
    );
    assert!(
        cats.contains_key("DynamicTyping"),
        "DynamicTyping: {cats:?}"
    );
    assert!(
        cats.contains_key("Metaprogramming"),
        "Metaprogramming: {cats:?}"
    );
    assert!(cats.contains_key("Import"), "Import: {cats:?}");
    assert!(cats.contains_key("Lambda"), "Lambda: {cats:?}");
    // Typed function still emitted
    assert!(report.emitted_items.iter().any(|n| n == "typed_add"));
    assert!(rust.contains("fn typed_add"));
}

#[test]
fn meta_exec_is_metaprogramming() {
    let (label, source) = fixture("meta_exec.py");
    let report = analyze_source(&source, &label).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::Metaprogramming),
        "exec body should gap as Metaprogramming: {:?}",
        report.gaps
    );
}

#[test]
fn gap_json_schema_stable_keys() {
    let (label, source) = fixture("mixed.py");
    let report = analyze_source(&source, &label).unwrap();
    let json = report.to_json_pretty().unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    for key in [
        "schema_version",
        "source",
        "emitted_items",
        "gaps",
        "total_top_level_items",
    ] {
        assert!(v.get(key).is_some(), "missing stable key {key} in {json}");
    }
    assert_eq!(v["schema_version"], GAP_SCHEMA_VERSION);
    if let Some(arr) = v["gaps"].as_array() {
        if let Some(g0) = arr.first() {
            for key in [
                "file",
                "line",
                "col",
                "category",
                "python_construct",
                "snippet",
                "reason",
            ] {
                assert!(g0.get(key).is_some(), "gap missing {key}");
            }
        }
    }
}

#[test]
fn no_silent_function_body_todo_without_gap() {
    // Any emitted fn whose body is not lowered must carry FunctionBody gap.
    let src = "def complex(a: int) -> int:\n    x = a + 1\n    y = x * 2\n    return y\n";
    let (report, rust) = transpile_source(src, "complex.py", None).unwrap();
    assert!(report.emitted_items.contains(&"complex".into()));
    assert!(
        rust.contains("GAP: FunctionBody") || rust.contains("todo!"),
        "expected honest body placeholder, got:\n{rust}"
    );
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "FunctionBody gap required when body not lowered: {:?}",
        report.gaps
    );
}
