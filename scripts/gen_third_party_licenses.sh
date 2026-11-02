#!/usr/bin/env bash
# gen_third_party_licenses.sh — regenerate THIRD-PARTY-LICENSES.md at the
# repo root. `voltex build` copies that file into every game's dist/, so
# rerun this whenever dependencies change.
#
# Requires: cargo install --locked cargo-about --features cli
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo about generate \
    -c scripts/licenses/about.toml \
    scripts/licenses/about.hbs \
    -o THIRD-PARTY-LICENSES.md
