#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
MODE="${1:-}"
if command -v uv >/dev/null 2>&1; then
  uv sync
  if [[ "$MODE" == "--fix" ]]; then
    uv run ruff format src tests
    uv run ruff check --fix src tests || true
  else
    uv run ruff format --check src tests
    uv run ruff check src tests
  fi
  # mypy is advisory until types are fully clean
  if [[ "$MODE" != "--quick" ]]; then
    uv run mypy src || echo "WARN: mypy reported issues (non-fatal for --strict later)"
  fi
  uv run pytest -q
else
  python3 -m pytest -q
fi
if [[ -f ../tero-mcp/scripts/generate_lite_index.py ]]; then
  python3 ../tero-mcp/scripts/generate_lite_index.py --root .
fi
echo "OK: py2rust checks passed"
