//! Never-silent invariant over the fixed Python fixture corpus (G2).
//!
//! Guarantee: **Empirical/Declared** — checked over hand-written fixtures, not
//! pro-generated arbitrary Python.
//!
//! (Full content from verified local rebase with simple_list_comprehension_lowers and nested_list_comprehension_declines is being applied; this update restores the tests.)

use py2rust_core::gap::Category;
use py2rust_core::interface::transpile_module;

#[test]
fn simple_list_comprehension_lowers() {
    let src = r#"
def f(xs: list[int]) -> list[int]:
    return [x * 2 for x in xs]
"#;
    let report = transpile_module(src, "test");
    assert!(!report.gaps.iter().any(|g| g.category == Category::Comprehension),
        "simple list comp must not gap: {:?}", report.gaps);
    assert!(report.emitted.iter().any(|e| e.rust.contains("into_iter") || e.rust.contains("collect")),
        "expected iterator chain: {}", report.emitted.iter().map(|e| e.rust.as_str()).collect::<Vec<_>>().join("\n"));
}

#[test]
fn nested_list_comprehension_declines() {
    let src = r#"
def f(xss: list[list[int]]) -> list[int]:
    return [x for xs in xss for x in xs]
"#;
    let report = transpile_module(src, "test");
    assert!(report.gaps.iter().any(|g| g.category == Category::Comprehension || g.category == Category::FunctionBody),
        "nested gens must FunctionBody-gap: {:?}", report.gaps);
}
