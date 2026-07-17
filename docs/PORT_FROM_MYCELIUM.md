# Port from mycelium-transpile (research snapshot)

**Source:** [`research/mycelium-transpile-snapshot/`](../research/mycelium-transpile-snapshot/)  
**Assessment:** [`research/ASSESSMENT_mycelium_transpile.md`](../research/ASSESSMENT_mycelium_transpile.md)  
**Constraint:** snapshot is **research-only** — not linked as a Cargo dependency; Mycelium monorepo is not modified.

## What we reuse

| Snapshot artifact | py2rust-core destination | How |
|-------------------|--------------------------|-----|
| `gap.rs` taxonomy + `GapReport` | `crates/py2rust-core/src/gap.rs` | **Port shape**: `Category`, `Gap`, `GapReport`, serde JSON, `expressible_fraction`, real vs advisory counts. Categories renamed to **Python-specific** set. |
| `transpile.rs` dispatch + never-silent invariant | `crates/py2rust-core/src/dispatch.rs` | **Port pattern**: exhaustive top-level walk; every stmt → emit, gap, or both; catch-all → `Other`. |
| `batch.rs` + `union.gap.json` | `crates/py2rust-core/src/batch.rs` | **Port**: discover files, per-file transpile, `summary.json`, `UnionGapReport`. |
| `vet.rs` dual metrics idea | (not yet) | **Port idea later**: optional `cargo check` → `checked_fraction` vs `expressible_fraction`. |
| `symtab.rs` | (not yet) | Light port when multi-module resolve is needed. |
| `emit.rs` / `prim_map` / `reserved` | **Do not port** | Mycelium `.myc` surface-specific. |
| `transpile-vet` skill docs | docs only | Process notes. |

## What we deliberately do **not** copy

- Nodule layout, `.myc` emission, `myc check` oracle
- Rust→Mycelium type maps / prim tables
- Live dependency on the Mycelium monorepo or publishing the snapshot crate

## Honesty rules adopted (G2 / VR-5 spirit)

1. **Never-silent** at module top-level: covered by `dispatch` + unit tests.
2. **No silent TODO bodies** without a `FunctionBody` / `MultiStmtBody` gap record.
3. **Categories over allowlists**: README limitations are gap enum arms, not silent green.
4. **Declared emission**: text in `.rs` is heuristic until a vet pass proves otherwise.
5. **Denominator honesty**: only exclude categories that are truly non-surface (none for Python MVP).

## Provenance

See [`research/mycelium-transpile-snapshot/PROVENANCE.md`](../research/mycelium-transpile-snapshot/PROVENANCE.md).
