//! Numeric literals must lower to the type their context demands.
//!
//! Python treats `48` and `48.0` as interchangeable in a float context. Rust does
//! not, and every gate below L3 was blind to the difference: the emitter happily
//! produced `window_hours <= 48` against an `f64` parameter, recorded no gap, and
//! counted the module as successfully emitted. Only handing the result to `rustc`
//! surfaced it.
//!
//! So these tests assert on **what `rustc` says**, not on substrings. A test that
//! checks for `"48.0"` passes when the emitter writes `48.0` into code that does
//! not compile for some other reason; a test that compiles the output cannot.
//! Substring assertions appear only where the point is the *shape* of the
//! emission rather than its validity.

use py2rust_core::{transpile_source, Checker};

/// One lowering case: Python in, a required fragment of Rust out.
struct Case {
    name: &'static str,
    py: &'static str,
    /// Must appear in the emitted Rust.
    wants: &'static str,
    /// Must NOT appear — the pre-fix output, so a regression is caught by the
    /// exact string it would produce.
    rejects: &'static str,
}

const CASES: &[Case] = &[
    // The one found in the wild: lib/metrics_agg.py::_bucket_size_seconds.
    Case {
        name: "int literal compared against a float parameter",
        py: "def f(h: float) -> int:\n    return 3600 if h <= 48 else 86400\n",
        wants: "(h <= 48.0)",
        rejects: "(h <= 48)",
    },
    Case {
        name: "int literal in float arithmetic",
        py: "def f(x: float) -> float:\n    return x * 2\n",
        wants: "(x * 2.0)",
        rejects: "(x * 2)",
    },
    Case {
        name: "int literal returned from a float function",
        py: "def f() -> float:\n    return 3600\n",
        wants: "3600.0",
        rejects: "    3600\n",
    },
    // A whole-number float renders as `1` under `{}`, which is an *integer*
    // literal in Rust — so `return 1.0` from `-> float` did not compile either.
    Case {
        name: "whole-number float keeps its point",
        py: "def f() -> float:\n    return 1.0\n",
        wants: "1.0",
        rejects: "    1\n",
    },
    // Both arms of a conditional are one Rust type.
    Case {
        name: "conditional arms agree with the float return type",
        py: "def f(b: bool) -> float:\n    return 1 if b else 2\n",
        wants: "{ 1.0 } else { 2.0 }",
        rejects: "{ 1 } else { 2 }",
    },
    // Integers must stay integers. Over-eager float promotion would be a
    // different silent wrongness, not a fix.
    Case {
        name: "integer context is left alone",
        py: "def f(n: int) -> int:\n    return n + 1\n",
        wants: "(n + 1)",
        rejects: "1.0",
    },
];

fn emit(py: &str) -> String {
    let (_, rust) = transpile_source(py, "t.py", Some("t")).expect("parses");
    rust
}

#[test]
fn literals_take_the_type_their_context_requires() {
    for c in CASES {
        let rust = emit(c.py);
        assert!(
            rust.contains(c.wants),
            "case {}: expected {:?} in\n{rust}",
            c.name,
            c.wants
        );
        assert!(
            !rust.contains(c.rejects),
            "case {}: {:?} is the pre-fix output and must not reappear in\n{rust}",
            c.name,
            c.rejects
        );
    }
}

#[test]
fn every_case_actually_compiles() {
    // The assertion that matters. Skipped rather than failed without a
    // toolchain: an environment with no `rustc` must not manufacture a red test
    // for a defect it never observed.
    let Ok(checker) = Checker::probe() else {
        eprintln!("no rustc — numeric lowering compile check skipped");
        return;
    };
    let dir = std::env::temp_dir().join(format!("py2rust-num-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    for (i, c) in CASES.iter().enumerate() {
        let rust = emit(c.py);
        let path = dir.join(format!("case{i}.rs"));
        std::fs::write(&path, &rust).unwrap();
        // 0 declared unlowered bodies: every case here lowers fully, and if one
        // stops doing so the emitted `todo!()` makes that visible.
        let r = checker.check(&path, 0);
        assert!(
            r.compiles(),
            "case {}: emitted Rust does not compile — {:?}\n{rust}",
            c.name,
            r.errors
        );
        assert!(
            r.compiles_without_stubs(),
            "case {}: expected a full lowering, got scaffolding\n{rust}",
            c.name
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_operand_types_decline_rather_than_guess() {
    // An unannotated parameter is a placeholder type, deliberately absent from
    // the type environment. Inference must return "unknown" and leave the
    // literal alone — promoting it on a guess is the silent inference this
    // project exists to refuse.
    let rust = emit("def f(x) -> int:\n    return x + 1\n");
    assert!(
        rust.contains("(x + 1)"),
        "literal must be left alone when the other operand's type is unknown:\n{rust}"
    );
    assert!(!rust.contains("1.0"));
}
