# Build status — analysis-only snapshot

This directory is **`research/mycelium-transpile-snapshot/`** in the **py2rust** repo. It is intentionally **not** a member of any Cargo workspace here (py2rust is a Python package; there is no root `Cargo.toml`).

## Default expectation

- **Primary use:** read and compare sources (gap reporting, emit discipline, batch IR, vet loop) for py2rust design — see upcoming `research/ASSESSMENT_mycelium_transpile.md` (P23b).
- **Not required** for `scripts/check.sh`, CI, or `pip install` flows.

## Why it does not build standalone

Upstream `Cargo.toml` uses:

- `version.workspace = true`, `edition.workspace = true`, … (Mycelium workspace)
- Path dependencies: `mycelium-workstack`, and dev-dep `mycelium-l1`

Those crates live only under the Mycelium monorepo. This snapshot keeps the **original** manifest so paths and feature flags stay faithful for review.

## Optional compile (maintainers only)

To compile or test this snapshot you must either:

1. **Open the copy inside the Mycelium workspace** at the provenance commit and treat this tree as a reference diff, or  
2. **Vendor stub** `mycelium-workstack` / `mycelium-l1` and replace `workspace = true` keys with explicit versions in a *local-only* fork of `Cargo.toml` (do not commit stubs unless P23 assessment recommends integration).

Do **not** add this crate to a future py2rust Cargo workspace unless path deps are resolved and CI is updated explicitly.