#!/usr/bin/env bash
# Install the voltex git hooks by pointing core.hooksPath at .githooks/.
# Idempotent — safe to re-run.

set -e

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$REPO_ROOT" ]; then
    echo "✗ not inside a git repository" >&2
    exit 1
fi

cd "$REPO_ROOT"

git config core.hooksPath .githooks
chmod +x .githooks/pre-commit

echo "✓ git hooks installed."
echo "  core.hooksPath = $(git config --get core.hooksPath)"
echo
echo "  Disable temporarily: git commit --no-verify"
echo "  Disable permanently: git config --unset core.hooksPath"
