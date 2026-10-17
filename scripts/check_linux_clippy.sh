#!/usr/bin/env bash
# check_linux_clippy.sh — clippy the workspace for the CI runner's target
# (aarch64-unknown-linux-gnu) with the CI's RUSTFLAGS, from any host.
#
# Usage:
#   scripts/check_linux_clippy.sh
#
# Exit codes:
#   0 = clean, or the target is not installed (warning only, skipped)
#   1 = clippy / rustc reported an error for the Linux target
#
# Why exists: Windows-only cfg paths left Linux-only dead code in
# voltex_audio, so the Gitea CI (Linux) failed from its first run while the
# local Windows checks passed. Enable with:
#   rustup target add aarch64-unknown-linux-gnu
#
# Cross-check limits: build scripts run for the target, and a Windows host has
# no aarch64-linux-gnu C compiler. The crates below are excluded for that
# reason only; the Linux CI builds them natively. Excluded: 13 of 37 crates.

set -euo pipefail

TARGET="aarch64-unknown-linux-gnu"

EXCLUDES=(
    voltex_script     # build.rs compiles the bundled Lua C sources (cc)
    voltex_runtime    # depends on voltex_script
    voltex            # depends on voltex_runtime
    boids_demo        # depends on voltex_script
    game_ready_demo   # depends on voltex_script
    hierarchy_demo    # depends on voltex_script
    runtime_demo      # depends on voltex_script
    survivor_game     # depends on voltex_script
    triangle          # depends on voltex_script
    tutorial_minigame # depends on voltex_script
    voltex_depths     # depends on voltex_script
    voltex_leap       # depends on voltex_script
    voltex_survivor   # depends on voltex_script
)

YELLOW='\033[0;33m'
NC='\033[0m'

if ! command -v rustup >/dev/null 2>&1 || ! rustup target list --installed | grep -qx "$TARGET"; then
    echo -e "${YELLOW}! Linux clippy skipped: target $TARGET not installed${NC}"
    echo "  enable: rustup target add $TARGET"
    exit 0
fi

args=()
for crate in "${EXCLUDES[@]}"; do
    args+=(--exclude "$crate")
done

echo "Linux clippy ($TARGET, ${#EXCLUDES[@]} crates excluded: cross C build)"
if ! RUSTFLAGS="-D warnings" cargo clippy --target "$TARGET" --workspace --all-targets "${args[@]}" -- -D warnings; then
    exit 1
fi
