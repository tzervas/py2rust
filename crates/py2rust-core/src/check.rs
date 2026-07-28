//! L3 — does the emitted Rust actually compile?
//!
//! L0–L2 all measure the same thing from different angles: how much Python we
//! *claimed* to lower. None of them asks the only question a consumer of the
//! output cares about — does `rustc` accept it. A transpiler can report 100%
//! statement coverage and emit a file that does not compile, and every number
//! above L3 would still look good.
//!
//! # Two numbers, because one of them flatters
//!
//! The emitter's fallback for an unlowered body is `todo!(…)`, which compiles.
//! So "the emitted Rust compiles" is trivially satisfiable by emitting nothing
//! but signatures — exactly what the corpus does today. Reporting compile-pass
//! alone would be the L1 mistake again in a new costume.
//!
//! This module therefore records two facts per file:
//!
//! - **compiles** — `rustc` accepted it.
//! - **unlowered bodies** — measured twice, once by reading the emitted text for
//!   `todo!()` markers and once from the `FunctionBody` gaps the transpiler
//!   recorded. See [`CheckResult::unlowered_bodies`] for why both.
//!
//! and the reports lead with *compiles **and** has no unlowered body*. A file
//! that compiles with 40 `todo!()`s is a header file, not a port.
//!
//! # Not-run is not failure
//!
//! If `rustc` is absent or unrunnable, every result is [`L3Status::NotRun`] with
//! a reason, and callers must render that as "not measured". A missing toolchain
//! rendering as 0% passing is the same class of bug as a blind `/proc` rendering
//! as "no sockets found".
//!
//! # Why `rustc` and not `cargo check`
//!
//! Emitted modules are dependency-free single files. Driving `cargo` would mean
//! synthesising a package per module, and one module's failure would abort the
//! build for the rest — losing exactly the per-module verdict this exists to
//! produce. `rustc --emit=metadata` per file is isolated, faster, and gives
//! machine-readable diagnostics via `--error-format=json`.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Markers the emitter uses where a body was not lowered. Counted, not matched
/// on for control flow: these compile, which is the whole problem.
const STUB_MARKERS: [&str; 2] = ["todo!(", "unimplemented!("];

/// Outcome of asking `rustc` about one emitted file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum L3Status {
    /// Never asked — no toolchain, or the file was never emitted. **Not** a failure.
    NotRun,
    /// `rustc` accepted it. Says nothing about whether it *does* anything; see
    /// [`CheckResult::unlowered_bodies`].
    Passed,
    /// `rustc` rejected it.
    Failed,
}

/// One `rustc` error, reduced to the parts that are stable across runs.
///
/// Deliberately drops `rendered` and the file path: both embed the absolute
/// output path, which varies per run and would make two reports of identical
/// input diff. Code + message + line is enough to rank and to find.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RustcDiag {
    /// e.g. `E0425`. `None` for errors rustc does not code, such as parse errors.
    pub code: Option<String>,
    pub message: String,
    pub line: usize,
}

/// Per-file L3 verdict.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub status: L3Status,
    /// Populated only for [`L3Status::NotRun`], and always populated there — a
    /// not-run with no reason is indistinguishable from a silent skip.
    pub not_run_reason: Option<String>,
    pub errors: Vec<RustcDiag>,
    /// `todo!()` / `unimplemented!()` markers found by reading the emitted text.
    pub stub_markers: usize,
    /// `FunctionBody` gaps the transpiler recorded for this file — the same
    /// quantity, arrived at from the other direction.
    pub declared_unlowered: usize,
}

impl CheckResult {
    /// A file we never asked about. Reason is required by the signature.
    pub fn not_run(reason: impl Into<String>, stub_markers: usize, declared: usize) -> Self {
        Self {
            status: L3Status::NotRun,
            not_run_reason: Some(reason.into()),
            errors: Vec::new(),
            stub_markers,
            declared_unlowered: declared,
        }
    }

    pub fn compiles(&self) -> bool {
        self.status == L3Status::Passed
    }

    /// Unlowered bodies: the **larger** of the two measurements.
    ///
    /// Two independent routes to the same number — what the emitter recorded as
    /// a gap, and what is actually in the file — so a disagreement means one of
    /// them missed something, and taking the max fails toward "less credit".
    ///
    /// Not hypothetical. The emitter used to give unit-returning functions an
    /// empty body rather than a `todo!()`, so the text scan saw nothing while
    /// the gap report saw everything; eleven `tg-agent-relay` modules of pure
    /// scaffolding were scored as fully lowered ports. The emitter now always
    /// writes `todo!()` — this stays because the next such divergence should
    /// cost accuracy, not silence.
    pub fn unlowered_bodies(&self) -> usize {
        self.stub_markers.max(self.declared_unlowered)
    }

    /// True when the two measurements disagree — worth surfacing, since one of
    /// them is then known to be wrong.
    pub fn measurements_disagree(&self) -> bool {
        self.stub_markers != self.declared_unlowered
    }

    /// Compiles **and** carries no unlowered body by either measure.
    ///
    /// This is the number worth steering by. [`Self::compiles`] alone is
    /// satisfied by a file of signatures whose every body is `todo!()`.
    pub fn compiles_without_stubs(&self) -> bool {
        self.compiles() && self.unlowered_bodies() == 0
    }
}

/// A probed Rust toolchain. Construct once per batch; probing per file would
/// spawn a process per file just to learn the same answer.
#[derive(Debug)]
pub struct Checker {
    edition: String,
    scratch: PathBuf,
}

impl Checker {
    /// Probe the toolchain.
    ///
    /// `Err(reason)` means L3 cannot be measured on this machine. Callers must
    /// carry that reason through to the report rather than substituting a zero.
    pub fn probe() -> Result<Self, String> {
        let out = Command::new("rustc")
            .arg("--version")
            .output()
            .map_err(|e| format!("rustc could not be run: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "`rustc --version` exited {}",
                out.status.code().unwrap_or(-1)
            ));
        }
        let scratch = std::env::temp_dir().join(format!("py2rust-l3-{}", std::process::id()));
        std::fs::create_dir_all(&scratch)
            .map_err(|e| format!("cannot create L3 scratch dir {}: {e}", scratch.display()))?;
        Ok(Self {
            edition: "2021".to_string(),
            scratch,
        })
    }

    /// Ask `rustc` about one emitted file.
    ///
    /// Never panics and never returns an error: a harness failure becomes
    /// `NotRun` with the cause, because reporting it as `Failed` would blame the
    /// emitted code for the harness.
    /// `declared_unlowered` is the caller's own count of `FunctionBody` gaps for
    /// this file. Required rather than optional: without it there is only one
    /// measurement, and a single measurement of "did this really lower" is the
    /// one that was wrong.
    pub fn check(&self, rust_path: &Path, declared_unlowered: usize) -> CheckResult {
        let stub_markers = count_stub_bodies(rust_path);
        // Fixed crate name. Left to rustc it is derived from the file stem, so a
        // module named `my-thing.py` would fail on the *name*, not on the code —
        // a harness artifact reported as a transpiler defect.
        let meta = self.scratch.join("l3.rmeta");
        let out = Command::new("rustc")
            .args([
                "--edition",
                &self.edition,
                "--crate-type",
                "lib",
                "--crate-name",
                "py2rust_l3",
                "--emit",
                "metadata",
                "--error-format",
                "json",
                "-o",
            ])
            .arg(&meta)
            .arg(rust_path)
            .output();

        let out = match out {
            Ok(o) => o,
            Err(e) => {
                return CheckResult::not_run(
                    format!("rustc invocation failed: {e}"),
                    stub_markers,
                    declared_unlowered,
                )
            }
        };
        let errors = parse_diagnostics(&String::from_utf8_lossy(&out.stderr));

        // Trust the exit status for pass/fail, not the diagnostic count: a
        // future rustc that fails without an `error`-level line must not read as
        // a pass.
        if out.status.success() {
            CheckResult {
                status: L3Status::Passed,
                not_run_reason: None,
                errors: Vec::new(),
                stub_markers,
                declared_unlowered,
            }
        } else if errors.is_empty() {
            CheckResult::not_run(
                format!(
                    "rustc exited {} but emitted no parseable diagnostics",
                    out.status.code().unwrap_or(-1)
                ),
                stub_markers,
                declared_unlowered,
            )
        } else {
            CheckResult {
                status: L3Status::Failed,
                not_run_reason: None,
                errors,
                stub_markers,
                declared_unlowered,
            }
        }
    }
}

impl Drop for Checker {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.scratch);
    }
}

/// Count unlowered-body markers in an emitted file.
///
/// An unreadable file yields 0, which is the conservative direction: it makes
/// the "compiles without stubs" figure *more* generous, so it can never hide a
/// stub that is actually there behind an I/O error.
pub fn count_stub_bodies(rust_path: &Path) -> usize {
    let Ok(src) = std::fs::read_to_string(rust_path) else {
        return 0;
    };
    STUB_MARKERS.iter().map(|m| src.matches(m).count()).sum()
}

/// Extract error-level diagnostics from `rustc --error-format=json` output.
///
/// Skips rustc's own trailers ("aborting due to N previous errors", "For more
/// information…") — those are summaries of the list, and counting them would
/// inflate every error total by two.
fn parse_diagnostics(stderr: &str) -> Vec<RustcDiag> {
    let mut out = Vec::new();
    for line in stderr.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("level").and_then(|l| l.as_str()) != Some("error") {
            continue;
        }
        let message = v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_default()
            .to_string();
        if message.starts_with("aborting due to") || message.starts_with("For more information") {
            continue;
        }
        let code = v
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .map(str::to_string);
        let line_no = v
            .get("spans")
            .and_then(|s| s.as_array())
            .and_then(|s| s.first())
            .and_then(|s| s.get("line_start"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0) as usize;
        out.push(RustcDiag {
            code,
            message,
            line: line_no,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture: one rustc JSON line, built from parts so the tests stay readable.
    fn diag_line(level: &str, code: Option<&str>, message: &str, line: usize) -> String {
        let code_json = match code {
            Some(c) => format!(r#"{{"code":"{c}","explanation":null}}"#),
            None => "null".into(),
        };
        format!(
            r#"{{"$message_type":"diagnostic","message":"{message}","code":{code_json},"level":"{level}","spans":[{{"file_name":"/abs/x.rs","line_start":{line}}}],"children":[]}}"#
        )
    }

    #[test]
    fn parses_errors_and_ignores_warnings_and_trailers() {
        let stderr = [
            diag_line("warning", Some("dead_code"), "never used", 4),
            diag_line("error", Some("E0425"), "cannot find value `y`", 2),
            diag_line("error", None, "expected `;`", 7),
            diag_line("error", None, "aborting due to 2 previous errors", 0),
            diag_line("failure-note", None, "For more information", 0),
        ]
        .join("\n");
        let got = parse_diagnostics(&stderr);
        assert_eq!(
            got,
            vec![
                RustcDiag {
                    code: Some("E0425".into()),
                    message: "cannot find value `y`".into(),
                    line: 2
                },
                RustcDiag {
                    code: None,
                    message: "expected `;`".into(),
                    line: 7
                },
            ]
        );
    }

    #[test]
    fn diagnostics_carry_no_paths_so_reports_are_reproducible() {
        // The span's file_name is an absolute path that differs per run. If it
        // leaked into the record, two runs over identical input would diff.
        let got = parse_diagnostics(&diag_line("error", Some("E0308"), "mismatched types", 3));
        let json = serde_json::to_string(&got).unwrap();
        assert!(
            !json.contains("/abs/"),
            "path leaked into the record: {json}"
        );
    }

    #[test]
    fn non_json_noise_is_skipped_not_fatal() {
        let stderr = format!(
            "warning: some plain-text line\n{}\nnot json at all\n",
            diag_line("error", Some("E0433"), "failed to resolve", 1)
        );
        assert_eq!(parse_diagnostics(&stderr).len(), 1);
    }

    /// Cases for stub counting: (name, source, expected).
    const STUB_CASES: &[(&str, &str, usize)] = &[
        ("none", "fn f() -> i64 { 1 }\n", 0),
        ("one_todo", "fn f() {\n    todo!(\"body\")\n}\n", 1),
        (
            "mixed",
            "fn a() { todo!(\"x\") }\nfn b() { unimplemented!() }\nfn c() { todo!() }\n",
            3,
        ),
    ];

    #[test]
    fn stub_bodies_are_counted() {
        let dir = std::env::temp_dir().join(format!("py2rust-stub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, src, want) in STUB_CASES {
            let p = dir.join(format!("{name}.rs"));
            std::fs::write(&p, src).unwrap();
            assert_eq!(count_stub_bodies(&p), *want, "case {name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_counts_zero_stubs_rather_than_panicking() {
        assert_eq!(count_stub_bodies(Path::new("/no/such/file.rs")), 0);
    }

    #[test]
    fn not_run_always_carries_a_reason() {
        let r = CheckResult::not_run("no toolchain", 3, 3);
        assert_eq!(r.status, L3Status::NotRun);
        assert!(r.not_run_reason.is_some());
        assert!(!r.compiles(), "not-run must never read as a pass");
        assert!(!r.compiles_without_stubs());
    }

    fn passed(stub_markers: usize, declared: usize) -> CheckResult {
        CheckResult {
            status: L3Status::Passed,
            not_run_reason: None,
            errors: Vec::new(),
            stub_markers,
            declared_unlowered: declared,
        }
    }

    /// (name, markers found in text, FunctionBody gaps declared, unlowered, is a port)
    const CREDIT_CASES: &[(&str, usize, usize, usize, bool)] = &[
        ("fully lowered — both agree on nothing", 0, 0, 0, true),
        ("stubs — both agree", 4, 4, 4, false),
        // The regression that made eleven scaffolding modules read as ports: a
        // unit-returning function's unlowered body left no marker in the text.
        ("text blind, gaps saw it", 0, 3, 3, false),
        // The mirror case: a `todo!()` the emitter forgot to record as a gap.
        ("gaps blind, text saw it", 2, 0, 2, false),
    ];

    #[test]
    fn credit_for_a_port_requires_both_measurements_to_agree_on_zero() {
        for (name, markers, declared, want_unlowered, want_port) in CREDIT_CASES {
            let r = passed(*markers, *declared);
            assert!(r.compiles(), "case {name}");
            assert_eq!(r.unlowered_bodies(), *want_unlowered, "case {name}");
            assert_eq!(
                r.compiles_without_stubs(),
                *want_port,
                "case {name}: a module only counts as a port when neither \
                 measurement finds an unlowered body"
            );
            assert_eq!(
                r.measurements_disagree(),
                markers != declared,
                "case {name}"
            );
        }
    }

    #[test]
    fn real_rustc_accepts_emitted_shape_and_rejects_broken_rust() {
        // Skipped rather than failed where there is no toolchain: an environment
        // without rustc must not manufacture a red test.
        let Ok(checker) = Checker::probe() else {
            eprintln!("no rustc — L3 end-to-end check skipped");
            return;
        };
        let dir = std::env::temp_dir().join(format!("py2rust-l3t-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Shape the emitter actually produces, including a name rustc would
        // reject as a crate name.
        let good = dir.join("my-mod.rs");
        std::fs::write(
            &good,
            "fn add(a: i64, b: i64) -> i64 {\n    todo!(\"py2rust: body not lowered\")\n}\n",
        )
        .unwrap();
        let r = checker.check(&good, 1);
        assert!(r.compiles(), "emitted shape must compile: {:?}", r.errors);
        assert_eq!(r.stub_markers, 1);
        assert!(
            !r.compiles_without_stubs(),
            "and must not be counted as a real port"
        );

        // Keyword function name: `def true(...)` is ordinary Python, and this is
        // the raw-identifier lowering that L3 forced onto the emitter.
        let kw = dir.join("kw.rs");
        std::fs::write(&kw, "fn r#true(r#match: bool) {\n    todo!()\n}\n").unwrap();
        assert!(
            checker.check(&kw, 1).compiles(),
            "raw identifiers must be accepted — this is the fix for `def true()`"
        );

        let bad = dir.join("bad.rs");
        std::fs::write(&bad, "fn f() -> i64 {\n    y + 1\n}\n").unwrap();
        let r = checker.check(&bad, 0);
        assert_eq!(r.status, L3Status::Failed);
        assert!(
            r.errors.iter().any(|e| e.code.as_deref() == Some("E0425")),
            "expected an unresolved-name error, got {:?}",
            r.errors
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
