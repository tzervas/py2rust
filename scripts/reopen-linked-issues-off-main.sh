#!/bin/bash
# scripts/reopen-linked-issues-off-main.sh
#
# Counterweight to close-linked-issues.sh:
#   GitHub natively closes Issues on *any* base when a PR uses Fixes/Closes #N.
#   Feature work lands on **dev**, so those closes must be undone until the
#   work actually reaches **main** (where close-linked-issues.sh owns the close).
#
# Usage:
#   bash scripts/reopen-linked-issues-off-main.sh --self-test
#   bash scripts/reopen-linked-issues-off-main.sh --pr 37
#   bash scripts/reopen-linked-issues-off-main.sh --pr 37 --dry-run
#   bash scripts/reopen-linked-issues-off-main.sh --pr 38   # base=main → report, no reopen
#
# Exit codes:
#   0  acted or deliberately skipped with a proven classification
#   1  self-test / API / extractor failure (real gate red)
#   2  bad args
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

DRY_RUN=0
SELF_TEST=0
PR_NUM=""
MAIN_BRANCH="${RELAY_MAIN_BRANCH:-main}"

usage() {
    sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --pr) PR_NUM="${2:-}"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        --self-test) SELF_TEST=1; shift ;;
        --main-branch) MAIN_BRANCH="${2:-main}"; shift 2 ;;
        -h | --help) usage; exit 0 ;;
        *)
            printf 'reopen-linked-issues-off-main.sh: unknown arg: %s\n' "$1" >&2
            exit 2
            ;;
    esac
done

if ! command -v gh >/dev/null 2>&1; then
    printf 'reopen-linked-issues-off-main.sh: gh CLI required\n' >&2
    exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
    printf 'reopen-linked-issues-off-main.sh: python3 required\n' >&2
    exit 1
fi

# Reads text on stdin → one issue number per line (closing keywords only).
# Intentionally narrower than close-linked-issues.sh: we only undo native
# Fixes/Closes/Resolves auto-close, not conventional feat(#N) title refs.
extract_closing_issue_numbers() {
    local _prog
    _prog="$(cat <<'PY'
import re, sys
text = sys.stdin.read()
kw = r"(?:fix(?:e[sd])?|close[sd]?|resolve[sd]?)"
found = set()
for m in re.finditer(
    rf"(?is)\b{kw}\b(?:\s*:)?\s*((?:#?\d+(?:\s*[,&]?\s*(?:and\s+)?#?\d+)*))",
    text,
):
    prefix = text[max(0, m.start() - 24) : m.start()].lower()
    if re.search(r"(?:no|not|without|dont)\W*$", prefix):
        continue
    for n in re.findall(r"\d+", m.group(1)):
        found.add(int(n))
for n in sorted(found):
    print(n)
PY
)"
    python3 -c "$_prog"
}

self_test() {
    local fail=0 got want

    # 1) extractor fixtures — must produce exact sets
    want=$'3\n12\n44'
    got="$(printf 'Fixes #12\nAlso Closes #3, #44\nRefs #99 only\n' | extract_closing_issue_numbers)"
    if [[ "$got" != "$want" ]]; then
        printf 'SELF-TEST FAIL extract multi: got=%q want=%q\n' "$got" "$want" >&2
        fail=1
    fi

    want=$'7'
    got="$(printf 'This resolves #7 permanently.\n' | extract_closing_issue_numbers)"
    if [[ "$got" != "$want" ]]; then
        printf 'SELF-TEST FAIL extract resolve: got=%q want=%q\n' "$got" "$want" >&2
        fail=1
    fi

    want=""
    got="$(printf 'No Fixes #22\nnot close #5\nwithout Resolves #9\nRefs #100\n' | extract_closing_issue_numbers || true)"
    if [[ -n "$got" ]]; then
        printf 'SELF-TEST FAIL extract negations/refs: got unexpected %q\n' "$got" >&2
        fail=1
    fi

    # 2) gh auth + repo visibility — a "green" run with a dead token is a lie
    if ! gh api user --jq .login >/dev/null; then
        printf 'SELF-TEST FAIL: gh api user (token dead or missing scopes)\n' >&2
        fail=1
    fi
    if ! gh api "repos/${GITHUB_REPOSITORY:-tzervas/py2rust}" --jq .full_name >/dev/null; then
        printf 'SELF-TEST FAIL: cannot read target repo via gh api\n' >&2
        fail=1
    fi

    if (( fail != 0 )); then
        printf 'self-test FAILED\n' >&2
        exit 1
    fi
    printf 'self-test OK (extractor fixtures + gh api)\n'
}

reopen_one() {
    local issue="$1" pr="$2" base="$3" state
    state="$(gh api "repos/${GITHUB_REPOSITORY}/issues/${issue}" --jq .state 2>/dev/null || echo missing)"
    if [[ "$state" == "missing" ]]; then
        printf '  skip  #%s (not found)\n' "$issue"
        return 0
    fi
    if [[ "$state" == "open" ]]; then
        printf '  skip  #%s (already open)\n' "$issue"
        return 0
    fi
    if (( DRY_RUN == 1 )); then
        printf '  dry   would reopen #%s (PR #%s → %s)\n' "$issue" "$pr" "$base"
        return 0
    fi
    gh api -X PATCH "repos/${GITHUB_REPOSITORY}/issues/${issue}" -f state=open >/dev/null
    gh api -X POST "repos/${GITHUB_REPOSITORY}/issues/${issue}/comments" \
        -f body="Reopened automatically: closing keywords on PR #${pr} merged into \`${base}\` (not main). Issues close only when work reaches **main**. Prefer \`Refs #${issue}\` on non-main PRs." \
        >/dev/null
    printf '  reopened #%s (PR #%s → %s)\n' "$issue" "$pr" "$base"
}

process_pr() {
    local pr="$1" json title body base state merged_at text nums is_merged

    : "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set (owner/repo)}"

    json="$(gh api "repos/${GITHUB_REPOSITORY}/pulls/${pr}")" || {
        printf 'cannot fetch PR #%s\n' "$pr" >&2
        return 1
    }
    title="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("title") or "")')"
    body="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("body") or "")')"
    base="$(printf '%s' "$json" | python3 -c 'import json,sys; print((json.load(sys.stdin).get("base") or {}).get("ref") or "")')"
    state="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("state") or "")')"
    merged_at="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("merged_at") or "")')"

    text="${title}"$'\n'"${body}"
    nums="$(printf '%s' "$text" | extract_closing_issue_numbers || true)"

    is_merged=0
    if [[ -n "$merged_at" || "$state" == "closed" && -n "$merged_at" ]]; then
        is_merged=1
    fi
    # gh returns state=closed for merged PRs; merged_at is the ground truth
    if [[ -n "$merged_at" ]]; then
        is_merged=1
    fi

    printf 'PR #%s state=%s base=%s merged_at=%s\n' "$pr" "$state" "$base" "${merged_at:-none}"
    if [[ -z "$nums" ]]; then
        printf '  no Fixes/Closes/Resolves keywords — nothing to reopen\n'
        return 0
    fi
    printf '  closing keywords reference: %s\n' "$(printf '%s' "$nums" | tr '\n' ' ')"

    if (( is_merged == 0 )); then
        printf '  not merged — no reopen (would consider after merge to non-main)\n'
        return 0
    fi

    if [[ "$base" == "$MAIN_BRANCH" || "$base" == "master" ]]; then
        printf '  base is trunk (%s) — leave closed (main owns issue close)\n' "$base"
        while IFS= read -r n; do
            [[ -z "$n" ]] && continue
            printf '  leave closed #%s\n' "$n"
        done <<<"$nums"
        return 0
    fi

    printf '  off-main merge into %s — reopening closed linked issues\n' "$base"
    while IFS= read -r n; do
        [[ -z "$n" ]] && continue
        reopen_one "$n" "$pr" "$base"
    done <<<"$nums"
}

if (( SELF_TEST == 1 )); then
    self_test
    if [[ -z "$PR_NUM" ]]; then
        exit 0
    fi
fi

if [[ -z "$PR_NUM" ]]; then
    printf 'reopen-linked-issues-off-main.sh: --pr N required (or --self-test alone)\n' >&2
    exit 2
fi

# Always run self-test before acting so a green job means "logic works", not "echo worked"
self_test
process_pr "$PR_NUM"
