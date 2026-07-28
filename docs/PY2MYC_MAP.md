# Python → Mycelium-native mapping

**Status:** 0.x scaffolding (this PR)  
**Contract:** [`crates/py2rust-core/src/interface.rs`](../crates/py2rust-core/src/interface.rs)  
**Table:** [`crates/py2rust-core/src/myc_map.rs`](../crates/py2rust-core/src/myc_map.rs)  
**Gap bridge:** [`crates/py2rust-core/src/gap.rs`](../crates/py2rust-core/src/gap.rs)

## Intent

Close gaps so a pipeline of **Python → Mycelium-native** can skip an interim
Rust route. Rust remains the **reference** (how the research snapshot learned
syn → surface mapping), not the destination.

## Common interface (stable)

| Type | Role |
|------|------|
| `PyConstruct` | Closed enum + `Other(String)` mirroring `gap::Category` (`FunctionBody` ↔ `PartialEmit`) |
| `MycForm` | `{ surface, authority, guarantee }` — Mycelium spelling + honesty |
| `Guarantee` | `Exact` \| `Empirical` \| `Refusal` (from `lib/std/math.myc` vocabulary) |
| `Citation` | `Corpus` \| `Test` \| `Unverified` — Unverified is first-class (VR-5) |
| `MapOutcome` | `Mapped(MycForm)` \| `Unmappable { construct, reason, needed }` — **no third case** |

Coverage is assessable: `Mapped / (Mapped + Unmappable)` over
`taxonomy_constructs()`.

## Module ownership

| Module | Owner lane |
|--------|------------|
| `interface.rs` | interface-owner only |
| `myc_map.rs` + `fixtures/myc_map/` | surface-mapper only |
| `gap.rs` extensions (Category ↔ PyConstruct) | gap-closer only |
| `tests/myc_map_coverage.rs` | verifier only |

## What changed in this PR

1. Added the stable mapping contract (`interface`).
2. Filled a total taxonomy table with Mycelium-native forms or explicit refusals
   (`myc_map`) — not Rust transliterations.
3. Bridged `gap::Category` ↔ `PyConstruct` and `Gap::myc_map_outcome()`.
4. Verifier tests for full taxonomy accounting and citation honesty.

## What this does **not** do

- Does not modify Mycelium grammar or evaluator (`{ a; b }` surface work is
  owned by mycelium-l1).
- Does not emit `.myc` from the Python driver yet — only the **map** that makes
  100% coverage assessable.
- Does not cut a 1.x release (repo stays 0.x.x under commitizen).

## Taxonomy snapshot (high level)

See `fixtures/myc_map/README.md` for the row table. Run:

```bash
cargo test -p py2rust-core --test myc_map_coverage
```
