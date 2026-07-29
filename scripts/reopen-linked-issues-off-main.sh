#!/bin/bash
# scripts/reopen-linked-issues-off-main.sh
#
# Counterweight to close-linked-issues.sh:
#   GitHub natively closes Issues on *any* base when a PR uses Fixes/Closes #N.
#   Feature work lands on **dev**, so those closes must be undone until the
#   work actually reaches **main** (where close-linked-issues.sh owns the close).
#
# Outcomes (exit code):
#   0  GREEN — nothing to reopen, OR every needed reopen succeeded
#              (already-open / base=main leave-closed / no keywords are green)
#   1  RED   — self-test failed, API/auth failure, issue not found when we
#              needed to act, or a reopen PATCH/comment failed / didn't stick
#   2  bad args
#
# Usage:
#   bash scripts/reopen-linked-issues-off-main.sh --self-test
#   bash scripts/reopen-linked-issues-off-main.sh --pr 37
#   bash scripts/reopen-linked-issues-off-main.sh --pr 37 --dry-run
#   bash scripts/reopen-linked-issues-off-main.sh --pr 38   # base=main → leave closed
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

DRY_RUN=0
SELF_TEST=0
PR_NUM=""
MAIN_BRANCH="${RELAY_MAIN_BRANCH:-main}"

COUNT_REOPENED=0
COUNT_WOULD_REOPEN=0
COUNT_ALREADY_OPEN=0
COUNT_LEAVE_CLOSED=0
COUNT_MISSING=0

usage() {
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
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

# Pure decision helper (no network). Prints one of:
#   not_merged | leave_closed | skip_open | reopen | missing
classify_action() {
    local base="$1" merged="$2" issue_state="$3"
    if [[ "$merged" != "true" ]]; then
        printf 'not_merged\n'
        return 0
    fi
    if [[ "$base" == "$MAIN_BRANCH" || "$base" == "master" ]]; then
        printf 'leave_closed\n'
        return 0
    fi
    case "$issue_state" in
        open) printf 'skip_open\n' ;;
        closed) printf 'reopen\n' ;;
        *) printf 'missing\n' ;;
    esac
}

self_test() {
    local fail=0 got want repo

    # --- 1) extractor fixtures ---
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

    got="$(printf '\n' | extract_closing_issue_numbers || true)"
    if [[ -n "$got" ]]; then
        printf 'SELF-TEST FAIL extract empty: got unexpected %q\n' "$got" >&2
        fail=1
    fi

    # --- 2) decision matrix (no network) ---
    got="$(classify_action dev true closed)"
    [[ "$got" == "reopen" ]] || { printf 'SELF-TEST FAIL classify dev/closed: %q\n' "$got" >&2; fail=1; }
    got="$(classify_action dev true open)"
    [[ "$got" == "skip_open" ]] || { printf 'SELF-TEST FAIL classify dev/open: %q\n' "$got" >&2; fail=1; }
    got="$(classify_action main true closed)"
    [[ "$got" == "leave_closed" ]] || { printf 'SELF-TEST FAIL classify main/closed: %q\n' "$got" >&2; fail=1; }
    got="$(classify_action dev false closed)"
    [[ "$got" == "not_merged" ]] || { printf 'SELF-TEST FAIL classify not_merged: %q\n' "$got" >&2; fail=1; }
    got="$(classify_action dev true missing)"
    [[ "$got" == "missing" ]] || { printf 'SELF-TEST FAIL classify missing: %q\n' "$got" >&2; fail=1; }

    # --- 3) gh auth + repo visibility ---
    # Do NOT call `gh api user` here. Actions GITHUB_TOKEN is an installation
    # token: `GET /user` returns HTTP 403 "Resource not accessible by
    # integration" even when issues:write works. Probe the resources this
    # workflow actually needs (repo meta + issue read/write path).
    repo="${GITHUB_REPOSITORY:-tzervas/py2rust}"
    if ! gh api "repos/${repo}" --jq .full_name >/dev/null; then
        printf 'SELF-TEST FAIL: cannot read target repo via gh api (token dead or missing scopes)\n' >&2
        fail=1
    fi

    # --- 4) prove we can GET issue state (reopen path depends on this) ---
    local probe_state
    probe_state="$(gh api "repos/${repo}/issues/13" --jq .state 2>/dev/null || echo missing)"
    if [[ "$probe_state" == "missing" ]]; then
        printf 'SELF-TEST FAIL: cannot GET repos/%s/issues/13 (API/read broken)\n' "$repo" >&2
        fail=1
    else
        printf 'self-test issue-read probe: #13 state=%s\n' "$probe_state"
    fi

    # --- 5) outside Actions only: also verify a user-scoped token works ---
    # Personal/local runs should still catch a dead user token early.
    if [[ -z "${GITHUB_ACTIONS:-}" ]]; then
        if ! gh api user --jq .login >/dev/null; then
            printf 'SELF-TEST FAIL: gh api user (local token dead or missing scopes)\n' >&2
            fail=1
        fi
    fi

    if (( fail != 0 )); then
        printf 'self-test FAILED\n' >&2
        exit 1
    fi
    printf 'self-test OK (extractor + classify matrix + gh api + issue read)\n'
}

issue_state() {
    local issue="$1" out err
    if out="$(gh api "repos/${GITHUB_REPOSITORY}/issues/${issue}" --jq .state 2>/dev/null)"; then
        printf '%s\n' "$out"
        return 0
    fi
    err="$(gh api "repos/${GITHUB_REPOSITORY}/issues/${issue}" 2>&1 || true)"
    if printf '%s' "$err" | grep -qiE 'HTTP 404|Not Found|"message": "Not Found"'; then
        printf 'missing\n'
        return 0
    fi
    printf 'API error reading issue #%s: %s\n' "$issue" "$err" >&2
    return 1
}

reopen_one() {
    local issue="$1" pr="$2" base="$3" state after

    state="$(issue_state "$issue")" || return 1

    case "$state" in
        missing)
            printf '  FAIL  #%s not found (closing keyword points at nothing)\n' "$issue" >&2
            COUNT_MISSING=$((COUNT_MISSING + 1))
            return 1
            ;;
        open)
            printf '  skip  #%s (already open)\n' "$issue"
            COUNT_ALREADY_OPEN=$((COUNT_ALREADY_OPEN + 1))
            return 0
            ;;
        closed)
            ;;
        *)
            printf '  FAIL  #%s unexpected state=%s\n' "$issue" "$state" >&2
            return 1
            ;;
    esac

    if (( DRY_RUN == 1 )); then
        printf '  dry   would reopen #%s (PR #%s → %s)\n' "$issue" "$pr" "$base"
        COUNT_WOULD_REOPEN=$((COUNT_WOULD_REOPEN + 1))
        return 0
    fi

    if ! gh api -X PATCH "repos/${GITHUB_REPOSITORY}/issues/${issue}" -f state=open >/dev/null; then
        printf '  FAIL  #%s PATCH state=open failed\n' "$issue" >&2
        return 1
    fi
    if ! gh api -X POST "repos/${GITHUB_REPOSITORY}/issues/${issue}/comments" \
        -f body="Reopened automatically: closing keywords on PR #${pr} merged into \`${base}\` (not main). Issues close only when work reaches **main**. Prefer \`Refs #${issue}\` on non-main PRs." \
        >/dev/null; then
        printf '  FAIL  #%s comment after reopen failed (issue may be open; check manually)\n' "$issue" >&2
        return 1
    fi

    after="$(issue_state "$issue")" || return 1
    if [[ "$after" != "open" ]]; then
        printf '  FAIL  #%s reopen did not stick (state=%s after PATCH)\n' "$issue" "$after" >&2
        return 1
    fi

    printf '  reopened #%s (PR #%s → %s) verified open\n' "$issue" "$pr" "$base"
    COUNT_REOPENED=$((COUNT_REOPENED + 1))
    return 0
}

print_summary() {
    local failures="${1:-0}"
    printf 'summary: reopened=%d would_reopen=%d already_open=%d leave_closed=%d missing=%d failures=%d\n' \
        "$COUNT_REOPENED" "$COUNT_WOULD_REOPEN" "$COUNT_ALREADY_OPEN" "$COUNT_LEAVE_CLOSED" "$COUNT_MISSING" "$failures"
}

process_pr() {
    local pr="$1" json title body base state merged_at text nums is_merged
    local failures=0

    : "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set (owner/repo)}"

    if ! json="$(gh api "repos/${GITHUB_REPOSITORY}/pulls/${pr}")"; then
        printf 'FAIL: cannot fetch PR #%s\n' "$pr" >&2
        return 1
    fi
    title="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("title") or "")')"
    body="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("body") or "")')"
    base="$(printf '%s' "$json" | python3 -c 'import json,sys; print((json.load(sys.stdin).get("base") or {}).get("ref") or "")')"
    state="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("state") or "")')"
    merged_at="$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("merged_at") or "")')"

    text="${title}"$'\n'"${body}"
    nums="$(printf '%s' "$text" | extract_closing_issue_numbers || true)"

    is_merged=0
    if [[ -n "$merged_at" ]]; then
        is_merged=1
    fi

    printf 'PR #%s state=%s base=%s merged_at=%s\n' "$pr" "$state" "$base" "${merged_at:-none}"

    # GREEN: no tickets referenced
    if [[ -z "$nums" ]]; then
        printf '  no Fixes/Closes/Resolves keywords — nothing to reopen (green)\n'
        print_summary 0
        return 0
    fi
    printf '  closing keywords reference: %s\n' "$(printf '%s' "$nums" | tr '\n' ' ')"

    # GREEN: not merged yet
    if (( is_merged == 0 )); then
        printf '  not merged — no reopen (green; would consider after off-main merge)\n'
        print_summary 0
        return 0
    fi

    # GREEN: main owns close — leave closed, but still GET each issue (API must work)
    if [[ "$base" == "$MAIN_BRANCH" || "$base" == "master" ]]; then
        printf '  base is trunk (%s) — leave closed (main owns issue close)\n' "$base"
        while IFS= read -r n; do
            [[ -z "$n" ]] && continue
            if ! issue_state "$n" >/dev/null; then
                printf '  FAIL reading #%s while classifying leave_closed\n' "$n" >&2
                failures=$((failures + 1))
                continue
            fi
            printf '  leave closed #%s\n' "$n"
            COUNT_LEAVE_CLOSED=$((COUNT_LEAVE_CLOSED + 1))
        done <<<"$nums"
        print_summary "$failures"
        if (( failures != 0 )); then
            printf 'FAIL: %d issue read error(s) on leave_closed path\n' "$failures" >&2
            return 1
        fi
        return 0
    fi

    # OFF-MAIN: reopen closed tickets; already-open is green skip; missing/API = red
    printf '  off-main merge into %s — reopening closed linked issues\n' "$base"
    while IFS= read -r n; do
        [[ -z "$n" ]] && continue
        if ! reopen_one "$n" "$pr" "$base"; then
            failures=$((failures + 1))
        fi
    done <<<"$nums"

    print_summary "$failures"
    if (( failures != 0 )); then
        printf 'FAIL: %d issue(s) could not be reopened/verified\n' "$failures" >&2
        return 1
    fi
    return 0
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

self_test
process_pr "$PR_NUM"
