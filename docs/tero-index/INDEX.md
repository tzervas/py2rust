# py2rust — Tero Index (Layer 1)

> **Honesty:** Empirical/Declared — lite heading/line heuristic over markdown in py2rust via tero-mcp/scripts/generate_lite_index.py; source files are ground truth. Generated 2026-07-29.
> Use this index to find where to Read, not as authoritative ground truth.

- **Items:** 64
- **Flagged:** 0
- **item_tag:** `Empirical/Declared`
- **Machine index:** [`index.json`](./index.json)
- **Manifest:** [`MANIFEST.toml`](./MANIFEST.toml)

## doc (64 entries)

| Anchor | Kind | Id | Title | File:Line | Status | Summary |
|---|---|---|---|---|---|---|
| `agents` | other | — | AGENTS.md — py2rust | `AGENTS.md:2` | — | Use Tero + cabal-devmelopner for work here. |
| `agents--tero-layer-1-corpus-index` | section | — | Tero (Layer-1 corpus index) | `AGENTS.md:6` | — | Repo has docs/tero-index/index.json (generated/ refreshed via tero-mcp/scripts/generateliteindex.py). |
| `agents--agent-with-context` | other | — | agent with context: | `AGENTS.md:18` | — | uv run --project ../cabal-devmelopner cabal-devmelopner "task description here" --use-tero |
| `agents--working-with-cabal-devmelopner-agent-tool` | section | — | Working with cabal-devmelopner agent tool | `AGENTS.md:24` | — | This project is prepared for integration: |
| `agents--local-checks` | section | — | Local checks | `AGENTS.md:36` | — | Prefer the Rust product path: |
| `agents--further-reading` | section | — | Further reading | `AGENTS.md:53` | — | - README.md |
| `agents--hygiene-tero-landing-chore-tero-index-cabal-ready-2026-07-09-appended` | section | — | Hygiene + Tero landing (chore/tero-index-cabal-ready, 2026-07-09 appended) | `AGENTS.md:63` | — | Tero-first (via /root/git/scripts/tero.sh identify + textsearch "chore tero hygiene check ROADMAP scaffolding" + cites to AGENTS local-checks, tero-index). |
| `claude` | section | — | CLAUDE.md — py2rust | `CLAUDE.md:1` | — | Short notes for Claude / coding agents working in this repo. |
| `claude--product` | section | — | Product | `CLAUDE.md:5` | — | Rust-first Python → Rust transpiler with honest gap reporting (never silent success). Unlowered constructs become structured .gap.json categories. |
| `claude--5-minute-cli` | section | — | 5-minute CLI | `CLAUDE.md:11` | — | cargo build -p py2rust |
| `claude--from-repo-root` | other | — | From repo root | `CLAUDE.md:14` | — | cargo build -p py2rust |
| `claude--py2rust-0.2.0` | other | — | → py2rust 0.2.0 | `CLAUDE.md:19` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simplefn.py |
| `claude--analyze-issues-stem.gap.json` | other | — | Analyze: issues + <stem>.gap.json | `CLAUDE.md:21` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simplefn.py |
| `claude--transpile-write-.rs-.gap.json` | other | — | Transpile: write .rs + .gap.json | `CLAUDE.md:24` | — | cargo run -p py2rust -- transpile crates/py2rust-core/fixtures/simplefn.py -o /tmp/simplefn.rs |
| `claude--mixed-fixture-gaps-expected` | other | — | Mixed fixture (gaps expected) | `CLAUDE.md:27` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/mixed.py --json |
| `claude--layout` | section | — | Layout | `CLAUDE.md:39` | — | — |
| `claude--checks` | section | — | Checks | `CLAUDE.md:49` | — | cargo test --workspace |
| `claude--rules` | section | — | Rules | `CLAUDE.md:57` | — | 1. Prefer Rust CLI (cargo run -p py2rust -- …) over the deprecated Python package. |
| `readme` | other | — | py2rust | `README.md:1` | — | <!-- FLEET-BADGES:BEGIN --> |
| `readme--status` | section | — | Status | `README.md:14` | — | Requirements: Rust 1.85+ (see rust-toolchain.toml). |
| `readme--5-minute-path` | section | — | 5-minute path | `README.md:24` | — | Requirements: Rust 1.85+ (see rust-toolchain.toml). |
| `readme--py2rust-0.2.0` | other | — | → py2rust 0.2.0 | `README.md:36` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simplefn.py |
| `readme--analyze-fixture-writes-.gap.json-beside-source` | other | — | Analyze fixture (writes .gap.json beside source) | `README.md:38` | — | cargo run -p py2rust -- analyze crates/py2rust-core/fixtures/simplefn.py |
| `readme--transpile-to-a-temp-path` | other | — | Transpile to a temp path | `README.md:41` | — | cargo run -p py2rust -- transpile crates/py2rust-core/fixtures/simplefn.py -o /tmp/simplefn.rs |
| `readme--tmp-simplefn.rs-tmp-simplefn.gap.json` | other | — | → /tmp/simple_fn.rs + /tmp/simple_fn.gap.json | `README.md:43` | — | Install the CLI onto your PATH: |
| `readme--example` | section | — | Example | `README.md:54` | — | def add(x: int, y: int) -> int: |
| `readme--demo.py` | other | — | demo.py | `README.md:57` | — | def add(x: int, y: int) -> int: |
| `readme--demo.rs-emits-add` | other | — | → demo.rs  (emits add) | `README.md:67` | — | crates/ |
| `readme--demo.gap.json-class-gap-for-foo` | other | — | → demo.gap.json  (Class gap for Foo) | `README.md:68` | — | crates/ |
| `readme--architecture` | section | — | Architecture | `README.md:71` | — | crates/ |
| `readme--gap-categories-not-silent-failures` | section | — | Gap categories (not silent failures) | `README.md:92` | — | — |
| `readme--what-works-today-mvp` | section | — | What works today (MVP) | `README.md:110` | — | - Top-level def foo(x: int) -> int with a simple body (return constant, name, or binary op) |
| `readme--what-does-not-work-by-design-later-packages` | section | — | What does **not** work (by design / later packages) | `README.md:117` | — | - Full semantic equivalence Python ↔ Rust |
| `readme--development` | section | — | Development | `README.md:124` | — | cargo test --workspace |
| `readme--python-scaffold-deprecated` | section | — | Python scaffold (deprecated) | `README.md:132` | — | src/py2rust/ remains as a legacy thin CLI. Prefer the Rust binary: |
| `readme--license` | section | — | License | `README.md:142` | — | MIT — see [LICENSE](LICENSE). |
| `reuse-debt` | section | — | REUSE debt — py2rust (P24f bootstrap) | `REUSE-DEBT.md:1` | — | Generated by P24f REUSE bootstrap. Full REUSE compliance is deferred; |
| `reuse-debt--missing-copyright-and-licensing-information` | other | — | MISSING COPYRIGHT AND LICENSING INFORMATION | `REUSE-DEBT.md:9` | — | The following files have no copyright and licensing information: |
| `reuse-debt--summary` | other | — | SUMMARY | `REUSE-DEBT.md:20` | — |  Bad licenses: 0 |
| `reuse-debt--recommendations` | other | — | RECOMMENDATIONS | `REUSE-DEBT.md:36` | — |  Fix missing copyright/licensing information: For one or more files, the tool |
| `fleetstandards` | section | — | Fleet standards (tzervas) | `docs/FLEET_STANDARDS.md:1` | — | Applied from the workstation pack under plans/fleet-standards/pack/. |
| `fleetstandards--workflows` | section | — | Workflows | `docs/FLEET_STANDARDS.md:5` | — | - dev / feature merges: Refs #n only — issues stay open |
| `fleetstandards--issue-close-policy` | section | — | Issue close policy | `docs/FLEET_STANDARDS.md:14` | — | - dev / feature merges: Refs #n only — issues stay open |
| `fleetstandards--badges` | section | — | Badges | `docs/FLEET_STANDARDS.md:20` | — | README badges use GitHub Actions SVG for trunk branch — live status, not static green. |
| `fleetstandards--copilot` | section | — | Copilot | `docs/FLEET_STANDARDS.md:24` | — | Automatic Copilot code reviews are disabled for fleet-managed repos. Do not request Copilot on PRs. |
| `fleetstandards--permissions` | section | — | Permissions | `docs/FLEET_STANDARDS.md:28` | — | Workflows use minimum permissions: blocks (contents read; issues write only for close/reopen jobs). |
| `portfrommycelium` | section | — | Port from mycelium-transpile (research snapshot) | `docs/PORT_FROM_MYCELIUM.md:1` | — | Source: [research/mycelium-transpile-snapshot/](../research/mycelium-transpile-snapshot/) |
| `portfrommycelium--what-we-reuse` | section | — | What we reuse | `docs/PORT_FROM_MYCELIUM.md:7` | — | — |
| `portfrommycelium--what-we-deliberately-do-not-copy` | section | — | What we deliberately do **not** copy | `docs/PORT_FROM_MYCELIUM.md:19` | — | - Nodule layout, .myc emission, myc check oracle |
| `portfrommycelium--honesty-rules-adopted-g2-vr-5-spirit` | section | — | Honesty rules adopted (G2 / VR-5 spirit) | `docs/PORT_FROM_MYCELIUM.md:25` | — | 1. Never-silent at module top-level: covered by dispatch + unit tests. |
| `portfrommycelium--python-mycelium-native-map-this-tree` | section | — | Python → Mycelium-native map (this tree) | `docs/PORT_FROM_MYCELIUM.md:33` | — | Beyond the Rust emission path, interface + mycmap record the correct |
| `portfrommycelium--provenance` | section | — | Provenance | `docs/PORT_FROM_MYCELIUM.md:46` | — | See [research/mycelium-transpile-snapshot/PROVENANCE.md](../research/mycelium-transpile-snapshot/PROVENANCE.md). |
| `py2mycmap` | section | — | Python → Mycelium-native mapping | `docs/PY2MYC_MAP.md:1` | 0.x scaffolding (this PR) | Status: 0.x scaffolding (this PR) |
| `py2mycmap--intent` | section | — | Intent | `docs/PY2MYC_MAP.md:8` | — | Close gaps so a pipeline of Python → Mycelium-native can skip an interim |
| `py2mycmap--common-interface-stable` | section | — | Common interface (stable) | `docs/PY2MYC_MAP.md:14` | — | Coverage is assessable: Mapped / (Mapped + Unmappable) over |
| `py2mycmap--module-ownership` | section | — | Module ownership | `docs/PY2MYC_MAP.md:27` | — | 1. Added the stable mapping contract (interface). |
| `py2mycmap--what-changed-in-this-pr` | section | — | What changed in this PR | `docs/PY2MYC_MAP.md:36` | — | 1. Added the stable mapping contract (interface). |
| `py2mycmap--what-this-does-not-do` | section | — | What this does **not** do | `docs/PY2MYC_MAP.md:44` | — | - Does not modify Mycelium grammar or evaluator ({ a; b } surface work is |
| `py2mycmap--taxonomy-snapshot-high-level` | section | — | Taxonomy snapshot (high level) | `docs/PY2MYC_MAP.md:52` | — | See fixtures/mycmap/README.md for the row table. Run: |
| `roadmap` | note | — | py2rust — Roadmap | `docs/ROADMAP.md:1` | Scaffolding / hygiene (2026-07-09) | Status: Scaffolding / hygiene (2026-07-09) |
| `roadmap--waves-minimal-for-scaffolding` | section | — | Waves (minimal for scaffolding) | `docs/ROADMAP.md:9` | — | - Add scripts/check.sh modeled on search-box/cabal (uv if present, ruff, pytest, tero index gen) |
| `roadmap--wave-h-hygiene-tero-closure-this-chore` | section | — | Wave H — Hygiene & Tero closure (this chore) | `docs/ROADMAP.md:11` | — | - Add scripts/check.sh modeled on search-box/cabal (uv if present, ruff, pytest, tero index gen) |
| `roadmap--wave-p-polish-integration` | section | — | Wave P — Polish & Integration | `docs/ROADMAP.md:17` | — | - cabal-devmelopner integration + tero for transpiler hints |
| `roadmap--wave-m-python-mycelium-native-map-gap-closure-scaffolding` | section | — | Wave M — Python → Mycelium-native map (gap closure scaffolding) | `docs/ROADMAP.md:22` | — | - Stable contract: PyConstruct / MycForm / Guarantee / Citation / MapOutcome |

