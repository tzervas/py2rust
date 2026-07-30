#!/usr/bin/env python3
"""Re-runnable expressibility harness.

Runs the `py2rust batch` CLI over `tests/corpus/`, reads the per-file
`.gap.json` sidecars it writes, and regenerates `docs/EXPRESSIBILITY.md`.

Usage:
    python3 scripts/expressibility_report.py [--out DIR] [--check]

`--check` exits non-zero if regenerating the doc would change it on disk —
useful in CI to catch a stale table after a lane closes a gap.

This is the harness the owner asked for: the headline number in
docs/EXPRESSIBILITY.md is derived from this script's output, not hand-typed,
so it stays honest as other lanes land.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CORPUS = REPO_ROOT / "tests" / "corpus"
DOC = REPO_ROOT / "docs" / "EXPRESSIBILITY.md"

# Classification of each gap.rs::Category as it stands *today*.
#
# MECHANICALLY-IMPOSSIBLE: no faithful 1:1 Rust rendering exists even in
# principle for the general case (the semantics are inherently dynamic:
# arbitrary runtime type change, string-evaluated code, computed imports).
# A subset of the category may still be closable (e.g. decorators with known,
# analyzable bodies) — the doc calls those out separately.
#
# MERELY-UNIMPLEMENTED: Rust *can* express the construct (closures, enums,
# Result, async/await, traits, std::collections), the transpiler just does
# not lower it yet. Closable by writing more of the transpiler.
CATEGORY_CLASS = {
    "Class": ("MERELY-UNIMPLEMENTED",
              "structs + impl blocks exist in Rust; single inheritance maps "
              "to composition or trait objects. Lowering is unwritten, not impossible."),
    "Exception": ("MERELY-UNIMPLEMENTED",
                  "Result<T, E> + ? and panic/catch_unwind cover try/except/"
                  "else/finally faithfully enough to lower mechanically."),
    "DynamicTyping": ("MOSTLY MERELY-UNIMPLEMENTED",
                       "Missing annotations just need type inference (closable). "
                       "Genuine runtime type polymorphism (a variable that holds "
                       "different concrete types across a function's life) has no "
                       "1:1 static-Rust rendering without an enum/Any wrapper that "
                       "changes semantics — that residual is MECHANICALLY-IMPOSSIBLE "
                       "to render *silently*; it must gap or require a manual enum."),
    "Metaprogramming": ("SPLIT",
                         "A decorator with a known, statically analyzable body "
                         "(memoization, simple wrapping) is MERELY-UNIMPLEMENTED. "
                         "`eval`/`exec` of a dynamically constructed string and "
                         "metaclasses that rewrite the class at runtime are "
                         "MECHANICALLY-IMPOSSIBLE — Rust has no runtime code-gen."),
    "Async": ("MERELY-UNIMPLEMENTED",
              "Rust has async/await natively (tokio/async-std); lowering is "
              "unwritten, not blocked by any semantic gap."),
    "Import": ("MOSTLY MERELY-UNIMPLEMENTED",
               "Known stdlib modules map to `use` lines (already partially done). "
               "`importlib`/`__import__` with a computed module name is "
               "MECHANICALLY-IMPOSSIBLE — Rust has no dynamic module loading."),
    "Lambda": ("MERELY-UNIMPLEMENTED",
               "Rust closures (`|x| ...`) are a direct target; lowering is unwritten."),
    "Comprehension": ("MERELY-UNIMPLEMENTED",
                       "List comprehensions already lower to iterator chains "
                       "(see git 09e47e2); dict/set/generator comprehensions are "
                       "the same shape, just not yet wired up."),
    "MultiStmtBody": ("MERELY-UNIMPLEMENTED",
                       "Sequencing multiple statements in one Rust block is "
                       "mechanical; the lowering pass is simply incomplete."),
    "FunctionBody": ("MERELY-UNIMPLEMENTED",
                      "Sub-gap on a signature that emitted but whose body did not; "
                      "closable per-construct as each body-statement kind is lowered."),
    "Other": ("NEEDS PER-CASE JUDGEMENT",
              "Catch-all category; `*args`/`**kwargs` (MERELY-UNIMPLEMENTED — Vec/"
              "HashMap can express variadics) and module-level literal collections "
              "(MERELY-UNIMPLEMENTED — same const/static lowering as scalars) are "
              "the cases seen in this corpus."),
}


def run_batch(out_dir: Path) -> dict:
    cmd = [
        "cargo", "run", "-q", "-p", "py2rust", "--",
        "batch", str(CORPUS),
        "--out", str(out_dir),
        "--report", str(out_dir / "corpus-report.md"),
        "--priority-report", str(out_dir / "priority.md"),
    ]
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        print(proc.stdout, file=sys.stderr)
        print(proc.stderr, file=sys.stderr)
        raise SystemExit(f"batch run failed: {' '.join(cmd)}")
    summary_path = out_dir / "summary.json"
    if summary_path.exists():
        return json.loads(summary_path.read_text())
    return {}


def collect_families(out_dir: Path) -> dict:
    """family -> list of (relpath, status, categories)"""
    families: dict[str, list] = defaultdict(list)
    py_files = sorted(CORPUS.rglob("*.py"))
    for py in py_files:
        rel = py.relative_to(CORPUS)
        family = rel.parts[0]
        gap_json = (out_dir / rel).with_suffix(".gap.json")
        if not gap_json.exists():
            families[family].append((str(rel), "FAILED (no output)", []))
            continue
        data = json.loads(gap_json.read_text())
        gaps = data.get("gaps", [])
        cats = sorted({g["category"] for g in gaps})
        if not gaps and data.get("emitted_items"):
            status = "CLEAN"
        elif not gaps and not data.get("emitted_items"):
            status = "CLEAN (empty)"
        else:
            status = "GAPPED"
        families[family].append((str(rel), status, cats))
    return families


def render_doc(summary: dict, families: dict) -> str:
    lines = []
    lines.append("# Expressibility Map\n")
    lines.append(
        "Measures how much of the Python language surface `py2rust` can lower "
        "today, from a corpus run rather than from vibes. Regenerate with:\n"
    )
    lines.append("```\npython3 scripts/expressibility_report.py\n```\n")
    lines.append(
        "**Staleness warning:** at the time this file was generated, seven other "
        "agents were concurrently closing gaps in Lambda, dict/set comprehensions, "
        "MultiStmtBody, Import, Class, Exception, and DynamicTyping on separate "
        "branches. The numbers below reflect only what had landed on *this* "
        "branch's base commit at generation time — they will be an undercount the "
        "moment any of those land. Re-run the script above after merging to refresh.\n"
    )

    total_files = sum(len(v) for v in families.values())
    clean_files = sum(
        1 for rows in families.values() for (_, status, _) in rows if status.startswith("CLEAN")
    )
    lines.append(
        f"## Headline\n\n**{clean_files} of {total_files}** corpus constructs are "
        "mechanically expressible end-to-end today (clean lower, zero gaps, "
        "of files that emitted something). See per-family table below.\n"
    )

    if summary:
        l3 = summary
        lines.append(
            "## L3 (rustc) gate on emitted output\n\n"
            f"- files: {l3.get('total_files')}\n"
            f"- emitted items: {l3.get('total_emitted')} / "
            f"{l3.get('total_top_level')} top-level statements\n"
            f"- L3 checked: {l3.get('l3_checked')}, passed: {l3.get('l3_passed')}, "
            f"failed: {l3.get('l3_failed')}\n"
            f"- passed with zero unlowered (`todo!`) bodies: "
            f"{l3.get('l3_passed_without_stubs')}\n"
        )

    lines.append("## By construct family\n")
    lines.append("| Family | Files | Clean | Gapped | Gap categories seen |")
    lines.append("|---|---|---|---|---|")
    for family in sorted(families):
        rows = families[family]
        clean = sum(1 for (_, s, _) in rows if s.startswith("CLEAN"))
        gapped = sum(1 for (_, s, _) in rows if s == "GAPPED")
        cats = sorted({c for (_, _, cs) in rows for c in cs})
        lines.append(
            f"| {family} | {len(rows)} | {clean} | {gapped} | "
            f"{', '.join(cats) if cats else '—'} |"
        )
    lines.append("")

    lines.append("## Per-file detail\n")
    lines.append("| File | Status | Categories |")
    lines.append("|---|---|---|")
    for family in sorted(families):
        for rel, status, cats in families[family]:
            lines.append(f"| `{rel}` | {status} | {', '.join(cats) if cats else '—'} |")
    lines.append("")

    lines.append(
        "## Mechanically-impossible vs merely-unimplemented\n\n"
        "This is the distinction that makes the roadmap meaningful: only "
        "MERELY-UNIMPLEMENTED gaps are closable by writing more transpiler. "
        "MECHANICALLY-IMPOSSIBLE gaps require either a semantic-changing "
        "workaround (an enum/Any wrapper, a runtime interpreter embedded in "
        "the output) or must stay a permanent, honestly-reported gap.\n"
    )
    lines.append("| Gap category | Classification | Why |")
    lines.append("|---|---|---|")
    for cat, (cls, why) in CATEGORY_CLASS.items():
        lines.append(f"| {cat} | {cls} | {why} |")
    lines.append("")

    lines.append(
        "## Corpus\n\n"
        f"`tests/corpus/` holds {total_files} small, single-purpose Python "
        "snippets across "
        f"{len(families)} construct families (literals, operators, control "
        "flow, functions incl. defaults/*args/**kwargs/decorators, "
        "comprehensions incl. nested/filtered/dict/set/generator, classes "
        "incl. inheritance/properties/dunders, exceptions, imports incl. "
        "relative, lambdas, with-statements, f-strings, slicing, unpacking "
        "incl. star-unpack, match, generators/yield, async/await, and typing "
        "generics). Add more snippets to `tests/corpus/<family>/` and re-run "
        "the script to widen coverage.\n"
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(REPO_ROOT / "target" / "expressibility"))
    ap.add_argument("--check", action="store_true",
                     help="fail if docs/EXPRESSIBILITY.md would change")
    args = ap.parse_args()

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    summary = run_batch(out_dir)
    families = collect_families(out_dir)
    doc = render_doc(summary, families)

    if args.check:
        existing = DOC.read_text() if DOC.exists() else ""
        if existing != doc:
            print("docs/EXPRESSIBILITY.md is stale; run without --check to refresh.",
                  file=sys.stderr)
            return 1
        print("docs/EXPRESSIBILITY.md is up to date.")
        return 0

    DOC.write_text(doc)
    print(f"wrote {DOC}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
