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
# Coverage today (measured on Windows + RTX 4080 SUPER, 2026-05-28):
#   --lib only           : 79.69% lines
#   --lib + GPU ignored  : 84.24% lines
#
# CI gates at 78% so a missing GPU on the runner (lavapipe absent) still
# passes — graceful skip in voltex_capture's HeadlessGpu::new().

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
