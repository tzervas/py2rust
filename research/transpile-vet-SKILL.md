---
name: transpile-vet
description: >-
  Run the Rust→Mycelium transpiler WITH its real-toolchain vet loop (M-1000/M-1001, kickoff trx2):
  transpile targets, `myc check` every emitted `.myc`, and read the two honesty-split metrics —
  `expressible_fraction` (text emitted) vs **`checked_fraction`** (myc-check-clean, the number that
  matters). Per the M-991 verdict (DN-34 §8.7–§8.8, `Empirical`): the transpiler is a **gap-profiling
  instrument, not a bulk porter** — emitted `.myc` is a `Declared` starting point until a
  differential upgrades it; a mostly-gapped result is a successful, honest output (G2/VR-5).
when_to_use: >-
  Use when assessing how much of a Rust crate/module the toolchain can already express (before a
  port), when profiling which gap classes block a port target (M-993 semcore, the M-1006 ladder),
  or after changing the transpiler/emitter to measure the real effect. NOT for producing "finished"
  ports — that claim needs the differential. For the committed draft corpus + graduation workflow,
  use /myc-drafts instead.
allowed-tools: Bash(just transpile-vet:*), Bash(scripts/checks/transpile-vet.sh:*), Bash(cargo run:*), Bash(cargo build:*), Bash(cargo test:*)
---

# transpile-vet

The operational form of **M-1000/M-1001** (`crates/mycelium-transpile`): the transpile → `myc check`
→ classify loop that turned "coverage" from an emission heuristic into a toolchain-checked number.

## Run it

```
just transpile-vet <input.rs|input-dir> <out-dir>     # the wrapper (recommended)
bash scripts/checks/transpile-vet.sh <input> <out>    # same thing, direct
```

The wrapper **builds the `myc-check` oracle once** and hands the binary to the transpiler via
`MYC_CHECK_CMD` — never pay a nested `cargo run` per file. Direct CLI (when you need flags):

```
cargo run -p mycelium-transpile -- --vet <input.rs|input-dir> <out-dir>
```

Directory inputs: every target gets its own out-dir if you loop crates yourself — batch mode warns
loudly on stem collisions (12 stdlib crates all have `lib.rs`) and last-write-wins.

## Read the output

| Artifact | What it is |
|---|---|
| `<stem>.myc` | the emitted draft — **`Declared`**, a starting point, never a port |
| `<stem>.gap.json` | per-item gaps: 17 closed `Category` classes + reason + source location |
| `vet.json` | per-file vet records: exit class (`0 ok · 2 parse · 3 check · 64 usage · 66 io`) + diagnostic |
| `summary.json` / `union.gap.json` | dir-mode aggregates |

**The two metrics — never conflate them (VR-5):**
- `expressible_fraction` — items for which *some* text was emitted. Historically over-counted
  (emissions that poison the checker); useful only as an upper envelope.
- **`checked_fraction`** — items in files whose *entire* emitted `.myc` is myc-check-clean.
  **File-gated, honestly conservative** (a failing file credits 0; denominator = non-test top-level
  items, so `checked ≤ expressible` always). This is the port-planning number.
- An unknown exit code / signal classifies `ToolUnavailable` and is **never counted clean** (G2).

## Honesty rules (bind every use)

1. Emission is **`Declared`**; a vet verdict is **`Empirical`** (measured, this toolchain, this
   commit). Only a Rust-oracle differential upgrades a draft beyond that — see `/myc-dogfood` and
   DN-26's stage conventions.
2. Never fix a gap by fabricating a body the emitter can't faithfully lower — fallback arms return
   `Err(GapReason)` (the DN-34 §8 honesty-correction precedent). New emission paths trace to a
   production in `docs/spec/grammar/mycelium.ebnf`.
3. Reserved-word/`use` handling is guard-first (`src/reserved.rs`, declaration sites included —
   PR #1207): a collision is a **gap**, not an emit-and-hope.
4. If you publish numbers (a claim), record them **append-only** in DN-34 §8.x with the commit SHA
   and denominators — numbers are true *as dated*; a later emitter change does not rewrite them.

## Calibration (so you don't re-derive it)

Wave-1 ground truth (DN-34 §8.9, 2026-07-06): union `checked_fraction` **3.7%** over the 17-target
boot10 port surface; best stdlib target 33.3% (tiny), semcore ≤2.4%. The residual 812-gap worklist
(type-coverage 322 · Impl 119 · Import 117 · Struct 80 · GenericBound 59 · tail) is the M-1006
ladder's input. Expect profiling value, not porting value, until those classes close.
