
# AGENTS.md — py2rust

**Use Tero + cabal-devmelopner for work here.**

## Tero (Layer-1 corpus index)

Repo has `docs/tero-index/index.json` (generated/ refreshed via tero-mcp/scripts/generate_lite_index.py).

**Rule:** Use tero queries before large greps or assumptions.
- Grok: tero__text_search / query_by_id (token "local-dev")
- Direct: tero-mcp-lite --index docs/tero-index/index.json
- cabal-devmelopner: auto-detects local index when run from within this tree (or set TERO_INDEX_PATH).

Example:
```bash
cd /root/git/py2rust
# agent with context:
uv run --project ../cabal-devmelopner cabal-devmelopner "task description here" --use-tero
```

Citations point at file:line — open them.

## Working with cabal-devmelopner agent tool

This project is prepared for integration:
- Tero index committed on chore/tero-index-cabal-ready (and PRable to dev)
- Local auto index support in cabal
- This AGENTS.md

**PR flow (protect main/dev):**
- Create/checkout feature or chore branch
- Make changes (agent will often use working branch)
- PR the branch → `dev` (then dev → main when ready)

## Local checks

Look for:
- scripts/check.sh
- Cargo.toml / pyproject.toml + standard commands (cargo test, uv run pytest, ruff, etc.)

Run checks before considering work complete.

## Further reading

- README.md
- docs/ROADMAP.md or ROADMAP.md (if present)
- docs/ASSESSMENT.md or similar for intent/gaps
- ../cabal-devmelopner/docs/* for agent architecture
- ../tero-mcp for how indexes are built and served

Leave mycelium isolated; all coordination here targets the other repos + cabal.

## Hygiene + Tero landing (chore/tero-index-cabal-ready, 2026-07-09 appended)

Tero-first (via /root/git/scripts/tero.sh identify + text_search "chore tero hygiene check ROADMAP scaffolding" + cites to AGENTS local-checks, tero-index).

- Added ruff to dev-deps + [tool.ruff] config in pyproject.toml (parity with cabal).
- Added scripts/check.sh (modeled on search-box/cabal + tero-mcp): uv sync → ruff format/check/fix, mypy advisory, pytest; + tero index gen.
- Added minimal docs/ROADMAP.md (scaffolding role, tero/hygiene ready, links workspace plan.md).
- Appended this (append-only).
- branch-guard (chore/tero-index-cabal-ready), dev-workflow, -S commits.
- Land: merge --no-ff → dev push; → main --no-ff push; propagate.
- Post: update-tero; commit; check.sh; tero.
- Cites: plan.md p1 hygiene-thin-repos, wsfull-wave...

Use `./scripts/check.sh`. cabal --use-tero supported.

Tero cite: agents--hygiene-tero-landing-2026-07-09-py2rust

