//! Multi-file batch helpers (summary + union gap report).
//!
//! Shape inspired by `research/mycelium-transpile-snapshot/src/batch.rs`.

use crate::dispatch;
use crate::gap::{Gap, GapReport};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    pub error: Option<String>,
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
pub fn transpile_batch(
    root: &Path,
    out_dir: &Path,
) -> std::io::Result<(BatchSummary, UnionGapReport)> {
    std::fs::create_dir_all(out_dir)?;
    let files = discover_py_files(root)?;
    let mut file_results = Vec::new();
    let mut reports = Vec::new();
    let mut total_emitted = 0usize;
    let mut total_gaps = 0usize;
    let mut total_top = 0usize;
    let mut total_stmts = 0usize;
    let mut lowered_stmts = 0usize;
    let mut ok = 0usize;

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
                total_emitted += report.emitted_items.len();
                total_gaps += report.real_gap_count();
                total_top += report.total_top_level_items;
                total_stmts += report.total_statements;
                lowered_stmts += report.lowered_statement_count();
                ok += 1;
                file_results.push(FileResult {
                    source: path.display().to_string(),
                    rust_path: Some(rust_path.display().to_string()),
                    gap_path: Some(gap_path.display().to_string()),
                    emitted: report.emitted_items.len(),
                    gaps: report.real_gap_count(),
                    total_top_level: report.total_top_level_items,
                    expressible_fraction: report.expressible_fraction(),
                    total_statements: report.total_statements,
                    lowered_statements: report.lowered_statement_count(),
                    error: None,
                });
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
    let mut cats: Vec<(&str, usize)> =
        union.category_counts.iter().map(|(k, v)| (*k, *v)).collect();
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
            error: None,
        }
    }

    fn summary_of(files: Vec<FileResult>) -> BatchSummary {
        let ok = files.iter().filter(|f| f.error.is_none()).count();
        BatchSummary {
            total_files: files.len(),
            ok_files: ok,
            total_emitted: files.iter().map(|f| f.emitted).sum(),
            total_gaps: files.iter().map(|f| f.gaps).sum(),
            total_top_level: files.iter().map(|f| f.total_top_level).sum(),
            total_statements: files.iter().map(|f| f.total_statements).sum(),
            lowered_statements: files.iter().map(|f| f.lowered_statements).sum(),
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
    fn l2_is_reported_and_leads_l1() {
        let summary = summary_of(vec![fr_l2("/r/a.py", 3, 60)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union_of(&[]));
        assert!(md.contains("L2 statement coverage: 5.0%"), "got:\n{md}");
        assert!(md.contains("3 of 60 statements lowered"));
        assert!(
            md.find("L2 statement coverage").unwrap() > md.find("L1 top-level").unwrap(),
            "L2 must appear with L1, not replace it"
        );
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

    #[test]
    fn report_ranks_categories_by_frequency_not_alphabetically() {
        // category_counts is a BTreeMap, so its natural order is alphabetical:
        // Async, DynamicTyping, Import. Ranked output must not be.
        let union = union_of(&[("Async", 3), ("DynamicTyping", 40), ("Import", 12)]);
        let summary = summary_of(vec![fr("/r/a.py", 1, 55, 4, 0.25)]);
        let md = render_ranked_report(Path::new("/r"), &summary, &union);
        let dt = md.find("`DynamicTyping`").unwrap();
        let im = md.find("`Import`").unwrap();
        let asy = md.find("`Async`").unwrap();
        assert!(dt < im && im < asy, "ranked by count desc, got:\n{md}");
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
        assert!(md.contains("signature-only"), "should still quantify it:\n{md}");
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
        let order: Vec<usize> = ["high.py", "a_tie.py", "b_tie.py", "low.py"]
            .iter()
            .map(|n| md.find(n).unwrap_or_else(|| panic!("missing {n} in\n{md}")))
            .collect();
        assert!(
            order.windows(2).all(|w| w[0] < w[1]),
            "expected high, a_tie, b_tie, low — ties by name for stable diffs:\n{md}"
        );
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
        assert!(!table.contains("broken.py"), "unparsed file leaked into rankings:\n{table}");
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
