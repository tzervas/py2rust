//! `py2rust-core` — Python → Rust transpiler library, plus a **Python →
//! Mycelium-native** mapping layer that does not require an interim Rust route.
//!
//! Pipeline: Python source → parse (`rustpython-parser`) → never-silent
//! [`dispatch`] over `Module.body` → best-effort Rust emission + structured
//! [`.gap.json`](gap::GapReport) (G2 / VR-5 honesty patterns ported from the
//! mycelium-transpile research snapshot; **not** a Mycelium dependency).
//!
//! The Mycelium mapping surface lives in [`interface`] + [`myc_map`]: every
//! [`gap::Category`] / [`interface::PyConstruct`] is either Mapped to a
//! Mycelium spelling or explicitly Unmappable with reason (no silent drops).
//!
//! # Guarantee tags (VR-5)
//!
//! - Emitted Rust is **Declared**: heuristic lowering, not validated by `rustc`.
//! - Never-silent invariant (every top-level item is emitted, gapped, or both) is
//!   checked over a fixed fixture corpus — **Empirical/Declared**, not Proven
//!   (`Stmt` exhaustiveness rests on a catch-all arm).
//! - Mycelium map rows use [`interface::Guarantee`] (Exact / Empirical / Refusal)
//!   and always carry a [`interface::Citation`] (Corpus / Test / Unverified).

pub mod batch;
pub mod check;
pub mod dispatch;
pub mod emit;
pub mod gap;
pub mod interface;
pub mod map;
pub mod myc_map;
pub mod parse;
pub mod source_loc;

pub use batch::{
    discover_py_files, render_priority_report, render_ranked_report, transpile_batch,
    transpile_batch_with, BatchOptions, BatchSummary, FileResult, UnionGapReport,
};
pub use check::{CheckResult, Checker, L3Status, RustcDiag};
pub use dispatch::{
    analyze_file, analyze_source, dispatch_stmt, transpile_file, transpile_source, DispatchError,
    Outcome,
};
pub use gap::{gap_json_path, Category, Gap, GapReason, GapReport, GAP_SCHEMA_VERSION};
pub use interface::{taxonomy_constructs, Citation, Guarantee, MapOutcome, MycForm, PyConstruct};
pub use myc_map::{honesty_violations, map_construct, map_taxonomy, taxonomy_coverage};
pub use parse::{parse_file, parse_source, ParseFail, ParsedModule};
