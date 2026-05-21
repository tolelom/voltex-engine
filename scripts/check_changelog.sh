#!/usr/bin/env bash
# check_changelog.sh — fail if a commit range touches code without a CHANGELOG.md edit.
#
# Usage:
#   scripts/check_changelog.sh <base_ref> <head_ref>
#   scripts/check_changelog.sh HEAD~1 HEAD            # last commit only
#   scripts/check_changelog.sh origin/master HEAD     # everything to be pushed
#
# Exit codes:
#   0 = ok (CHANGELOG modified, or only doc/build files touched)
#   1 = CHANGELOG entry missing for code changes
#   2 = script usage / bad ref
#
# Why exists: solo-dev workflow auto-pushes to master, so we can't rely on
# manual PR review to catch missing release notes. Pre-push hook + this
# script give the same coverage without needing a code-review gate.

set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <base_ref> <head_ref>" >&2
    exit 2
fi

BASE="$1"
HEAD="$2"

# Verify both refs exist.
if ! git rev-parse --verify "$BASE" >/dev/null 2>&1; then
    echo "✗ unknown base ref: $BASE" >&2
    exit 2
fi
if ! git rev-parse --verify "$HEAD" >/dev/null 2>&1; then
    echo "✗ unknown head ref: $HEAD" >&2
    exit 2
fi

# Collect the list of files changed between BASE and HEAD.
CHANGED="$(git diff --name-only "$BASE" "$HEAD")"

CODE_TOUCHED=false
CHANGELOG_TOUCHED=false
TOUCHED_PATHS=()

while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    TOUCHED_PATHS+=("$file")
    case "$file" in
        # Code paths that should produce a CHANGELOG entry.
        crates/*|examples/*|games/*)
            # …unless it's a docs-only file inside a code dir.
            case "$file" in
                *.md|*/CODEMAP.md) ;;
                *) CODE_TOUCHED=true ;;
            esac
            ;;
        # The CHANGELOG itself.
        CHANGELOG.md)
            CHANGELOG_TOUCHED=true
            ;;
    esac
done <<< "$CHANGED"

if $CODE_TOUCHED && ! $CHANGELOG_TOUCHED; then
    echo "✗ code changes detected between $BASE..$HEAD but CHANGELOG.md not modified" >&2
    echo "" >&2
    echo "  touched files:" >&2
    for p in "${TOUCHED_PATHS[@]}"; do echo "    $p" >&2; done
    echo "" >&2
    echo "  add a CHANGELOG entry under a new [vX.Y.Z] header, then commit again." >&2
    echo "  to bypass: git push --no-verify  (rare — see CONTRIBUTING.md)" >&2
    exit 1
fi

if $CODE_TOUCHED; then
    echo "✓ CHANGELOG entry present for code changes ($BASE..$HEAD)"
else
    echo "✓ docs-only or non-code change — CHANGELOG entry not required ($BASE..$HEAD)"
fi
exit 0
