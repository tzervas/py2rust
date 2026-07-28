# py2rust — Tero Index (Layer 1)

> **Honesty:** Empirical/Declared — lite heading/line heuristic over markdown in py2rust via update_tero_index.py; source files are ground truth. Generated 2026-07-09.
> Use this index to find where to Read, not as authoritative ground truth.

- **Items:** 30
- **Flagged:** 0
- **item_tag:** `Empirical/Declared`
- **Machine index:** [`index.json`](./index.json)
- **Manifest:** [`MANIFEST.toml`](./MANIFEST.toml)

## doc (30 entries)

| Anchor | Kind | Id | Title | File:Line | Status | Summary |
|---|---|---|---|---|---|---|
| `agents` | other | — | AGENTS.md — py2rust | `AGENTS.md:2` | — | **Use Tero + cabal-devmelopner for work here.** |
| `agents--tero-layer-1-corpus-index` | section | — | Tero (Layer-1 corpus index) | `AGENTS.md:6` | — | Repo has `docs/tero-index/index.json` (generated/ refreshed via tero-mcp/scripts/generate_lite_in... |
| `agents` | other | — | agent with context: | `AGENTS.md:18` | — | uv run --project ../cabal-devmelopner cabal-devmelopner "task description here" --use-tero |
| `agents--working-with-cabal-devmelopner-agent-tool` | section | — | Working with cabal-devmelopner agent tool | `AGENTS.md:24` | — | This project is prepared for integration: |
| `agents--local-checks` | section | — | Local checks | `AGENTS.md:36` | — | Prefer the **Rust** product path: |
| `agents--further-reading` | section | — | Further reading | `AGENTS.md:53` | — | - README.md |
| `agents--hygiene-tero-landing-choretero-index-cabal-ready-2026-07-09-appended` | section | — | Hygiene + Tero landing (chore/tero-index-cabal-ready, 2026-07-09 appended) | `AGENTS.md:63` | — | Tero-first (via /root/git/scripts/tero.sh identify + text_search "chore tero hygiene check ROADMA... |
| `readme` | other | — | py2rust | `README.md:1` | — | [![CI](https://github.com/tzervas/py2rust/actions/workflows/fleet-ci.yml/badge.svg?branch=main)](... |
| `readme--status` | section | — | Status | `README.md:14` | — | \| Item \| State \| |
| `readme--5-minute-path` | section | — | 5-minute path | `README.md:24` | — | Requirements: Rust **1.85+** (see `rust-toolchain.toml`). |
| `readme` | other | — | → py2rust 0.2.0 | `README.md:36` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simple_fn.py |
| `readme` | other | — | Analyze fixture (writes .gap.json beside source) | `README.md:38` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simple_fn.py |
| `readme` | other | — | Transpile to a temp path | `README.md:41` | — | cargo run -p py2rust -- transpile crates/py2rust-core/fixtures/simple_fn.py -o /tmp/simple_fn.rs |
| `readme` | other | — | → /tmp/simple_fn.rs + /tmp/simple_fn.gap.json | `README.md:43` | — | ``` |
| `readme--example` | section | — | Example | `README.md:54` | — | ```python |
| `readme` | other | — | demo.py | `README.md:57` | — | def add(x: int, y: int) -> int: |
| `readme` | other | — | → demo.rs  (emits add) | `README.md:67` | — | ``` |
| `readme` | other | — | → demo.gap.json  (Class gap for Foo) | `README.md:68` | — | ``` |
| `readme--architecture` | section | — | Architecture | `README.md:71` | — | ```text |
| `readme--gap-categories-not-silent-failures` | section | — | Gap categories (not silent failures) | `README.md:92` | — | \| Category \| What is flagged \| |
| `readme--what-works-today-mvp` | section | — | What works today (MVP) | `README.md:110` | — | - Top-level `def foo(x: int) -> int` with a simple body (`return` constant, name, or binary op) |
| `readme--what-does-not-work-by-design-later-packages` | section | — | What does **not** work (by design / later packages) | `README.md:117` | — | - Full semantic equivalence Python ↔ Rust |
| `readme--development` | section | — | Development | `README.md:124` | — | ```bash |
| `readme--python-scaffold-deprecated` | section | — | Python scaffold (deprecated) | `README.md:132` | — | `src/py2rust/` remains as a **legacy** thin CLI. Prefer the Rust binary: |
| `readme--license` | section | — | License | `README.md:142` | — | MIT — see [LICENSE](LICENSE). |
| `roadmap` | note | — | py2rust — Roadmap | `docs/ROADMAP.md:1` | Scaffolding / hygiene (2026-07-09) | **Status:** Scaffolding / hygiene (2026-07-09) |
| `roadmap--waves-minimal-for-scaffolding` | section | — | Waves (minimal for scaffolding) | `docs/ROADMAP.md:9` | — | - Add scripts/check.sh modeled on search-box/cabal (uv if present, ruff, pytest, tero index gen) |
| `roadmap--wave-h-hygiene-tero-closure-this-chore` | section | — | Wave H — Hygiene & Tero closure (this chore) | `docs/ROADMAP.md:11` | — | - Add scripts/check.sh modeled on search-box/cabal (uv if present, ruff, pytest, tero index gen) |
| `roadmap--wave-p-polish-integration` | section | — | Wave P — Polish & Integration | `docs/ROADMAP.md:17` | — | - cabal-devmelopner integration + tero for transpiler hints |
| `roadmap--wave-m-python-mycelium-native-map-gap-closure-scaffolding` | section | — | Wave M — Python → Mycelium-native map (gap closure scaffolding) | `docs/ROADMAP.md:22` | — | - Stable contract: `PyConstruct` / `MycForm` / `Guarantee` / `Citation` / `MapOutcome` |
