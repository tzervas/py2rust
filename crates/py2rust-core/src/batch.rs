//! Multi-file batch helpers (summary + union gap report).
//!
//! Shape inspired by `research/mycelium-transpile-snapshot/src/batch.rs`.

use crate::check::{CheckResult, Checker, L3Status, RustcDiag};
use crate::dispatch;
use crate::gap::{Gap, GapReport};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Knobs for a batch run.
///
/// A struct rather than more positional parameters, because the next two gates
/// (L4 behaviour, L5 clippy) belong here too and each would otherwise be another
/// breaking signature change.
#[derive(Debug, Clone, Default)]
pub struct BatchOptions {
    /// Run the L3 gate: hand every emitted `.rs` to `rustc` and record whether
    /// it compiles. Off by default because it spawns a process per file; the
    /// CLI turns it on, since an uncompiled emission is not an emission.
    pub check_l3: bool,
}

/// Discover `*.py` files under `root` (sorted, skips `__pycache__` and `.venv`).
pub fn discover_py_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if matches!(name, "__pycache__" | ".venv" | "venv" | ".git" | "target") {
                    continue;
                }
                stack.push(path);
            } else if ft.is_file() && path.extension().and_then(|e| e.to_str()) == Some("py") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Per-file batch result.
#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub source: String,
    pub rust_path: Option<String>,
    pub gap_path: Option<String>,
    pub emitted: usize,
    pub gaps: usize,
    pub total_top_level: usize,
    pub expressible_fraction: f64,
    /// L2 denominator: every statement, nested included.
    pub total_statements: usize,
    /// L2 numerator: statements for which Rust was emitted.
    pub lowered_statements: usize,
    /// L3: did `rustc` accept the emitted file? `NotRun` when the gate was off
    /// or the toolchain was missing — never conflated with `Failed`.
    pub l3_status: L3Status,
    pub l3_not_run_reason: Option<String>,
    pub l3_errors: Vec<RustcDiag>,
    /// Unlowered bodies in the emitted file, by the larger of the two
    /// measurements. A file of these compiles, so L3 pass alone says nothing
    /// without this number beside it.
    pub unlowered_bodies: usize,
    pub error: Option<String>,
}

impl FileResult {
    fn with_check(mut self, c: CheckResult) -> Self {
        self.l3_status = c.status;
        self.unlowered_bodies = c.unlowered_bodies();
        self.l3_not_run_reason = c.not_run_reason;
        self.l3_errors = c.errors;
        self
    }
}

/// Aggregate batch summary.
#[derive(Debug, Clone, Serialize)]
pub struct BatchSummary {
    pub files: Vec<FileResult>,
    pub total_files: usize,
    pub ok_files: usize,
    pub total_emitted: usize,
    pub total_gaps: usize,
    pub total_top_level: usize,
    pub total_statements: usize,
    pub lowered_statements: usize,
    /// Files actually handed to `rustc` — those that emitted at least one item.
    /// Zero means L3 was not measured, which is not the same as zero files
    /// compiling, and the reports must not say so.
    pub l3_checked: usize,
    pub l3_passed: usize,
    pub l3_failed: usize,
    /// Passed **and** free of unlowered bodies. The honest L3 numerator.
    pub l3_passed_without_stubs: usize,
    /// Unlowered bodies across the corpus (max of the two measurements).
    pub total_unlowered_bodies: usize,
    /// The subset of those that actually left a `todo!()` in the emitted text.
    pub total_stub_markers: usize,
    /// Files where the two measurements disagreed. Non-zero means one of them is
    /// missing something and the emitter deserves a look.
    pub l3_measurement_disagreements: usize,
    /// Error codes across the corpus, for ranking what to fix in the emitter.
    /// Uncoded errors (parse errors) bucket under `"<uncoded>"`.
    pub l3_error_codes: BTreeMap<String, usize>,
    /// Why L3 produced no measurement, when it produced none.
    pub l3_not_run_reason: Option<String>,
}

/// Union of all gaps across a batch (for backlog ranking).
#[derive(Debug, Clone, Serialize)]
pub struct UnionGapReport {
    pub gaps: Vec<Gap>,
    pub category_counts: BTreeMap<&'static str, usize>,
    pub file_count: usize,
}

impl UnionGapReport {
    pub fn from_reports(reports: &[GapReport]) -> Self {
        let mut gaps = Vec::new();
        let mut category_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        for r in reports {
            for g in &r.gaps {
                *category_counts.entry(g.category.as_str()).or_insert(0) += 1;
                gaps.push(g.clone());
            }
        }
        Self {
            gaps,
            category_counts,
            file_count: reports.len(),
        }
    }
}

/// Transpile each `.py` under `root` into `out_dir`, writing `.rs` + `.gap.json`.
///
/// L3 off. See [`transpile_batch_with`].
pub fn transpile_batch(
    root: &Path,
    out_dir: &Path,
) -> std::io::Result<(BatchSummary, UnionGapReport)> {
    transpile_batch_with(root, out_dir, &BatchOptions::default())
}

/// Transpile each `.py` under `root` into `out_dir`, writing `.rs` + `.gap.json`,
/// and optionally running the L3 compile gate over the result.
///
/// The toolchain is probed **once**, before the loop. A probe failure disables
/// the gate for the whole run and is recorded as a reason on every file and on
/// the summary — so the report can say "not measured, because X" rather than
/// silently reporting that nothing compiled.
pub fn transpile_batch_with(
    root: &Path,
    out_dir: &Path,
    opts: &BatchOptions,
) -> std::io::Result<(BatchSummary, UnionGapReport)> {
    std::fs::create_dir_all(out_dir)?;
    let files = discover_py_files(root)?;

    let (checker, l3_off_reason) = if opts.check_l3 {
        match Checker::probe() {
            Ok(c) => (Some(c), None),
            Err(why) => (None, Some(format!("toolchain unavailable: {why}"))),
        }
    } else {
        (None, Some("L3 gate not requested for this run".to_string()))
    };
    let mut file_results = Vec::new();
    let mut reports = Vec::new();
    let mut total_emitted = 0usize;
    let mut total_gaps = 0usize;
    let mut total_top = 0usize;
    let mut total_stmts = 0usize;
    let mut lowered_stmts = 0usize;
    let mut ok = 0usize;
    let mut l3_checked = 0usize;
    let mut l3_passed = 0usize;
    let mut l3_failed = 0usize;
    let mut l3_clean = 0usize;
    let mut total_unlowered = 0usize;
    let mut total_markers = 0usize;
    let mut l3_disagreements = 0usize;
    let mut l3_codes: BTreeMap<String, usize> = BTreeMap::new();

    for path in &files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .with_extension("");
        let rust_path = out_dir.join(&rel).with_extension("rs");
        // foo.rs → foo.gap.json (Path::with_extension replaces final extension)
        let gap_path = {
            let mut p = rust_path.clone();
            p.set_extension("gap.json");
            p
        };
        if let Some(parent) = rust_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        match dispatch::transpile_file(path, None) {
            Ok((report, rust)) => {
                std::fs::write(&rust_path, &rust)?;
                report.write_json_file(&gap_path)?;
                // A module that emitted nothing is not a module that compiles.
                // Its `.rs` is a header comment, which rustc accepts, with no
                // `todo!()` in it — so left in the denominator it would score a
                // perfect "compiles, no stubs" for having produced no code at
                // all. Excluded explicitly, with the reason recorded, rather
                // than silently: an unmeasured file must never look measured.
                let declared = report
                    .gaps
                    .iter()
                    .filter(|g| g.category == crate::gap::Category::FunctionBody)
                    .count();
                let check = match (&checker, report.emitted_items.is_empty()) {
                    (_, true) => CheckResult::not_run("nothing emitted — no Rust to compile", 0, 0),
                    (Some(c), false) => c.check(&rust_path, declared),
                    (None, false) => CheckResult::not_run(
                        l3_off_reason.clone().unwrap_or_else(|| "L3 not run".into()),
                        crate::check::count_stub_bodies(&rust_path),
                        declared,
                    ),
                };
                total_emitted += report.emitted_items.len();
                total_gaps += report.real_gap_count();
                total_top += report.total_top_level_items;
                total_stmts += report.total_statements;
                lowered_stmts += report.lowered_statement_count();
                ok += 1;
                match check.status {
                    L3Status::Passed => {
                        l3_checked += 1;
                        l3_passed += 1;
                        if check.compiles_without_stubs() {
                            l3_clean += 1;
                        }
                    }
                    L3Status::Failed => {
                        l3_checked += 1;
                        l3_failed += 1;
                        for d in &check.errors {
                            let key = d.code.clone().unwrap_or_else(|| "<uncoded>".to_string());
                            *l3_codes.entry(key).or_insert(0) += 1;
                        }
                    }
                    L3Status::NotRun => {}
                }
                total_unlowered += check.unlowered_bodies();
                total_markers += check.stub_markers;
                if check.measurements_disagree() {
                    l3_disagreements += 1;
                }
                file_results.push(
                    FileResult {
                        source: path.display().to_string(),
                        rust_path: Some(rust_path.display().to_string()),
                        gap_path: Some(gap_path.display().to_string()),
                        emitted: report.emitted_items.len(),
                        gaps: report.real_gap_count(),
                        total_top_level: report.total_top_level_items,
                        expressible_fraction: report.expressible_fraction(),
                        total_statements: report.total_statements,
                        lowered_statements: report.lowered_statement_count(),
                        l3_status: L3Status::NotRun,
                        l3_not_run_reason: None,
                        l3_errors: Vec::new(),
                        unlowered_bodies: 0,
                        error: None,
                    }
                    .with_check(check),
                );
                reports.push(report);
            }
            Err(e) => {
                file_results.push(FileResult {
                    source: path.display().to_string(),
                    rust_path: None,
                    gap_path: None,
                    emitted: 0,
                    gaps: 0,
                    total_top_level: 0,
                    expressible_fraction: 0.0,
                    total_statements: 0,
                    lowered_statements: 0,
                    // Nothing was emitted, so there is nothing to compile. Not a
                    // compile failure — the file never got that far.
                    l3_status: L3Status::NotRun,
                    l3_not_run_reason: Some("nothing emitted — file did not parse".into()),
                    l3_errors: Vec::new(),
                    unlowered_bodies: 0,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let summary = BatchSummary {
        files: file_results,
        total_files: files.len(),
        ok_files: ok,
        total_emitted,
        total_gaps,
        total_top_level: total_top,
        total_statements: total_stmts,
        lowered_statements: lowered_stmts,
        l3_checked,
        l3_passed,
        l3_failed,
        l3_passed_without_stubs: l3_clean,
        total_unlowered_bodies: total_unlowered,
        total_stub_markers: total_markers,
        l3_measurement_disagreements: l3_disagreements,
        l3_error_codes: l3_codes,
        l3_not_run_reason: if l3_checked == 0 { l3_off_reason } else { None },
    };
    let union = UnionGapReport::from_reports(&reports);

    let summary_path = out_dir.join("summary.json");
    std::fs::write(
        &summary_path,
        serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".into()),
    )?;
    let union_path = out_dir.join("union.gap.json");
    std::fs::write(
        &union_path,
        serde_json::to_string_pretty(&union).unwrap_or_else(|_| "{}".into()),
    )?;

    Ok((summary, union))
}

/// Short, stable identifier for a priority item.
///
/// Derived from *what to fix*, never from a line number: line-keyed ids churn on
/// every unrelated edit, which makes two priority reports undiffable and destroys
/// the delta model. FNV-1a is enough — this needs to be stable and short, not
/// unforgeable.
fn stable_id(key: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in key.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// Render the **high-priority** report — the short list, not the record.
///
/// Split from the full report on purpose. The full report is for drilling into;
/// this answers "what do I do next", and is useless the moment it stops being
/// short. Three rules keep it that way:
///
/// 1. **Bounded.** `budget` caps the item count. A priority report with 400
///    entries is the full report wearing a different title.
/// 2. **States what it omitted.** A short list that does not say how much it left
///    out reads as the complete picture — the same failure as an unmeasured value
///    rendering as 0%.
/// 3. **Stable ids**, so two runs diff and progress is visible.
///
/// Ranking today is a **proxy**: unparsed files first (nothing else surfaces
/// them), then signature-only modules, then category frequency. The ranking that
/// matters is blast radius — closure size over the gap graph — which lands once
/// gaps record their dependencies.
pub fn render_priority_report(
    root: &Path,
    summary: &BatchSummary,
    union: &UnionGapReport,
    budget: usize,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let root_s = root.display().to_string();
    let rel = |s: &str| -> String {
        s.strip_prefix(&root_s)
            .unwrap_or(s)
            .trim_start_matches('/')
            .to_string()
    };
    let mut shown = 0usize;
    let mut candidates = 0usize;

    let _ = writeln!(out, "# py2rust — high priority\n");
    let _ = writeln!(out, "- root: `{root_s}`");
    let _ = writeln!(
        out,
        "- budget: {budget} item(s); full detail in the corpus report\n"
    );

    // 1. Unparsed files. First because they are invisible everywhere else: they
    //    contribute no gaps and no emitted items, so coverage *improves* as more
    //    files fail to parse.
    let failed: Vec<&FileResult> = summary.files.iter().filter(|f| f.error.is_some()).collect();
    candidates += failed.len();
    if !failed.is_empty() {
        let _ = writeln!(out, "## Did not parse — fix first\n");
        let _ = writeln!(
            out,
            "These contribute no gaps and no emitted items, so every percentage is"
        );
        let _ = writeln!(
            out,
            "computed as though they do not exist — coverage improves as more files"
        );
        let _ = writeln!(out, "fail. Nothing else surfaces them.\n");
        for f in failed.iter() {
            if shown >= budget {
                break;
            }
            let r = rel(&f.source);
            let _ = writeln!(
                out,
                "- `{}` — id `parse:{}` — {}",
                r,
                stable_id(&r),
                f.error.as_deref().unwrap_or("")
            );
            shown += 1;
        }
        let _ = writeln!(out);
    }

    // 2. Emitted, but `rustc` rejects it. Ahead of signature-only work because
    //    this is output that is actively wrong rather than merely incomplete:
    //    every other number in the corpus report counts these modules as
    //    successfully emitted.
    let broken: Vec<&FileResult> = summary
        .files
        .iter()
        .filter(|f| f.l3_status == L3Status::Failed)
        .collect();
    candidates += broken.len();
    if shown < budget && !broken.is_empty() {
        let _ = writeln!(out, "## Emitted but does not compile — fix next\n");
        let _ = writeln!(
            out,
            "These count as emitted in every expressible/L2 figure, so coverage looks"
        );
        let _ = writeln!(
            out,
            "the same whether they build or not. They do not build.\n"
        );
        for f in broken.iter() {
            if shown >= budget {
                break;
            }
            let r = rel(&f.source);
            let first = f
                .l3_errors
                .first()
                .map(|d| {
                    format!(
                        "L{} {}{}",
                        d.line,
                        d.code
                            .as_deref()
                            .map(|c| format!("{c} "))
                            .unwrap_or_default(),
                        d.message
                    )
                })
                .unwrap_or_else(|| "no diagnostic recorded".into());
            let _ = writeln!(
                out,
                "- `{}` — id `l3:{}` — {} error(s); first: {}",
                r,
                stable_id(&r),
                f.l3_errors.len(),
                first
            );
            shown += 1;
        }
        let _ = writeln!(out);
    }

    // 3. Signatures emitted, bodies not lowered — ranked by how much is behind them.
    let mut sig_only: Vec<&FileResult> = summary
        .files
        .iter()
        .filter(|f| f.error.is_none() && f.emitted > 0 && f.total_statements > f.emitted)
        .collect();
    candidates += sig_only.len();
    if shown < budget && !sig_only.is_empty() {
        sig_only.sort_by(|a, b| {
            (b.total_statements - b.emitted)
                .cmp(&(a.total_statements - a.emitted))
                .then_with(|| a.source.cmp(&b.source))
        });
        let _ = writeln!(out, "## Signatures only — most unlowered body statements\n");
        for f in sig_only.iter() {
            if shown >= budget {
                break;
            }
            let r = rel(&f.source);
            let _ = writeln!(
                out,
                "- `{}` — id `body:{}` — {} of {} statements unlowered",
                r,
                stable_id(&r),
                f.total_statements - f.emitted,
                f.total_statements
            );
            shown += 1;
        }
        let _ = writeln!(out);
    }

    // 4. Category frequency — a proxy for leverage until the gap graph exists.
    let mut cats: Vec<(&str, usize)> = union
        .category_counts
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if !cats.is_empty() {
        let _ = writeln!(out, "## Largest gap categories\n");
        let _ = writeln!(
            out,
            "Ranked by **frequency**, which is a proxy. Frequency says what is common;"
        );
        let _ = writeln!(
            out,
            "it does not say what is leveraged. The ranking that matters is blast"
        );
        let _ = writeln!(
            out,
            "radius — how many other gaps each root cause unblocks — and it arrives"
        );
        let _ = writeln!(out, "once gaps record their dependencies.\n");
        for (c, n) in cats.iter().take(5) {
            let _ = writeln!(out, "- `{c}` — {n}");
        }
        let _ = writeln!(out);
    }

    // 5. What was left out. Never omitted.
    let _ = writeln!(out, "## Omitted\n");
    let omitted = candidates.saturating_sub(shown);
    let _ = writeln!(
        out,
        "Showing **{shown}** of **{candidates}** candidate item(s); **{omitted}** not listed."
    );
    let _ = writeln!(
        out,
        "Total recorded gaps across the corpus: **{}**.",
        union.gaps.len()
    );
    let _ = writeln!(
        out,
        "\nThis is a worklist, not a survey. Absence from it is not evidence of"
    );
    let _ = writeln!(out, "correctness — see the corpus report for everything.");

    out
}

/// The L3 headline: does the emitted Rust compile, and does compiling mean
/// anything for this corpus.
///
/// Three states, never collapsed into two:
///
/// - **not measured** — the gate did not run. Rendered with the reason, because
///   a missing toolchain shown as 0% passing is a manufactured regression.
/// - **compiles** — `rustc` accepted it.
/// - **compiles without stub bodies** — and it does something. This leads,
///   because the emitter's unlowered-body fallback is `todo!()`, which compiles.
///   A corpus of signatures scores 100% on the middle number and 0% on this one,
///   and the middle number is the one that would get quoted.
fn render_l3_block(summary: &BatchSummary) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    if summary.l3_checked == 0 {
        let why = summary
            .l3_not_run_reason
            .as_deref()
            .unwrap_or("no reason recorded");
        let _ = writeln!(out, "- L3 compile gate: **not measured** ({why})");
        let _ = writeln!(
            out,
            "\n> Not measured is not 0%. Nothing was handed to `rustc`, so this report"
        );
        let _ = writeln!(
            out,
            "> makes no claim either way about whether the emitted Rust builds."
        );
        return out;
    }

    let pct = |n: usize| n as f64 / summary.l3_checked as f64 * 100.0;
    let _ = writeln!(
        out,
        "- L3 compiles: {:.1}% ({} of {} emitted modules accepted by `rustc`)",
        pct(summary.l3_passed),
        summary.l3_passed,
        summary.l3_checked
    );
    let _ = writeln!(
        out,
        "- **L3 compiles with no stub body: {:.1}%** ({} of {} modules)",
        pct(summary.l3_passed_without_stubs),
        summary.l3_passed_without_stubs,
        summary.l3_checked
    );
    let _ = writeln!(
        out,
        "- unlowered bodies across the corpus: {}\n",
        summary.total_unlowered_bodies
    );

    // Two independent counts of the same quantity. When they diverge, one of
    // them is blind, and the report should say which way rather than quietly
    // publishing whichever it happened to use.
    if summary.l3_measurement_disagreements > 0 {
        let _ = writeln!(
            out,
            "> **{} module(s) disagree between the two unlowered-body measurements**",
            summary.l3_measurement_disagreements
        );
        let _ = writeln!(
            out,
            "> ({} `todo!()` marker(s) in the emitted text vs {} `FunctionBody` gap(s)",
            summary.total_stub_markers, summary.total_unlowered_bodies
        );
        let _ = writeln!(
            out,
            "> recorded). The larger count is used, so credit fails toward \"less\"."
        );
        let _ = writeln!(
            out,
            "> A divergence means the emitter is producing a body shape one of the"
        );
        let _ = writeln!(out, "> two cannot see; that is worth a look.\n");
    }

    if summary.l3_passed > 0 && summary.l3_passed_without_stubs == 0 {
        let _ = writeln!(
            out,
            "> **Everything that compiles is a stub.** {} module(s) satisfy `rustc` and",
            summary.l3_passed
        );
        let _ = writeln!(
            out,
            "> not one of them has a lowered body: the emitter's fallback for an"
        );
        let _ = writeln!(
            out,
            "> unlowered body is `todo!()`, which compiles and then panics. Read the"
        );
        let _ = writeln!(
            out,
            "> compile figure as \"the scaffolding is well-formed\", never as \"it works\"."
        );
    } else if summary.l3_passed > summary.l3_passed_without_stubs {
        let stubbed = summary.l3_passed - summary.l3_passed_without_stubs;
        let _ = writeln!(
            out,
            "> Note: {stubbed} of the {} compiling module(s) still contain `todo!()` bodies,",
            summary.l3_passed
        );
        let _ = writeln!(
            out,
            "> so they build but panic when called. Only the second figure is progress."
        );
    }
    out
}

/// The L3 detail: which modules failed, and which `rustc` errors dominate.
///
/// Error-code frequency is the emitter's own backlog — the L3 analogue of gap
/// categories. `E0425` in bulk means unresolved names (imports not lowered);
/// `E0308` in bulk means the type map is wrong. Those are different work.
fn render_l3_section(rel: &dyn Fn(&str) -> String, summary: &BatchSummary) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "## L3 — emitted Rust that `rustc` rejects\n");

    if summary.l3_checked == 0 {
        let _ = writeln!(
            out,
            "Not measured ({}). No module was compiled, so an empty list here means",
            summary
                .l3_not_run_reason
                .as_deref()
                .unwrap_or("no reason recorded")
        );
        let _ = writeln!(out, "\"unknown\", not \"none\".\n");
        return out;
    }
    if summary.l3_failed == 0 {
        let _ = writeln!(
            out,
            "None — all {} compiled module(s) were accepted. See the stub-body count",
            summary.l3_checked
        );
        let _ = writeln!(out, "above before reading that as working code.\n");
        return out;
    }

    let mut codes: Vec<(&str, usize)> = summary
        .l3_error_codes
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    codes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let _ = writeln!(out, "### Error codes by frequency\n");
    let _ = writeln!(out, "| rank | code | count |");
    let _ = writeln!(out, "|---|---|---:|");
    for (i, (code, n)) in codes.iter().enumerate() {
        let _ = writeln!(out, "| {} | `{}` | {} |", i + 1, code, n);
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "### Modules that do not compile\n");
    let _ = writeln!(out, "| module | errors | first error |");
    let _ = writeln!(out, "|---|---:|---|");
    for f in summary
        .files
        .iter()
        .filter(|f| f.l3_status == L3Status::Failed)
    {
        let first = f
            .l3_errors
            .first()
            .map(|d| {
                format!(
                    "L{} {}{}",
                    d.line,
                    d.code
                        .as_deref()
                        .map(|c| format!("`{c}` "))
                        .unwrap_or_default(),
                    d.message.replace('|', "\\|")
                )
            })
            .unwrap_or_else(|| "—".into());
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            rel(&f.source),
            f.l3_errors.len(),
            first
        );
    }
    let _ = writeln!(out);
    out
}

/// Render a ranked Markdown corpus report from a batch run.
///
/// `union.category_counts` is a `BTreeMap`, so it is ordered alphabetically —
/// good for stable JSON, useless for deciding what to work on next. This ranks
/// it by **measured frequency**, which is the point of running the analysis
/// over a corpus at all: it turns "which gap category do we close next" from a
/// judgement call into a count.
///
/// Modules are ranked by expressible fraction, highest first, because that is
/// the port-order question — which module lowers most completely, and so costs
/// least to finish by hand.
///
/// Ties break by name so the report is byte-stable across runs. It is meant to
/// be committed and diffed, and one that reorders spuriously makes every diff
/// meaningless.
pub fn render_ranked_report(root: &Path, summary: &BatchSummary, union: &UnionGapReport) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let root_s = root.display().to_string();
    let rel = |s: &str| -> String {
        s.strip_prefix(&root_s)
            .unwrap_or(s)
            .trim_start_matches('/')
            .to_string()
    };

    let _ = writeln!(out, "# py2rust corpus report\n");
    let _ = writeln!(out, "- root: `{root_s}`");
    let _ = writeln!(
        out,
        "- files: {} discovered, {} parsed, {} failed",
        summary.total_files,
        summary.ok_files,
        summary.total_files.saturating_sub(summary.ok_files)
    );
    let _ = writeln!(
        out,
        "- top-level items: {} | emitted: {} | gaps: {}",
        summary.total_top_level, summary.total_emitted, summary.total_gaps
    );
    let overall = if summary.total_top_level == 0 {
        0.0
    } else {
        summary.total_emitted as f64 / summary.total_top_level as f64 * 100.0
    };
    let _ = writeln!(out, "- L1 top-level expressible: {overall:.1}%");

    // L2 leads, because L1 is measured against top-level items only and real
    // code keeps ~82% of its statements inside bodies. Reporting L1 alone
    // overstates progress by roughly 5x.
    if summary.total_statements > 0 {
        let l2 = summary.lowered_statements as f64 / summary.total_statements as f64 * 100.0;
        let inside = summary
            .total_statements
            .saturating_sub(summary.total_top_level);
        let inside_pct = inside as f64 / summary.total_statements as f64 * 100.0;
        let _ = writeln!(
            out,
            "- **L2 statement coverage: {l2:.1}%** ({} of {} statements lowered)",
            summary.lowered_statements, summary.total_statements
        );
        let _ = writeln!(
            out,
            "- statements inside bodies: {inside} ({inside_pct:.1}% of all statements)\n"
        );
        let _ = writeln!(
            out,
            "> **L2 is the number to steer by.** L1 counts only top-level items, so it"
        );
        let _ = writeln!(
            out,
            "> measures against {} statements while the module actually contains {}.",
            summary.total_top_level, summary.total_statements
        );
        let _ = writeln!(
            out,
            "> L2 counts every statement, nested included, and an emitted signature"
        );
        let _ = writeln!(
            out,
            "> whose body did not lower counts as exactly one statement — not as its"
        );
        let _ = writeln!(out, "> whole body.\n");
    } else {
        let _ = writeln!(
            out,
            "- L2 statement coverage: **not measured** (no statement denominator recorded)\n"
        );
        let _ = writeln!(
            out,
            "> Absence of an L2 figure is not 0% — it means nothing counted. Re-run with"
        );
        let _ = writeln!(out, "> a build that records the statement denominator.\n");
    }

    let _ = writeln!(out, "{}", render_l3_block(summary));

    // What "expressible" actually counts. `Category::FunctionBody` is documented
    // as "signature emitted, body not fully lowered", so when its count equals
    // the emitted count, EVERY emitted item is a signature with an unlowered
    // body -- and the percentage above measures signature emission, not ported
    // code. Measured on tg-agent-relay: 585 == 585 across 91/91 modules. Left
    // unsaid, someone reads "70% expressible" as "70% ported" and plans a
    // release around it.
    let fb = union
        .category_counts
        .get("FunctionBody")
        .copied()
        .unwrap_or(0);
    if summary.total_emitted > 0 && fb == summary.total_emitted {
        let _ = writeln!(
            out,
            "> **Read that number carefully.** `FunctionBody` gaps ({fb}) exactly equal emitted"
        );
        let _ = writeln!(
            out,
            "> items ({}), and that category means *signature emitted, body not lowered*. So",
            summary.total_emitted
        );
        let _ = writeln!(
            out,
            "> every emitted item here is a signature only: the percentage measures signature"
        );
        let _ = writeln!(
            out,
            "> emission, not ported code. Treat it as an upper bound on progress, never as"
        );
        let _ = writeln!(out, "> completion.\n");
    } else if summary.total_emitted > 0 && fb > 0 {
        let partial = fb as f64 / summary.total_emitted as f64 * 100.0;
        let _ = writeln!(
            out,
            "> Note: {fb} of {} emitted items ({partial:.0}%) are signature-only",
            summary.total_emitted
        );
        let _ = writeln!(out, "> (`FunctionBody` gap). The rest lowered fully.\n");
    }

    // --- categories, ranked by measured frequency ---------------------------
    let mut cats: Vec<(&str, usize)> = union
        .category_counts
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let total_gaps: usize = cats.iter().map(|(_, n)| *n).sum();

    let _ = writeln!(out, "## Gap categories by measured frequency\n");
    if cats.is_empty() {
        let _ = writeln!(
            out,
            "No gaps recorded. Either the corpus lowered completely, or nothing was"
        );
        let _ = writeln!(
            out,
            "analysed — check the file count above before reading this as success.\n"
        );
    } else {
        let _ = writeln!(out, "| rank | category | count | share |");
        let _ = writeln!(out, "|---|---|---:|---:|");
        for (i, (cat, n)) in cats.iter().enumerate() {
            let share = if total_gaps == 0 {
                0.0
            } else {
                *n as f64 / total_gaps as f64 * 100.0
            };
            let _ = writeln!(out, "| {} | `{}` | {} | {:.1}% |", i + 1, cat, n, share);
        }
        let _ = writeln!(out);
    }

    // --- modules, ranked by how completely they lower -----------------------
    // Gap.file and FileResult.source are both `path.display().to_string()` of
    // the same Path, so this join is exact rather than best-effort.
    let mut per_file: BTreeMap<&str, BTreeMap<&'static str, usize>> = BTreeMap::new();
    for g in &union.gaps {
        *per_file
            .entry(g.file.as_str())
            .or_default()
            .entry(g.category.as_str())
            .or_insert(0) += 1;
    }

    let mut ok_files: Vec<&FileResult> =
        summary.files.iter().filter(|f| f.error.is_none()).collect();
    ok_files.sort_by(|a, b| {
        b.expressible_fraction
            .partial_cmp(&a.expressible_fraction)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.source.cmp(&b.source))
    });

    let _ = writeln!(out, "## Modules by expressible fraction\n");
    let _ = writeln!(out, "Highest first — cheapest to finish by hand.\n");
    let _ = writeln!(
        out,
        "| module | top-level | emitted | gaps | expressible | dominant gaps |"
    );
    let _ = writeln!(out, "|---|---:|---:|---:|---:|---|");
    for f in &ok_files {
        let dominant = match per_file.get(f.source.as_str()) {
            None => "—".to_string(),
            Some(m) => {
                let mut v: Vec<(&str, usize)> = m.iter().map(|(k, n)| (*k, *n)).collect();
                v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
                v.iter()
                    .take(3)
                    .map(|(c, n)| format!("{c}×{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {:.0}% | {} |",
            rel(&f.source),
            f.total_top_level,
            f.emitted,
            f.gaps,
            f.expressible_fraction * 100.0,
            dominant
        );
    }
    let _ = writeln!(out);

    out.push_str(&render_l3_section(&rel, summary));

    // --- failures, deliberately not folded into the rankings ----------------
    let failed: Vec<&FileResult> = summary.files.iter().filter(|f| f.error.is_some()).collect();
    let _ = writeln!(out, "## Files that did not parse\n");
    if failed.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(
            out,
            "These contributed no gaps and no emitted items, so they are absent from"
        );
        let _ = writeln!(
            out,
            "every ranking above. A high expressible fraction next to a long list here"
        );
        let _ = writeln!(
            out,
            "means the corpus was barely read, not that it lowered well.\n"
        );
        let _ = writeln!(out, "| module | error |");
        let _ = writeln!(out, "|---|---|");
        for f in failed {
            let _ = writeln!(
                out,
                "| `{}` | {} |",
                rel(&f.source),
                f.error.as_deref().unwrap_or("").replace('|', "\\|")
            );
        }
        let _ = writeln!(out);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // --- report assertions -------------------------------------------------
    // Extracted so the tests below contain no string-offset arithmetic. Nine
    // separate `md.find(..).unwrap()` expressions is nine independent ways for a
    // wording change to break a test for a reason unrelated to what it asserts.

    /// Assert `first` appears before `second`, naming both on failure.
    fn assert_order(md: &str, first: &str, second: &str) {
        let a = md
            .find(first)
            .unwrap_or_else(|| panic!("missing {first:?} in report:\n{md}"));
        let b = md
            .find(second)
            .unwrap_or_else(|| panic!("missing {second:?} in report:\n{md}"));
        assert!(a < b, "expected {first:?} before {second:?}:\n{md}");
    }

    /// Assert every marker appears, in the order given.
    fn assert_sequence(md: &str, markers: &[&str]) {
        for pair in markers.windows(2) {
            assert_order(md, pair[0], pair[1]);
        }
    }

    /// The body of one `## ` section, so a test can assert about that section
    /// alone rather than slicing the whole document by hand.
    fn section<'a>(md: &'a str, heading: &str) -> &'a str {
        let start = md
            .find(heading)
            .unwrap_or_else(|| panic!("no section {heading:?} in:\n{md}"));
        let rest = &md[start + heading.len()..];
        match rest.find("\n## ") {
            Some(end) => &rest[..end],
            None => rest,
        }
    }

    /// The stable id following `prefix`, e.g. "body:" -> "5628a1f0".
    fn id_after<'a>(md: &'a str, prefix: &str) -> &'a str {
        let marker = format!("id `{prefix}");
        let at = md
            .find(&marker)
            .unwrap_or_else(|| panic!("no id with prefix {prefix:?} in:\n{md}"));
        let rest = &md[at + marker.len()..];
        let end = rest.find('`').unwrap_or(rest.len());
        &rest[..end]
    }

    fn fr(source: &str, emitted: usize, gaps: usize, top: usize, frac: f64) -> FileResult {
        FileResult {
            source: source.into(),
            rust_path: None,
            gap_path: None,
            emitted,
            gaps,
            total_top_level: top,
            expressible_fraction: frac,
            total_statements: 0,
            lowered_statements: 0,
            l3_status: L3Status::NotRun,
            l3_not_run_reason: Some("fixture: L3 not run".into()),
            l3_errors: Vec::new(),
            unlowered_bodies: 0,
            error: None,
        }
    }

    /// Attach an L3 verdict to a fixture file. Two words at the call site beats
    /// four field assignments, and keeps the tables below readable.
    fn with_l3(
        mut f: FileResult,
        status: L3Status,
        errs: &[(&str, &str)],
        stubs: usize,
    ) -> FileResult {
        f.l3_status = status;
        f.l3_not_run_reason = None;
        f.unlowered_bodies = stubs;
        f.l3_errors = errs
            .iter()
            .map(|(code, msg)| RustcDiag {
                code: Some((*code).into()),
                message: (*msg).into(),
                line: 1,
            })
            .collect();
        f
    }

    /// Build a summary by summing its files — the same trivial folds the batch
    /// driver does, so a fixture cannot express an internally inconsistent run.
    fn summary_of(files: Vec<FileResult>) -> BatchSummary {
        let ok = files.iter().filter(|f| f.error.is_none()).count();
        let count = |s: L3Status| files.iter().filter(|f| f.l3_status == s).count();
        let mut codes: BTreeMap<String, usize> = BTreeMap::new();
        for f in files.iter().filter(|f| f.l3_status == L3Status::Failed) {
            for d in &f.l3_errors {
                let k = d.code.clone().unwrap_or_else(|| "<uncoded>".into());
                *codes.entry(k).or_insert(0) += 1;
            }
        }
        let checked = count(L3Status::Passed) + count(L3Status::Failed);
        BatchSummary {
            total_files: files.len(),
            ok_files: ok,
            total_emitted: files.iter().map(|f| f.emitted).sum(),
            total_gaps: files.iter().map(|f| f.gaps).sum(),
            total_top_level: files.iter().map(|f| f.total_top_level).sum(),
            total_statements: files.iter().map(|f| f.total_statements).sum(),
            lowered_statements: files.iter().map(|f| f.lowered_statements).sum(),
            l3_checked: checked,
            l3_passed: count(L3Status::Passed),
            l3_failed: count(L3Status::Failed),
            l3_passed_without_stubs: files
                .iter()
                .filter(|f| f.l3_status == L3Status::Passed && f.unlowered_bodies == 0)
                .count(),
            total_unlowered_bodies: files.iter().map(|f| f.unlowered_bodies).sum(),
            total_stub_markers: files.iter().map(|f| f.unlowered_bodies).sum(),
            l3_measurement_disagreements: 0,
            l3_error_codes: codes,
            l3_not_run_reason: if checked == 0 {
                Some("fixture: L3 not run".into())
            } else {
                None
            },
            files,
        }
    }

    fn union_of(counts: &[(&'static str, usize)]) -> UnionGapReport {
        UnionGapReport {
            gaps: Vec::new(),
            category_counts: counts.iter().copied().collect(),
            file_count: 1,
        }
    }

    fn fr_l2(source: &str, emitted: usize, total_stmts: usize) -> FileResult {
        let mut f = fr(source, emitted, 0, emitted, 1.0);
        f.total_statements = total_stmts;
        f.lowered_statements = emitted;
        f
    }

    #[test]
    fn priority_report_respects_its_budget_and_says_what_it_omitted() {
        let mut files = Vec::new();
        for i in 0..12 {
            let mut f = fr(&format!("/r/m{i:02}.py"), 1, 0, 1, 1.0);
            f.total_statements = 50 + i;
            f.lowered_statements = 1;
            files.push(f);
        }
        let summary = summary_of(files);
        let md = render_priority_report(Path::new("/r"), &summary, &union_of(&[]), 3);
        let listed = md.matches("— id `body:").count();
        assert_eq!(listed, 3, "budget of 3 must cap the list:\n{md}");
        assert!(
            md.contains("Showing **3** of **12**") && md.contains("**9** not listed"),
            "must state what it omitted:\n{md}"
        );
        assert!(md.contains("not evidence of"), "must disclaim completeness");
    }

    #[test]
    fn priority_ids_are_stable_and_line_independent() {
        // Same file, different statement counts -- the id must not move, or two
        // runs cannot be diffed and the delta model is worthless.
        let mut a = fr("/r/x.py", 1, 0, 1, 1.0);
        a.total_statements = 40;
        let mut b = fr("/r/x.py", 1, 0, 1, 1.0);
        b.total_statements = 900;
        let ma = render_priority_report(Path::new("/r"), &summary_of(vec![a]), &union_of(&[]), 5);
        let mb = render_priority_report(Path::new("/r"), &summary_of(vec![b]), &union_of(&[]), 5);
        let id = |m: &str| {
            let i = m.find("id `body:").unwrap() + 9;
            m[i..i + 8].to_string()
        };
        assert_eq!(
            id(&ma),
            id(&mb),
            "id must key on the file, not its contents"
        );
    }

    #[test]
    fn unparsed_files_lead_the_priority_report() {
        let mut bad = fr("/r/broken.py", 0, 0, 0, 0.0);
        bad.error = Some("Parse: bad token".into());
        let mut ok = fr("/r/big.py", 1, 0, 1, 1.0);
        ok.total_statements = 500;
        let summary = summary_of(vec![ok, bad]);
        let md = render_priority_report(Path::new("/r"), &summary, &union_of(&[]), 10);
        assert_order(&md, "broken.py", "big.py");
        assert!(md.contains("coverage improves as more files"));
    }

    #[test]
    fn l2_is_reported_and_leads_l1() {
        let summary = summary_of(vec![fr_l2("/r/a.py", 3, 60)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert!(md.contains("L2 statement coverage: 5.0%"), "got:\n{md}");
        assert!(md.contains("3 of 60 statements lowered"));
        assert_order(&md, "L1 top-level", "L2 statement coverage");
    }

    #[test]
    fn unmeasured_l2_is_not_reported_as_zero() {
        // total_statements == 0 means "never counted". Rendering that as 0.0%
        // would be the /proc-is-blind mistake in a new costume.
        let summary = summary_of(vec![fr("/r/a.py", 3, 0, 3, 1.0)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert!(md.contains("not measured"), "got:\n{md}");
        assert!(!md.contains("L2 statement coverage: 0.0%"));
        assert!(md.contains("Absence of an L2 figure is not 0%"));
    }

    // --- L3 ----------------------------------------------------------------

    #[test]
    fn unmeasured_l3_is_not_reported_as_nothing_compiling() {
        // Same failure mode as unmeasured L2, and the more dangerous of the two:
        // "0% compiles" reads as a catastrophic regression rather than as an
        // absent toolchain.
        let md = render_ranked_report(
            Path::new("/r"),
            &summary_of(vec![fr("/r/a.py", 3, 0, 3, 1.0)]),
            &union_of(&[]),
        );
        assert!(
            md.contains("L3 compile gate: **not measured**"),
            "got:\n{md}"
        );
        assert!(md.contains("Not measured is not 0%"));
        assert!(!md.contains("L3 compiles: 0.0%"));
        assert!(
            section(&md, "## L3 — emitted Rust that `rustc` rejects")
                .contains("\"unknown\", not \"none\""),
            "an empty failure list under a not-run gate must not read as clean:\n{md}"
        );
    }

    #[test]
    fn compiling_stubs_do_not_read_as_working_code() {
        // The corpus today emits signatures with `todo!()` bodies. Every one
        // compiles. Reporting only that would be L1 in a new costume.
        let summary = summary_of(vec![
            with_l3(fr("/r/a.py", 2, 2, 2, 1.0), L3Status::Passed, &[], 2),
            with_l3(fr("/r/b.py", 3, 3, 3, 1.0), L3Status::Passed, &[], 5),
        ]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert!(md.contains("L3 compiles: 100.0%"), "got:\n{md}");
        assert!(md.contains("L3 compiles with no stub body: 0.0%"));
        assert!(md.contains("Everything that compiles is a stub"));
        assert!(md.contains("unlowered bodies across the corpus: 7"));
        assert_order(&md, "L3 compiles:", "L3 compiles with no stub body");
    }

    #[test]
    fn partially_stubbed_corpus_is_quantified_not_absolutised() {
        let summary = summary_of(vec![
            with_l3(fr("/r/a.py", 2, 0, 2, 1.0), L3Status::Passed, &[], 0),
            with_l3(fr("/r/b.py", 2, 0, 2, 1.0), L3Status::Passed, &[], 3),
        ]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert!(!md.contains("Everything that compiles is a stub"));
        assert!(
            md.contains("1 of the 2 compiling module(s) still contain `todo!()` bodies"),
            "got:\n{md}"
        );
    }

    #[test]
    fn l3_failures_rank_by_error_code_frequency() {
        let summary = summary_of(vec![
            with_l3(
                fr("/r/a.py", 1, 0, 1, 1.0),
                L3Status::Failed,
                &[
                    ("E0425", "cannot find value `os`"),
                    ("E0425", "cannot find value `sys`"),
                ],
                0,
            ),
            with_l3(
                fr("/r/b.py", 1, 0, 1, 1.0),
                L3Status::Failed,
                &[("E0308", "mismatched types")],
                0,
            ),
            with_l3(fr("/r/c.py", 1, 0, 1, 1.0), L3Status::Passed, &[], 0),
        ]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        let sec = section(&md, "## L3 — emitted Rust that `rustc` rejects");
        assert_sequence(sec, &["| 1 | `E0425` | 2 |", "| 2 | `E0308` | 1 |"]);
        assert!(
            sec.contains("| `a.py` | 2 | L1 `E0425` cannot find value `os` |"),
            "got:\n{sec}"
        );
        assert!(
            !sec.contains("`c.py`"),
            "a compiling module is not a failure"
        );
    }

    #[test]
    fn broken_output_outranks_unlowered_bodies_in_the_priority_report() {
        // A module that emits and does not compile is worse than one that emits
        // a signature: both count as emitted everywhere else, but only one of
        // them is wrong rather than merely unfinished.
        let mut sig = fr("/r/sig.py", 1, 0, 1, 1.0);
        sig.total_statements = 400;
        let broken = with_l3(
            fr("/r/broken.py", 1, 0, 1, 1.0),
            L3Status::Failed,
            &[("E0433", "failed to resolve")],
            0,
        );
        let summary = summary_of(vec![sig, broken]);
        let md = render_priority_report(Path::new("/r"), &summary, &union_of(&[]), 10);
        assert_order(&md, "id `l3:", "id `body:");
        assert_eq!(
            id_after(&md, "l3:"),
            id_after(
                &render_priority_report(Path::new("/r"), &summary, &union_of(&[]), 1),
                "l3:"
            ),
            "id must not depend on the budget"
        );
        assert!(md.contains("They do not build."));
    }

    #[test]
    fn report_ranks_categories_by_frequency_not_alphabetically() {
        // category_counts is a BTreeMap, so its natural order is alphabetical:
        // Async, DynamicTyping, Import. Ranked output must not be.
        let union = union_of(&[("Async", 3), ("DynamicTyping", 40), ("Import", 12)]);
        let summary = summary_of(vec![fr("/r/a.py", 1, 55, 4, 0.25)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union);
        assert_sequence(&md, &["`DynamicTyping`", "`Import`", "`Async`"]);
        assert!(md.contains("| 1 | `DynamicTyping` | 40 |"));
    }

    #[test]
    fn signature_only_caveat_fires_when_functionbody_equals_emitted() {
        let union = union_of(&[("FunctionBody", 5), ("DynamicTyping", 9)]);
        let summary = summary_of(vec![fr("/r/a.py", 5, 14, 10, 0.5)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union);
        assert!(
            md.contains("Read that number carefully"),
            "must warn that expressible counts signatures, not ported code:\n{md}"
        );
    }

    #[test]
    fn no_caveat_when_some_items_lowered_fully() {
        let union = union_of(&[("FunctionBody", 2), ("DynamicTyping", 9)]);
        let summary = summary_of(vec![fr("/r/a.py", 5, 11, 10, 0.5)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union);
        assert!(!md.contains("Read that number carefully"));
        assert!(
            md.contains("signature-only"),
            "should still quantify it:\n{md}"
        );
    }

    #[test]
    fn modules_rank_by_expressible_desc_with_stable_tiebreak() {
        let summary = summary_of(vec![
            fr("/r/low.py", 1, 9, 10, 0.10),
            fr("/r/b_tie.py", 5, 5, 10, 0.50),
            fr("/r/a_tie.py", 5, 5, 10, 0.50),
            fr("/r/high.py", 9, 1, 10, 0.90),
        ]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert_sequence(&md, &["high.py", "a_tie.py", "b_tie.py", "low.py"]);
    }

    #[test]
    fn unparsed_files_are_listed_and_excluded_from_rankings() {
        let mut bad = fr("/r/broken.py", 0, 0, 0, 0.0);
        bad.error = Some("Parse: unexpected token".into());
        let summary = summary_of(vec![fr("/r/ok.py", 5, 5, 10, 0.5), bad]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert!(md.contains("| `broken.py` | Parse: unexpected token |"));
        assert!(md.contains("1 discovered") || md.contains("2 discovered, 1 parsed, 1 failed"));
        // It must not appear in the ranked module table as a 0% row, which would
        // read as "analysed and found empty" rather than "never read".
        let table = &md[md.find("## Modules by expressible").unwrap()
            ..md.find("## Files that did not parse").unwrap()];
        assert!(
            !table.contains("broken.py"),
            "unparsed file leaked into rankings:\n{table}"
        );
    }

    #[test]
    fn empty_corpus_does_not_read_as_clean() {
        let md = render_ranked_report(Path::new("/r"), &summary_of(vec![]), &union_of(&[]));
        assert!(
            md.contains("check the file count above before reading this as success"),
            "an empty run must not look like a gap-free one:\n{md}"
        );
    }

    #[test]
    fn discover_and_batch_smoke() {
        let dir = std::env::temp_dir().join(format!("py2rust-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(dir.join("a.py")).unwrap();
        writeln!(f, "def f(x: int) -> int:\n    return x").unwrap();
        let mut g = std::fs::File::create(dir.join("b.py")).unwrap();
        writeln!(g, "class C:\n    pass").unwrap();

        let out = dir.join("out");
        let (summary, union) = transpile_batch(&dir, &out).unwrap();
        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.ok_files, 2);
        assert!(union.category_counts.contains_key("Class") || summary.total_emitted >= 1);
        assert!(out.join("summary.json").exists());
        assert!(out.join("union.gap.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
