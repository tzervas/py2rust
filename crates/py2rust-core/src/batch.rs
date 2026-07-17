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
                ok += 1;
                file_results.push(FileResult {
                    source: path.display().to_string(),
                    rust_path: Some(rust_path.display().to_string()),
                    gap_path: Some(gap_path.display().to_string()),
                    emitted: report.emitted_items.len(),
                    gaps: report.real_gap_count(),
                    total_top_level: report.total_top_level_items,
                    expressible_fraction: report.expressible_fraction(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
