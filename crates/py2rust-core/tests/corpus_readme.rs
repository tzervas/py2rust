//! README limitations → product behavior: each limitation is a Category flag, not a silent pass.

use py2rust_core::{analyze_source, transpile_source, Category};

#[test]
fn readme_classes_and_inheritance_are_class_gaps() {
    let src = "class Base: pass\nclass Child(Base): pass\n";
    let report = analyze_source(src, "classes.py").unwrap();
    assert_eq!(report.total_top_level_items, 2);
    assert!(report.gaps.iter().all(|g| g.category == Category::Class));
    assert!(report.emitted_items.is_empty());
}

#[test]
fn readme_exception_handling_is_exception_gap() {
    let src = "try:\n    x = 1\nexcept Exception:\n    x = 0\n";
    let report = analyze_source(src, "exc.py").unwrap();
    assert!(report
        .gaps
        .iter()
        .any(|g| g.category == Category::Exception));
}

#[test]
fn readme_dynamic_typing_when_types_missing() {
    let src = "def f(x):\n    return x\n";
    let (report, _) = transpile_source(src, "dyn.py", None).unwrap();
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.category == Category::DynamicTyping),
        "missing annotations must flag DynamicTyping: {:?}",
        report.gaps
    );
}

#[test]
fn readme_metaprogramming_decorators_and_eval() {
    let deco = "@cache\ndef g(x: int) -> int:\n    return x\n";
    let report = analyze_source(deco, "deco.py").unwrap();
    assert!(report
        .gaps
        .iter()
        .any(|g| g.category == Category::Metaprogramming));

    let ev = "eval('1')\n";
    let report = analyze_source(ev, "eval.py").unwrap();
    assert!(report
        .gaps
        .iter()
        .any(|g| g.category == Category::Metaprogramming));
}

#[test]
fn coverage_bound_emitted_plus_gaps() {
    let src = "import sys\ndef ok(a: int) -> int:\n    return a\nclass C: pass\n";
    let report = analyze_source(src, "cov.py").unwrap();
    assert!(report.emitted_items.len() + report.gaps.len() >= report.total_top_level_items);
}
