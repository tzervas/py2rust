# py2rust

<!-- FLEET-BADGES:BEGIN -->
[![CI](https://github.com/tzervas/py2rust/actions/workflows/fleet-ci.yml/badge.svg?branch=main)](https://github.com/tzervas/py2rust/actions/workflows/fleet-ci.yml?query=branch%3Amain)
[![Security](https://github.com/tzervas/py2rust/actions/workflows/fleet-security.yml/badge.svg?branch=main)](https://github.com/tzervas/py2rust/actions/workflows/fleet-security.yml?query=branch%3Amain)
<!-- FLEET-BADGES:END -->

**Rust-first** Python → Rust transpiler with an honesty-first gap model.

**Who / what / why:** porters (e.g. tg-agent-relay → tgar-rs) who need a best-effort Python→Rust lowerer that **never silently drops** constructs — unlowered code becomes structured **gap categories** in a sibling `.gap.json`. Basic typed functions can emit Rust; everything else is flagged.

> Research snapshot of the Mycelium transpile honesty machinery lives under [`research/mycelium-transpile-snapshot/`](research/mycelium-transpile-snapshot/) — analysis only, not a runtime dependency. See [`docs/PORT_FROM_MYCELIUM.md`](docs/PORT_FROM_MYCELIUM.md).

## Status

| Item | State |
|------|--------|
| Version | **0.2.0** (Rust workspace tip on `dev` / promoted `main`) |
| CLI | `version`, `analyze`, `transpile` |
| Emit | simple typed `def` bodies (`return` / names / binary ops) |
| Gaps | Class, Exception, DynamicTyping, Lambda, … (see table below) |
| Python package under `src/` | **legacy** — prefer Rust binary |

## 5-minute path

Requirements: Rust **1.85+** (see `rust-toolchain.toml`).

```bash
git clone https://github.com/tzervas/py2rust.git
cd py2rust

cargo build -p py2rust
cargo test --workspace

cargo run -p py2rust -- version
# → py2rust 0.2.0

# Analyze fixture (writes .gap.json beside source)
cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simple_fn.py

# Transpile to a temp path
cargo run -p py2rust -- transpile crates/py2rust-core/fixtures/simple_fn.py -o /tmp/simple_fn.rs
# → /tmp/simple_fn.rs + /tmp/simple_fn.gap.json
```

Install the CLI onto your `PATH`:

```bash
cargo install --path crates/py2rust
py2rust analyze path/to/script.py
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
