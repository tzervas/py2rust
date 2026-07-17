# CLAUDE.md — py2rust

Short notes for Claude / coding agents working in this repo.

## Product

Rust-first Python → Rust transpiler with **honest gap reporting** (never silent success). Unlowered constructs become structured `.gap.json` categories.

Research snapshot under `research/` is **read-only analysis**, not a runtime dependency. Do not re-introduce mycelium as a build dep.

## 5-minute CLI

```bash
# From repo root
cargo build -p py2rust
cargo test --workspace

cargo run -p py2rust -- version
# → py2rust 0.2.0

# Analyze: issues + <stem>.gap.json
cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simple_fn.py

# Transpile: write .rs + .gap.json
cargo run -p py2rust -- transpile crates/py2rust-core/fixtures/simple_fn.py -o /tmp/simple_fn.rs

# Mixed fixture (gaps expected)
cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/mixed.py --json
```

Install:

```bash
cargo install --path crates/py2rust
py2rust analyze path/to/script.py
py2rust transpile path/to/script.py -o out.rs
```

## Layout

| Path | Role |
|------|------|
| `crates/py2rust` | CLI (`version`, `analyze`, `transpile`) |
| `crates/py2rust-core` | parse → dispatch → emit \| gap |
| `crates/py2rust-core/fixtures/` | never-silent corpus |
| `src/py2rust/` | **legacy** Python CLI — do not expand as primary path |
| `research/mycelium-transpile-snapshot/` | analysis only |

## Checks

```bash
cargo test --workspace
cargo check --workspace
./scripts/check.sh   # when Python scaffold hygiene is needed
```

## Rules

1. Prefer Rust CLI (`cargo run -p py2rust -- …`) over the deprecated Python package.
2. Never claim full semantic Python↔Rust equivalence; report gaps.
3. Branch → PR to `dev` (then promote `dev` → `main`).
4. See `AGENTS.md` for tero/cabal notes and `docs/ROADMAP.md` for waves.
