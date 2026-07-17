# Assessment — `mycelium-transpile` machinery for py2rust (and tgar-rs process)

**Worker:** P23b (L1) · **Model:** composer-2.5-fast  
**Snapshot:** `research/mycelium-transpile-snapshot/` ([PROVENANCE.md](mycelium-transpile-snapshot/PROVENANCE.md))  
**Plan:** `/root/work/plans/fractal/P23_TRANSPILE_SCAVENGE.md`  
**Upstream grounding:** Mycelium DN-34 (Rust→Mycelium transpiler strategy), M-873/`trx`, M-1000/`trx2` vet loop

---

## Executive summary

`mycelium-transpile` is a mature **honesty-first** reference for how a language transpiler should behave when it cannot fully lower source to target: structured gap JSON, exhaustive top-level dispatch, conservative coverage metrics, and batch-aware symbol resolution. It is **wrong-direction** for tgar-rs emission (Rust→Mycelium, not Python→Rust) but **right-process** for porting discipline. For py2rust today—a thin scaffold with a four-pattern allowlist analyzer and placeholder function bodies—the highest-value imports are **gap taxonomy + never-silent driver invariant**, **per-file `.gap.json` artifacts**, and **dual metrics** (emitted vs toolchain-checked), adapted to `ast` + `rustc`/`cargo check` instead of `syn` + `myc check`.

---

## 1. Architecture — syn → map → emit → gap

### 1.1 Pipeline shape (no separate IR file)

The PoC does **not** lower to a dedicated intermediate `.ir` file. The shape is:

| Stage | Module(s) | Role |
|-------|-----------|------|
| **Parse** | `syn::parse_file` | Rust source → `syn::File` |
| **Driver** | `transpile.rs` | Per top-level `syn::Item`, `dispatch_item` → emit or gap |
| **Map** | `map.rs`, `type_map.rs`, `prim_map.rs` | Rust types/paths → target surface text, or `GapReason` |
| **Emit** | `emit.rs` (+ `emit/*`) | Visitor-style lowering to `.myc` strings; `EmitCtx` (resolvable sets, layouts, symtab) |
| **Gap** | `gap.rs` | `Gap`, `GapReport`, `Category` taxonomy, serde JSON |
| **Batch** | `batch.rs`, `symtab.rs` | Multi-file discovery, two-pass symbol table, `summary.json`, `union.gap.json` |
| **Vet** | `vet.rs` | Optional oracle (`myc check`) → `checked_fraction` vs `expressible_fraction` |
| **Remap ledger** | `remap.rs` | DN-109 provenance manifest (idiom choices), not on hot path |

**py2rust analogue:** Python `ast.parse` → per-statement/node dispatcher → `TypeMapper` (Rust types) → `RustEmitter` → `GapReport` JSON. No need to copy Mycelium’s nodule layout or `emit.rs` size; the **seams** are what matter.

### 1.2 Outcomes per construct

Every top-level item ends in one of:

- **Emitted** — `.myc` chunk + name in `emitted_items` (may carry `sub_gaps` for partial fidelity, e.g. `NamedFieldDrop`, unresolved `use` leaves).
- **Gap** — structured record with `category`, `reason`, `snippet`, span, optional `item_name`.
- **TestExcluded** — recorded as `Category::TestItem`, excluded from coverage denominator.

The catch-all `_` arm on `#[non_exhaustive] syn::Item` is itself a gap—never a silent no-op.

### 1.3 Context installed per file

Before the item loop, the driver computes:

- **`resolvable_type_names`** — greatest fixed point over in-file struct/enum field deps (M-1006 gate: do not emit records that reference unmapped external types and poison `myc check`).
- **`struct_layouts` / field type maps** — positional constructor discipline + collision refusal (G2: no silent wrong-index bind).
- **`imported_type_keys`** — batch `use` resolution hints for cross-module calls.

These are **resolvability gates**: emit only when downstream oracle failure mode is understood.

---

## 2. Gap reporting honesty model (VR-5 / G2 lessons for py2rust)

Mycelium’s corpus uses:

- **G2 (never-silent):** Every construct is emitted, gapped, or both—never neither. Silent drops are a process failure.
- **VR-5 (guarantee lattice):** Tags like `Declared` vs `Empirical` are **per operation**, never upgraded past their verification basis.

### 2.1 What mycelium-transpile does well

1. **`Err(GapReason)` not placeholders** — Unmapped types, qualified paths, and macro bodies return explicit reasons; no fabricated `todo!()` in emitted target syntax (DN-34 flag-don’t-guess).
2. **Fine-grained `Category`** — `rust_construct` mirrors `category`, not coarse `syn::Item` kind, so union backlogs (`union.gap.json`, `UNION-BACKLOG.md`) rank real blockers.
3. **Denominator honesty** — `TestItem` and bodyless `mod foo;` (`ModuleDecl`) are recorded but **excluded** from expressible-fraction denominator; real gaps stay in the denominator (no flattering coverage).
4. **Advisory vs real gaps** — `DeriveSatisfied` is `is_non_gap_advisory()` so headline counts do not inflate; `NamedFieldDrop` stays a real fidelity gap.
5. **Vet loop conservatism** — `checked_fraction` credits items only when the **whole file** passes `myc check`; per-item blame on failure is refused (VR-5).
6. **Tool absence** — `VetClass::ToolUnavailable` never counts as clean.
7. **Tests** — `src/tests/invariant.rs` corpus enforces never-silent bound (Empirical/Declared, not Proven—`syn::Item` is `#[non_exhaustive]`).

### 2.2 py2rust anti-pattern today

`CompatibilityAnalyzer` in `src/py2rust/cli.py` only flags four constructs (`Import`, `ClassDef`, `Try`, `Lambda`) via `ast.walk`. **Everything else passes silently**—the exact anti-pattern `mycelium-transpile`’s `lib.rs` calls out. `PythonToRustTranspiler` emits `// TODO: Implement function body` without recording that the body was not lowered (silent partial emission).

### 2.3 Adoptable honesty rules for py2rust

| Rule | Mycelium source | py2rust adoption |
|------|-----------------|------------------|
| Never-silent top-level | `transpile.rs` + invariant tests | Every `ast` module/class/function: emit, gap, or both |
| No guess on ambiguous types | `map.rs` qualified paths | Gap on `Any`, dynamic attrs, unknown imports |
| Structured JSON sidecar | `.gap.json` | `.gap.json` next to `.rs` (or stdout JSON mode) |
| Coverage fractions | `GapReport::expressible_fraction` | `lowered_fraction` + optional `cargo check` `verified_fraction` |
| Category taxonomy | `gap.rs` `Category` | Python-specific: `Async`, `Decorator`, `DynamicAttr`, `GIL`, `Exception`, … |
| Sub-gaps on partial emit | `Emitted.sub_gaps` | e.g. emitted signature but gapped body |

---

## 3. What to adopt in py2rust (P0 / P1 / P2)

### P0 — Do first (small diff, high leverage)

1. **`.gap.json` report** — Serialize `{ source, emitted_symbols[], gaps[], total_top_level, category_counts }` on `analyze` and `transpile` commands; mirror `Gap` fields (file, line, col, category, snippet, reason, symbol).
2. **Never-silent module walk** — Replace allowlist-only `CompatibilityAnalyzer` with dispatcher over `ast.Module.body` (functions, classes, imports, assignments at module level); unknown `ast` nodes → `Category::Other` gap.
3. **Stop silent TODO bodies** — If function body not lowered, either gap the whole function or emit stub **and** `sub_gap` `"function body not lowered (flag not guess)"`.
4. **pytest invariant** — One corpus file (mixed expressible + gapped Python) asserting `emitted + gaps >= top_level_items`.

### P1 — Next wave (multi-file + metrics)

5. **Batch / package mode** — Discover `*.py` under a package root (skip `tests/`, `__pycache__`), per-file `.gap.json`, `summary.json` + `union.gap.json` (pattern from `batch.rs`).
6. **Import resolution gaps** — For `import` / `from … import`, gap with precise reason (stdlib vs third-party vs relative) instead of generic string; optional sibling-module table for package layouts (lighter `symtab.rs`).
7. **Dual metrics** — `expressible_fraction` (text emitted) vs `checked_fraction` (file passes `cargo check` on generated crate); file-gated numerator like `vet.rs`.
8. **Category enum** — Closed set + `Other` with free-text reason; `real_gap_count()` excluding advisories.

### P2 — Deeper (when py2rust has real emit)

9. **Resolvability gate before struct emit** — Do not emit `struct` with field types that are gapped or external-without-stub (poisons `cargo check`).
10. **Recursion budget** — Port `gap.rs::guarded` pattern for deep `ast` trees (avoid stack blow-up; `Category::RecursionBudget`).
11. **Remap / idiom ledger** — JSON manifest of manual idiom choices (Python `dataclass` → Rust derive policy), DN-109-style provenance for human port steps.
12. **Diff harness** — Not textual equality; characterize matched/refined/absent items (`tests/diff.rs` vs hand port).

---

## 4. What not to adopt

| Mycelium piece | Why skip for py2rust |
|----------------|----------------------|
| `.myc` emission / grammar mapping | Wrong target language |
| `myc check` vet oracle | Use `cargo check` / `rustc` on emitted crate |
| `prim_map`, Mycelium `Binary{N}` / `Bytes` | Target is Rust types, not Mycelium repr |
| `reserved.rs` Mycelium keywords | Rust keyword collision is a different table |
| `nodule_path` / phylum batch vet | Python modules → Rust `mod`/`crate` layout differs |
| Full `emit.rs` (~6k LOC) | Copy patterns, not file |
| Co-include / L2-B batch type closure | Mycelium single-file oracle quirk; Rust modules are native |
| Positional constructor lowering | Rust structs can keep field names |
| Submodule of live Mycelium repo | Policy: analysis snapshot only ([PROVENANCE.md](mycelium-transpile-snapshot/PROVENANCE.md)) |

---

## 5. Relevance to tgar-rs (process only vs code)

**tgar-rs** ports **Python → Rust** (`tg-agent-relay`), same direction as py2rust, not Mycelium’s Rust→Mycelium direction.

| Useful (process) | Not useful (code) |
|------------------|-------------------|
| Gap lists per upstream `.py` module for PORTING.md phases | Any `.myc` or Mycelium symtab logic |
| Dual golden tests (Python behavior vs Rust) already in P22 | Transpiler crate vendored into tgar-rs |
| Honest “ported / partial / blocked” inventory from gap JSON | `mycelium-transpile` CLI in CI |
| Strangler phases with explicit unsupported surface | |

Cross-link: [tgar-rs `docs/TRANSPILE_RESEARCH.md`](https://github.com/tzervas/tgar-rs/blob/main/docs/TRANSPILE_RESEARCH.md) (pointer only).

---

## 6. Concrete first experiment for py2rust

**Experiment:** `py2rust analyze --json` (or `transpile` with `--report`) emitting **unsupported-construct gap JSON** without improving full lowering.

### Scope (one PR, P23c-sized)

1. Add `gap.py` with `Category` enum (minimal: `Import`, `ClassDef`, `AsyncDef`, `Try`, `Lambda`, `Decorator`, `Dynamic`, `Other`), `Gap`, `GapReport` (`dataclasses` + `json`).
2. Add `dispatch.py` walking `tree.body` only; for each top-level node, record gap or `emitted_symbols` entry.
3. Wire `analyze` to write `<stem>.gap.json` alongside optional human stdout.
4. Add `tests/fixtures/mixed_module.py` + `test_gap_invariant.py`.

### Success criteria

- Fixture with import + class + simple `def`: gaps ≥ 2, never-silent inequality holds.
- JSON schema stable enough for union rollup later.
- No change to default “compatible” message without `--json` (backward compatible).

### Sample gap JSON shape (aligned with snapshot)

```json
{
  "source": "example.py",
  "emitted_items": ["fn_add"],
  "gaps": [
    {
      "file": "example.py",
      "line": 1,
      "col": 0,
      "category": "Import",
      "python_construct": "Import",
      "snippet": "import os",
      "reason": "import lowering not implemented; flagged not guessed (VR-5/G2)",
      "item_name": null
    }
  ],
  "total_top_level_items": 3
}
```

Reference fixture: `research/mycelium-transpile-snapshot/fixtures/std-cmp.gap.json`.

---

## 7. Batch / dependency (relevance to multi-file Python)

`batch.rs` + `symtab.rs` implement:

- Deterministic `discover_rs_files` (skip `tests/` trees).
- **Two-pass batch transpile:** build symbol table of emitted names per file, then re-transpile with `transpile_file_with_ctx`.
- `common_ancestor` for output paths when batching multiple crate roots (collision fix M-1079).
- `union.gap.json` for portfolio-level gap ranking (`UNION-BACKLOG.md`).

**Python parallel:** Package `src/` with `from .foo import Bar` needs a **module graph** and “sibling emitted surface” table before confident `use`/path emission. py2rust P1 should copy the **two-pass idea**, not Mycelium’s nodule/phylum keys.

---

## 8. IR shape summary

| Question (P23 axis) | Answer |
|---------------------|--------|
| syn AST → intermediate → emit? | **AST-direct:** map+emit over `syn`, context in thread-local/`EmitCtx` |
| Gap objects? | **Yes:** first-class `Gap`/`GapReport`, JSON artifact |
| Batch sibling resolution? | **Yes:** `SymbolTable`, import co-include for oracle |
| Vet oracle? | **`myc check`** (replace with `cargo check` for py2rust) |

---

## 9. Top 5 recommendations (final)

1. **P0: Replace allowlist analyzer with never-silent gap JSON** — Biggest honesty win; matches G2/VR-5; low dependency on full transpiler.
2. **P0: Document dual metrics** — Emit `lowered_fraction` and (when crate exists) `checked_fraction` with file-gated credit; never claim parity from emission alone.
3. **P1: Python-specific gap taxonomy** — Closed `Category` set so tgar-rs/py2rust ports can share union backlog tooling.
4. **P1: Package batch mode + `union.gap.json`** — Enables “what blocks agent_handle.py?” style port planning for tgar-rs.
5. **P2: Resolvability gates before struct/type emit** — Prevents `cargo check` poisoned files from looking “partially ported”; copy the *fixed-point* idea from `resolvable_type_names`, not Mycelium types.

---

## References in this repo

- Snapshot README: `research/mycelium-transpile-snapshot/README.md`
- Skill summary: `research/transpile-vet-SKILL.md`
- py2rust current CLI: `src/py2rust/cli.py`
- Evidence: `/root/work/plans/evidence/P23-transpile-scavenge/`