#!/usr/bin/env bash
# Local coverage measurement for Voltex. Reports line coverage; there is
# no threshold unless VOLTEX_COV_FAIL_UNDER_LINES is set.
#
# Usage:
#   scripts/coverage.sh            # summary, GPU-gated tests included
#   scripts/coverage.sh --html     # also generate an HTML report
#   scripts/coverage.sh --lib-only # skip GPU-gated #[ignore]'d tests
#   VOLTEX_COV_FAIL_UNDER_LINES=80 scripts/coverage.sh   # fail below 80%
#
# Requires:
#   cargo install cargo-llvm-cov
#   rustup component add llvm-tools-preview
#
# Without a GPU (or lavapipe) the #[ignore]'d tests cannot run, so their
# bodies and the renderer code they exercise count as uncovered and the
# number drops sharply. See docs/dev/AUDIT-BACKLOG.md F5.

set -euo pipefail

FAIL_UNDER_LINES="${VOLTEX_COV_FAIL_UNDER_LINES:-}"
INCLUDE_IGNORED=1
HTML_REPORT=0

for arg in "$@"; do
  case "$arg" in
    --lib-only) INCLUDE_IGNORED=0 ;;
    --html)     HTML_REPORT=1 ;;
    *)
      echo "Unknown flag: $arg" >&2
      echo "Usage: scripts/coverage.sh [--lib-only] [--html]" >&2
      exit 2
      ;;
  esac
done

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "cargo-llvm-cov not installed. Run: cargo install cargo-llvm-cov" >&2
  exit 1
fi

ARGS=(--workspace --lib --summary-only)
if [ -n "$FAIL_UNDER_LINES" ]; then
  ARGS+=(--fail-under-lines "$FAIL_UNDER_LINES")
fi

if [ "$HTML_REPORT" = "1" ]; then
  ARGS+=(--html)
fi

if [ "$INCLUDE_IGNORED" = "1" ]; then
  echo "Running with GPU-gated #[ignore]'d tests (--include-ignored)"
  cargo llvm-cov "${ARGS[@]}" -- --include-ignored
else
  echo "Running with --lib only (no GPU paths)"
  cargo llvm-cov "${ARGS[@]}"
fi
