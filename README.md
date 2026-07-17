# py2rust

**Rust-first** Python → Rust transpiler with an honesty-first gap model.

Unlowered Python is reported as structured **gap categories** (never silent success). Basic typed functions can emit best-effort Rust; everything else is flagged in a sibling `.gap.json`.

> Research snapshot of the Mycelium transpile honesty machinery lives under [`research/mycelium-transpile-snapshot/`](research/mycelium-transpile-snapshot/) — analysis only, not a runtime dependency. See [`docs/PORT_FROM_MYCELIUM.md`](docs/PORT_FROM_MYCELIUM.md).

## Install (Rust)

Requirements: Rust **1.85+** (see `rust-toolchain.toml`).

```bash
git clone https://github.com/tzervas/py2rust.git
cd py2rust
cargo build -p py2rust --release

# or run without installing
cargo run -p py2rust -- version
```

Install the CLI onto your `PATH`:

```bash
cargo install --path crates/py2rust
```

## Usage

```bash
# Version
py2rust version
# or: cargo run -p py2rust -- version

# Analyze: print issues + write <stem>.gap.json
py2rust analyze path/to/script.py

# Transpile: write .rs + .gap.json
py2rust transpile path/to/script.py
py2rust transpile path/to/script.py -o out/script.rs
```

### Example

```python
# demo.py
def add(x: int, y: int) -> int:
    return x + y

class Foo:
    pass
```

```bash
cargo run -p py2rust -- transpile demo.py
# → demo.rs  (emits add)
# → demo.gap.json  (Class gap for Foo)
```

## Architecture

```text
crates/
  py2rust/        # CLI (clap): version, analyze, transpile
  py2rust-core/   # library
    parse/        # rustpython-parser
    gap/          # Category, Gap, GapReport, serde JSON
    dispatch/     # never-silent walk of module body
    emit/         # best-effort Rust for simple typed defs
    batch/        # multi-file + summary.json + union.gap.json
research/         # mycelium-transpile snapshot (read-only)
```

**Pipeline:** Python source → parse → dispatch (emit | gap | both) → `.rs` + `.gap.json`

## Gap categories (not silent failures)

| Category | What is flagged |
|----------|-----------------|
| `Class` | classes & inheritance |
| `Exception` | `try` / `except` / `raise` |
| `DynamicTyping` | unannotated params, `Any`, unmapped annotations |
| `Metaprogramming` | decorators, `exec` / `eval` (when encountered) |
| `Async` | `async def` / `async with` / `async for` |
| `Import` | imports without a confirmed Rust mapping |
| `Lambda` | `lambda` → closure policy pending |
| `Comprehension` | list/dict/set/generator comps |
| `MultiStmtBody` | complex multi-statement bodies |
| `FunctionBody` | signature may emit; body not fully lowered (`sub_gap`) |
| `Other` | catch-all — **never silent** |

Coverage metric: `expressible_fraction` = emitted top-level items / non-excluded top-level items (**Declared** heuristic, not `cargo check` verified).

## What works today (MVP)

- Top-level `def foo(x: int) -> int` with a simple body (`return` constant, name, or binary op)
- Annotated params mapped: `int`→`i64`, `float`→`f64`, `bool`→`bool`, `str`→`String`
- Structured `.gap.json` on `analyze` and `transpile`
- Never-silent invariant over a fixed corpus (`cargo test`)

## What does **not** work (by design / later packages)

- Full semantic equivalence Python ↔ Rust
- Classes, exceptions, async, imports, lambdas (gapped, not faked)
- `cargo check` dual-metric vet loop (planned)
- Package-scale batch CLI surface (library `batch` module exists; CLI may grow)

## Development

```bash
cargo test --workspace
cargo check --workspace
cargo run -p py2rust -- analyze some.py
```

### Python scaffold (deprecated)

`src/py2rust/` remains as a **legacy** thin CLI. Prefer the Rust binary:

```bash
cargo run -p py2rust -- <args>
```

Do not expand the Python package as the primary product path.

## License

MIT — see [LICENSE](LICENSE).
