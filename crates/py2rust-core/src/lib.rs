//! `py2rust-core` — Python → Rust transpiler library.
//!
//! Pipeline: Python source → parse (`rustpython-parser`) → never-silent
//! [`dispatch`] over `Module.body` → best-effort Rust emission + structured
//! [`.gap.json`](gap::GapReport) (G2 / VR-5 honesty patterns ported from the
//! mycelium-transpile research snapshot; **not** a Mycelium dependency).
//!
//! # Guarantee tags (VR-5)
//!
//! - Emitted Rust is **Declared**: heuristic lowering, not validated by `rustc`.
//! - Never-silent invariant (every top-level item is emitted, gapped, or both) is
//!   checked over a fixed fixture corpus — **Empirical/Declared**, not Proven
//!   (`Stmt` exhaustiveness rests on a catch-all arm).

pub mod batch;
pub mod dispatch;
pub mod emit;
pub mod gap;
pub mod map;
pub mod parse;
pub mod source_loc;

pub use batch::{
    discover_py_files, transpile_batch, BatchSummary, FileResult, UnionGapReport,
};
pub use dispatch::{
    analyze_file, analyze_source, dispatch_stmt, transpile_file, transpile_source, DispatchError,
    Outcome,
};
pub use gap::{gap_json_path, Category, Gap, GapReason, GapReport, GAP_SCHEMA_VERSION};
pub use parse::{parse_file, parse_source, ParseFail, ParsedModule};
