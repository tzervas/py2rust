# py2rust — Roadmap

**Status:** Scaffolding / hygiene (2026-07-09)  
**Role:** Thin support repo for Python to Rust transpiler and code conversion assistant.  
**Living with:** [README.md](../README.md) · [AGENTS.md](../AGENTS.md) · workspace [plan.md](../../plan.md)

Tero-ready (docs/tero-index) and hygiene (scripts/check.sh) added per plan.md priority 1 (hygiene-thin-repos).

## Waves (minimal for scaffolding)

### Wave H — Hygiene & Tero closure (this chore)
- Add scripts/check.sh modeled on search-box/cabal (uv if present, ruff, pytest, tero index gen)
- Minimal docs/ROADMAP.md (this) + AGENTS append
- Land chore/tero-index-cabal-ready → dev (merge --no-ff), → main; propagate
- update-tero; verify checks

### Wave P — Polish & Integration
- cabal-devmelopner integration + tero for transpiler hints
- Expand conversion rules + tests
- CI parity

### Wave M — Python → Mycelium-native map (gap closure scaffolding)
- Stable contract: `PyConstruct` / `MycForm` / `Guarantee` / `Citation` / `MapOutcome`
- Total taxonomy table in `myc_map` (Mapped or Unmappable-with-reason; citations or Unverified)
- Bridge `gap::Category` ↔ `PyConstruct` (`FunctionBody` ↔ `PartialEmit`)
- Docs: [`PY2MYC_MAP.md`](PY2MYC_MAP.md) — does **not** touch mycelium-l1 grammar
- Next (out of scope here): driver emit path to `.myc` using the map; optional `myc check` vet

See workspace plan.md §4 for context. Cross-cite wsfull-wave-2026-07-09-compact.md .

Tero cite after: "py2rust hygiene"
