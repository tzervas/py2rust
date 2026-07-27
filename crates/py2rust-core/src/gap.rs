//! Structured, never-silent gap report for Python → Rust (G2 honesty model).
//!
//! Shape ported from `research/mycelium-transpile-snapshot/src/gap.rs`, with
//! **Python-specific** categories. Every construct the driver cannot (or will not)
//! lower is recorded here — never dropped silently.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Schema version embedded in JSON for forward-compatible consumers.
pub const GAP_SCHEMA_VERSION: u32 = 1;

/// Closed, PoC-scoped taxonomy of unsupported / uncertain Python constructs.
///
/// Constructs that fit none of these still get [`Category::Other`] plus a free-text
/// reason — never a silent drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Category {
    /// `class` definitions and inheritance.
    Class,
    /// `try` / `except` / `raise` / `finally`.
    Exception,
    /// Unannotated parameters, `Any`, dynamic attributes.
    DynamicTyping,
    /// Decorators that alter defs, `exec` / `eval`, metaclasses.
    Metaprogramming,
    /// `async` / `await` / `async def` / `async with` / `async for`.
    Async,
    /// Imports without a confirmed mapping (or not yet lowered).
    Import,
    /// `lambda` expressions.
    Lambda,
    /// List / dict / set / generator comprehensions.
    Comprehension,
    /// Multi-statement bodies not yet lowered.
    MultiStmtBody,
    /// Signature emitted, body not fully lowered (sub-gap on partial emit).
    FunctionBody,
    /// Catch-all — never silent.
    Other,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Class => "Class",
            Category::Exception => "Exception",
            Category::DynamicTyping => "DynamicTyping",
            Category::Metaprogramming => "Metaprogramming",
            Category::Async => "Async",
            Category::Import => "Import",
            Category::Lambda => "Lambda",
            Category::Comprehension => "Comprehension",
            Category::MultiStmtBody => "MultiStmtBody",
            Category::FunctionBody => "FunctionBody",
            Category::Other => "Other",
        }
    }

    /// Categories excluded from the expressible-fraction denominator.
    /// Currently none for Python (all top-level stmts are surface we care about).
    pub fn excluded_from_denominator(self) -> bool {
        false
    }

    /// Non-gap advisory categories (recorded but not counted in headline gap totals).
    pub fn is_non_gap_advisory(self) -> bool {
        false
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One construct this transpiler could not (or would not) fully lower to Rust.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Gap {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub category: Category,
    /// Always derived from `category` (stable JSON string).
    pub python_construct: String,
    pub snippet: String,
    pub reason: String,
    /// Best-effort name when the construct has one (`def`/`class`/…).
    pub item_name: Option<String>,
}

impl Gap {
    pub fn new(
        file: impl Into<String>,
        line: usize,
        col: usize,
        category: Category,
        snippet: impl Into<String>,
        reason: impl Into<String>,
        item_name: Option<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            col,
            category,
            python_construct: category.as_str().to_string(),
            snippet: snippet.into(),
            reason: reason.into(),
            item_name,
        }
    }
}

/// Internal helper carrying category + reason before a full [`Gap`] is materialised.
#[derive(Debug, Clone)]
pub struct GapReason {
    pub category: Category,
    pub reason: String,
}

impl GapReason {
    pub fn new(category: Category, reason: impl Into<String>) -> Self {
        Self {
            category,
            reason: reason.into(),
        }
    }
}

/// Full report for one transpiled Python source file.
///
/// **Transparency:** `emitted_items` records that *some* Rust text was produced —
/// **Declared** (heuristic), never a claim the output type-checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GapReport {
    /// Schema version for consumers.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub source: String,
    pub emitted_items: Vec<String>,
    pub gaps: Vec<Gap>,
    /// `module.body.len()` — every top-level statement, including those only gapped.
    pub total_top_level_items: usize,
}

fn default_schema_version() -> u32 {
    GAP_SCHEMA_VERSION
}

impl GapReport {
    pub fn new(source: impl Into<String>, total_top_level_items: usize) -> Self {
        Self {
            schema_version: GAP_SCHEMA_VERSION,
            source: source.into(),
            emitted_items: Vec::new(),
            gaps: Vec::new(),
            total_top_level_items,
        }
    }

    pub fn denominator_excluded_count(&self) -> usize {
        self.gaps
            .iter()
            .filter(|g| g.category.excluded_from_denominator())
            .count()
    }

    /// Translatable-surface denominator: total top-level minus excluded categories.
    pub fn non_excluded_item_count(&self) -> usize {
        self.total_top_level_items
            .saturating_sub(self.denominator_excluded_count())
    }

    /// Fraction of non-excluded top-level items for which some Rust text was emitted.
    /// **Declared** — ratio over a heuristic classification.
    pub fn expressible_fraction(&self) -> f64 {
        let denom = self.non_excluded_item_count();
        if denom == 0 {
            return 0.0;
        }
        self.emitted_items.len() as f64 / denom as f64
    }

    pub fn category_counts(&self) -> BTreeMap<&'static str, usize> {
        let mut m = BTreeMap::new();
        for g in &self.gaps {
            *m.entry(g.category.as_str()).or_insert(0) += 1;
        }
        m
    }

    /// Headline gap count (excludes non-gap advisories).
    pub fn real_gap_count(&self) -> usize {
        self.gaps
            .iter()
            .filter(|g| !g.category.is_non_gap_advisory())
            .count()
    }

    /// G2 never-silent bound (count form): every top-level item is witnessed by
    /// at least one of (emitted name, gap record). Sub-gaps on emitted items can
    /// make `gaps.len()` larger than the unaccounted remainder — that only strengthens
    /// the inequality.
    pub fn never_silent_holds(&self) -> bool {
        self.emitted_items.len() + self.gaps.len() >= self.total_top_level_items
    }

    /// Serialize to pretty JSON (`.gap.json` sidecar).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.to_json()
    }

    pub fn write_json_file(&self, path: &Path) -> std::io::Result<()> {
        let json = self
            .to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Write `<stem>.gap.json` next to `path_for_stem` (`.py` or `.rs` or any path).
    pub fn write_sidecar(&self, path_for_stem: &Path) -> std::io::Result<PathBuf> {
        let out = gap_json_path(path_for_stem);
        if let Some(parent) = out.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        self.write_json_file(&out)?;
        Ok(out)
    }
}

/// Compute `<stem>.gap.json` path for a source or output file.
pub fn gap_json_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.join(format!("{stem}.gap.json")),
        _ => PathBuf::from(format!("{stem}.gap.json")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_as_str_roundtrip_serde() {
        let cats = [
            Category::Class,
            Category::Exception,
            Category::DynamicTyping,
            Category::Metaprogramming,
            Category::Async,
            Category::Import,
            Category::Lambda,
            Category::Comprehension,
            Category::MultiStmtBody,
            Category::FunctionBody,
            Category::Other,
        ];
        for c in cats {
            let j = serde_json::to_string(&c).unwrap();
            let back: Category = serde_json::from_str(&j).unwrap();
            assert_eq!(c, back);
            assert_eq!(j, format!("\"{}\"", c.as_str()));
        }
    }

    #[test]
    fn expressible_fraction_empty_and_full() {
        let empty = GapReport::new("x.py", 0);
        assert_eq!(empty.expressible_fraction(), 0.0);
        assert!(empty.never_silent_holds());

        let mut full = GapReport::new("x.py", 2);
        full.emitted_items = vec!["a".into(), "b".into()];
        assert!((full.expressible_fraction() - 1.0).abs() < f64::EPSILON);
        assert!(full.never_silent_holds());

        let mut half = GapReport::new("x.py", 2);
        half.emitted_items = vec!["a".into()];
        half.gaps.push(Gap::new(
            "x.py",
            1,
            1,
            Category::Class,
            "class C: pass",
            "not lowered",
            Some("C".into()),
        ));
        assert!((half.expressible_fraction() - 0.5).abs() < f64::EPSILON);
        assert_eq!(half.real_gap_count(), 1);
        assert_eq!(half.category_counts().get("Class"), Some(&1));
        assert!(half.never_silent_holds());
    }

    #[test]
    fn gap_json_sidecar_shape() {
        let mut r = GapReport::new("demo.py", 1);
        r.gaps.push(Gap::new(
            "demo.py",
            3,
            1,
            Category::Lambda,
            "lambda x: x",
            "lambda not lowered",
            None,
        ));
        let j = r.to_json().unwrap();
        assert!(j.contains("\"category\": \"Lambda\""));
        assert!(j.contains("\"python_construct\": \"Lambda\""));
        assert!(j.contains("\"total_top_level_items\": 1"));
        assert!(j.contains("\"schema_version\""));
    }

    #[test]
    fn gap_json_path_naming() {
        assert_eq!(
            gap_json_path(Path::new("src/demo.py")),
            PathBuf::from("src/demo.gap.json")
        );
        assert_eq!(
            gap_json_path(Path::new("out/demo.rs")),
            PathBuf::from("out/demo.gap.json")
        );
    }
}
