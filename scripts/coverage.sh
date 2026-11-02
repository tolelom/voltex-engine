#!/usr/bin/env bash
# Local coverage measurement for Voltex — mirrors the CI coverage job
# (.github/workflows/ci.yml) so contributors can reproduce the gate
# before pushing.
#
# Usage:
#   scripts/coverage.sh            # summary, gate at 78% lines (CI default)
#   scripts/coverage.sh --html     # also generate an HTML report
#   scripts/coverage.sh --lib-only # skip GPU-gated #[ignore]'d tests
#
# Requires:
#   cargo install cargo-llvm-cov
#   rustup component add llvm-tools-preview
#
# Coverage (measured on Windows + RTX 4080 SUPER, 2026-10-12):
#   --lib only           : 68.83% lines (below the 78% gate)
#   --lib + GPU ignored  : 82.90% lines
#
# The GPU-gated #[ignore] tests must run to pass the gate: without a GPU
# (lavapipe absent) their bodies and the renderer code they exercise count
# as uncovered. See docs/dev/NEXT.md "커버리지 게이트".

set -euo pipefail

FAIL_UNDER_LINES="${VOLTEX_COV_FAIL_UNDER_LINES:-78}"
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

ARGS=(--workspace --lib --summary-only --fail-under-lines "$FAIL_UNDER_LINES")

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
