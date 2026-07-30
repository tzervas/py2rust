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
fn nested_constructs_deeply_scanned_recursively() {
    // We should correctly scan deep inside If, For, etc. inside a function body.
    let src = r#"
def my_complex_fn(x: int) -> int:
    if x > 10:
        for i in range(x):
            try:
                print(lambda: i)
                exec("y = 1")
            except Exception:
                pass
    return x
"#;
    let (report, _rust) = transpile_source(src, "nested.py", None).unwrap();
    let categories: BTreeSet<_> = report.gaps.iter().map(|g| g.category).collect();

    assert!(
        categories.contains(&Category::Exception),
        "Expected Exception gap: {:?}",
        report.gaps
    );
    assert!(
        categories.contains(&Category::Lambda),
        "Expected Lambda gap: {:?}",
        report.gaps
    );
    assert!(
        categories.contains(&Category::Metaprogramming),
        "Expected Metaprogramming gap: {:?}",
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
    assert!(
        report.gaps.is_empty(),
        "expected no gaps for supported logical and comparison expressions, got: {:?}",
        report.gaps
    );

    assert!(rust.contains("fn is_equal"), "Expected is_equal: {}", rust);
    assert!(
        rust.contains("(x == y)"),
        "Expected == comparison: {}",
        rust
    );

    assert!(
        rust.contains("fn logical_or"),
        "Expected logical_or: {}",
        rust
    );
    assert!(rust.contains("(a || b)"), "Expected logical or: {}", rust);

    assert!(
        rust.contains("fn ternary_expr"),
        "Expected ternary_expr: {}",
        rust
    );
    assert!(
        rust.contains("(if (x > 0) { 1 } else { 0 })"),
        "Expected ternary conditional: {}",
        rust
    );
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
    assert!(
        cats.contains_key("Import")
            || report
                .emitted_items
                .iter()
                .any(|n| n.starts_with("#import:")),
        "Import gap or stdlib import-map emit required: cats={cats:?} emitted={:?}",
        report.emitted_items
    );
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
fn multi_stmt_assign_return_lowers() {
    // Multi-statement bodies of assign+return are now lowered (L2 progress).
    let src = "def complex(a: int) -> int:\n    x = a + 1\n    y = x * 2\n    return y\n";
    let (report, rust) = transpile_source(src, "complex.py", None).unwrap();
    assert!(report.emitted_items.contains(&"complex".into()));
    assert!(
        rust.contains("let x =") && rust.contains("let y ="),
        "expected multi-stmt lowering, got:\n{rust}"
    );
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "fully lowered multi-stmt body must not carry FunctionBody: {:?}",
        report.gaps
    );
}

#[test]
fn no_silent_function_body_todo_without_gap() {
    // Any emitted fn whose body is not lowered must carry FunctionBody gap.
    // Uses a call we do not lower (`print`) so the body is forced to decline.
    let src = "def complex(a: int) -> int:\n    print(a)\n    return a\n";
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

#[test]
fn if_else_and_typing_import_erase() {
    let src = r#"
from typing import Optional
import typing

def clamp(x: int, lo: int, hi: int) -> int:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x

def double_list(xs: list[int]) -> list[int]:
    return xs
"#;
    let (report, rust) = transpile_source(src, "ifelse.py", None).unwrap();
    assert!(
        report
            .emitted_items
            .iter()
            .any(|n| n.starts_with("#erase:")),
        "typing imports should erase: {:?}",
        report.emitted_items
    );
    assert!(
        !report.gaps.iter().any(|g| g.category == Category::Import),
        "erasable imports must not leave Import gaps: {:?}",
        report.gaps
    );
    assert!(
        rust.contains("fn clamp") && rust.contains("if (x < lo)"),
        "if/else should lower:\n{rust}"
    );
    assert!(
        rust.contains("fn double_list") && rust.contains("Vec<i64>"),
        "list[int] should map:\n{rust}"
    );
    assert!(
        !report.gaps.iter().any(|g| {
            g.category == Category::FunctionBody && g.item_name.as_deref() == Some("clamp")
        }),
        "clamp should lower: {:?}",
        report.gaps
    );
}

#[test]
fn for_range_and_break_continue_lower() {
    // Pure for/range/break/continue without rebinding outer names — honest lower.
    let src = r#"
def first_positive(n: int) -> int:
    for i in range(n):
        if i == 0:
            continue
        if i > 100:
            break
        return i
    return 0

def sum_slice_ids(a: int, b: int) -> int:
    # no accumulator: just returns upper bound after iterating
    for i in range(a, b):
        if i < 0:
            continue
    return b
"#;
    let (report, rust) = transpile_source(src, "for_range.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody
                && g.item_name.as_deref() == Some("first_positive")),
        "for/range/break/continue without outer rebind should lower: {:?}",
        report.gaps
    );
    assert!(
        rust.contains("for i in 0..n") && rust.contains("continue;") && rust.contains("break;"),
        "expected for/range/break/continue:\n{rust}"
    );
    assert!(rust.contains("for i in a..b"), "range(a,b) → a..b:\n{rust}");
}

#[test]
fn for_loop_outer_rebind_uses_mut() {
    // Accumulator pattern: `let mut total` + assignment inside the loop (not shadow let).
    let src = r#"
def sum_range(n: int) -> int:
    total = 0
    for i in range(n):
        total = total + i
    return total
"#;
    let (report, rust) = transpile_source(src, "for_rebind.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "sum_range with loop accumulator must lower fully: gaps={:?}\nrust:\n{rust}",
        report.gaps
    );
    assert!(
        rust.contains("let mut total"),
        "expected let mut total for accumulator:\n{rust}"
    );
    assert!(
        rust.contains("total = (total + i)") || rust.contains("total = total + i"),
        "expected bare assignment inside loop (no re-let):\n{rust}"
    );
    // Must not re-let the accumulator inside the loop body.
    let after_for = rust.split("for i in").nth(1).unwrap_or("");
    assert!(
        !after_for.contains("let total") && !after_for.contains("let mut total"),
        "loop body must not re-let total:\n{rust}"
    );
    assert!(
        !rust.contains("todo!") && !rust.contains("GAP: FunctionBody"),
        "expected full lower, got stub:\n{rust}"
    );
}

#[test]
fn for_loop_annassign_outer_rebind_uses_mut() {
    // Annotated accumulator: same mut + assignment shape as bare Assign.
    let src = r#"
def sum_range(n: int) -> int:
    total: int = 0
    for i in range(n):
        total: int = total + i
    return total
"#;
    let (report, rust) = transpile_source(src, "for_ann_rebind.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "AnnAssign accumulator must lower fully: gaps={:?}\nrust:\n{rust}",
        report.gaps
    );
    assert!(
        rust.contains("let mut total"),
        "expected let mut total for annotated accumulator:\n{rust}"
    );
    assert!(
        rust.contains("total = (total + i)") || rust.contains("total = total + i"),
        "expected bare assignment inside loop:\n{rust}"
    );
    let after_for = rust.split("for i in").nth(1).unwrap_or("");
    assert!(
        !after_for.contains("let total") && !after_for.contains("let mut total"),
        "loop body must not re-let total:\n{rust}"
    );
}

#[test]
fn collections_abc_and_pathlib_imports_erase() {
    let src = r#"
from collections.abc import Mapping, Sequence
from pathlib import Path

def id_path(p: Path) -> Path:
    return p
"#;
    let (report, _rust) = transpile_source(src, "erase_more.py", None).unwrap();
    assert!(
        report
            .emitted_items
            .iter()
            .any(|n| n.starts_with("#erase:")),
        "type-only imports should erase: {:?}",
        report.emitted_items
    );
    assert!(
        !report.gaps.iter().any(|g| g.category == Category::Import),
        "collections.abc / pathlib must not Import-gap: {:?}",
        report.gaps
    );
}

/// Issue #51: module-level literal assignments lower to Rust `const` items.
#[test]
fn module_level_literal_assign_emits_const() {
    let src = r#"
x = 1
s = "hi"
b = True
f = 1.5
COUNT: int = 42
NAME: str = "ok"
x_dyn = unknown()
"#;
    let (report, rust) = transpile_source(src, "mod_lit.py", Some("mod_lit")).unwrap();
    assert!(report.never_silent_holds());
    for name in ["x", "s", "b", "f", "COUNT", "NAME"] {
        assert!(
            report.emitted_items.iter().any(|n| n == name),
            "expected const emit for {name}: {:?}",
            report.emitted_items
        );
    }
    assert!(rust.contains("const x: i64 = 1;"), "rust:\n{rust}");
    assert!(rust.contains("const s: &str = \"hi\";"), "rust:\n{rust}");
    assert!(rust.contains("const b: bool = true;"), "rust:\n{rust}");
    assert!(rust.contains("const COUNT: i64 = 42;"), "rust:\n{rust}");
    assert!(rust.contains("const NAME: &str = \"ok\";"), "rust:\n{rust}");
    // Non-literal stays honest DynamicTyping gap.
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::DynamicTyping
                && g.item_name.as_deref() == Some("x_dyn")),
        "non-literal must DynamicTyping-gap: {:?}",
        report.gaps
    );
}

#[test]
fn simple_call_and_attr_lower() {
    // Use mapped types only — DynamicTyping is out of scope for #50.
    let src = r#"
def get_x(p: list[int]) -> int:
    return p.x

def call_f(x: int) -> int:
    return f(x)

def method_call(p: list[int], n: int) -> int:
    return p.get(n)
"#;
    let (report, rust) = transpile_source(src, "call_attr.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "simple call/attr must not FunctionBody-gap: {:?}",
        report.gaps
    );
    for name in ["get_x", "call_f", "method_call"] {
        assert!(
            report.emitted_items.iter().any(|n| n == name),
            "expected {name}: {:?}",
            report.emitted_items
        );
    }
    assert!(rust.contains("p.x"), "attr: {rust}");
    assert!(rust.contains("f(x)"), "call: {rust}");
    assert!(rust.contains("p.get(n)"), "method: {rust}");
}

#[test]
fn call_with_kwargs_declines() {
    let src = r#"
def g(x: int) -> int:
    return f(x, y=1)
"#;
    let (report, _) = transpile_source(src, "kwargs.py", None).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "kwargs call must FunctionBody-gap: {:?}",
        report.gaps
    );
}

#[test]
fn starargs_call_declines() {
    let src = r#"
def g(xs: list[int]) -> int:
    return f(*xs)
"#;
    let (report, _) = transpile_source(src, "star.py", None).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "starargs must FunctionBody-gap: {:?}",
        report.gaps
    );
}

#[test]
fn free_range_call_declines() {
    let src = r#"
def g(n: int) -> int:
    return range(n)
"#;
    let (report, rust) = transpile_source(src, "free_range.py", None).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "free range must FunctionBody-gap: {:?}",
        report.gaps
    );
    // Body should be todo, not a free `range(` call claiming validity.
    assert!(
        rust.contains("todo!"),
        "expected FunctionBody stub, got:\n{rust}"
    );
}

#[test]
fn simple_list_comprehension_lowers() {
    let src = r#"
def map_double(xs: list[int]) -> list[int]:
    return [x * 2 for x in xs]

def filter_pos(xs: list[int]) -> list[int]:
    return [x for x in xs if x > 0]

def identity(xs: list[int]) -> list[int]:
    return [x for x in xs]
"#;
    let (report, rust) = transpile_source(src, "compr.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "simple list comps must not FunctionBody-gap: {:?}",
        report.gaps
    );
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::Comprehension),
        "successfully lowered list comps must not leave Comprehension gap: {:?}",
        report.gaps
    );
    assert!(
        rust.contains("xs.into_iter().map(|x| (x * 2)).collect::<Vec<_>>()"),
        "map path: {rust}"
    );
    assert!(
        rust.contains("xs.into_iter().filter(|x| (x > 0)).collect::<Vec<_>>()"),
        "filter identity path: {rust}"
    );
    // identity: collect only, no map
    assert!(
        rust.contains("xs.into_iter().collect::<Vec<_>>()"),
        "identity collect: {rust}"
    );
    // Isolate identity function body — must not introduce map for bare x
    let id_start = rust.find("fn identity").expect("identity fn");
    let id_body = &rust[id_start..];
    assert!(
        !id_body.contains(".map("),
        "identity must skip map: {id_body}"
    );
}

#[test]
fn nested_list_comprehension_declines() {
    let src = r#"
def nest(xss: list[list[int]]) -> list[int]:
    return [x for xs in xss for x in xs]
"#;
    let (report, _) = transpile_source(src, "nest.py", None).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "nested gens must FunctionBody-gap: {:?}",
        report.gaps
    );
}

#[test]
fn simple_set_comprehension_lowers() {
    let src = r#"
def squares(xs: list[int]) -> set[int]:
    return {x * x for x in xs}

def filter_pos(xs: list[int]) -> set[int]:
    return {x for x in xs if x > 0}
"#;
    let (report, rust) = transpile_source(src, "setcompr.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "simple set comps must not FunctionBody-gap: {:?}",
        report.gaps
    );
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::Comprehension),
        "successfully lowered set comps must not leave Comprehension gap: {:?}",
        report.gaps
    );
    assert!(
        rust.contains("xs.into_iter().map(|x| (x * x)).collect::<std::collections::HashSet<_>>()"),
        "map path: {rust}"
    );
    assert!(
        rust.contains(
            "xs.into_iter().filter(|x| (x > 0)).collect::<std::collections::HashSet<_>>()"
        ),
        "filter identity path: {rust}"
    );
}

#[test]
fn simple_dict_comprehension_lowers() {
    let src = r#"
def double_map(xs: list[int]) -> dict[int, int]:
    return {x: x * 2 for x in xs}

def filtered_map(xs: list[int]) -> dict[int, int]:
    return {x: x for x in xs if x > 0}
"#;
    let (report, rust) = transpile_source(src, "dictcompr.py", None).unwrap();
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "simple dict comps must not FunctionBody-gap: {:?}",
        report.gaps
    );
    assert!(
        !report
            .gaps
            .iter()
            .any(|g| g.category == Category::Comprehension),
        "successfully lowered dict comps must not leave Comprehension gap: {:?}",
        report.gaps
    );
    assert!(
        rust.contains(
            "xs.into_iter().map(|x| (x, (x * 2))).collect::<std::collections::HashMap<_, _>>()"
        ),
        "map path: {rust}"
    );
    assert!(
        rust.contains(
            "xs.into_iter().filter(|x| (x > 0)).map(|x| (x, x)).collect::<std::collections::HashMap<_, _>>()"
        ),
        "filter path: {rust}"
    );
}

#[test]
fn nested_set_and_dict_comprehensions_decline() {
    let src = r#"
def nest_set(xss: list[list[int]]) -> set[int]:
    return {x for xs in xss for x in xs}

def nest_dict(xss: list[list[int]]) -> dict[int, int]:
    return {x: x for xs in xss for x in xs}
"#;
    let (report, _) = transpile_source(src, "nestcompr.py", None).unwrap();
    let function_body_fns: std::collections::BTreeSet<_> = report
        .gaps
        .iter()
        .filter(|g| g.category == Category::FunctionBody)
        .filter_map(|g| g.item_name.clone())
        .collect();
    assert!(
        function_body_fns.contains("nest_set") && function_body_fns.contains("nest_dict"),
        "nested gens must FunctionBody-gap both fns: {:?}",
        report.gaps
    );
}

#[test]
fn dict_comprehension_walrus_declines() {
    // Walrus inside a comprehension is intentionally out of scope: `lower_simple_expr_in`
    // has no NamedExpr arm, so it falls through to None and the whole comp gaps honestly.
    let src = r#"
def f(xs: list[int]) -> dict[int, int]:
    return {x: y for x in xs if (y := x * 2) > 0}
"#;
    let (report, _) = transpile_source(src, "walrus.py", None).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::FunctionBody),
        "walrus in comprehension filter must still gap honestly: {:?}",
        report.gaps
    );
}
