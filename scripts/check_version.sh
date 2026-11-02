#!/usr/bin/env bash
# check_version.sh — fail if the newest numbered CHANGELOG entry and the
# Cargo [workspace.package] version disagree.
#
# Usage:
#   scripts/check_version.sh <CHANGELOG.md> <Cargo.toml>
#
# The newest numbered entry is the first "## vX.Y.Z" header. "## Unreleased"
# entries are skipped: their number is assigned at merge time, together with
# the workspace version. With no numbered entry there is nothing to compare.
#
# Exit codes:
#   0 = versions match (or no numbered entry)
#   1 = versions differ, or the workspace version is missing
#   2 = usage / unreadable file
#
# Why exists: the workspace version stayed at 1.0.114 while CHANGELOG reached
# v1.0.172, so `voltex --version` and crash reports showed the old number.

set -euo pipefail

if [[ $# -ne 2 || ! -r "$1" || ! -r "$2" ]]; then
    echo "usage: $0 <CHANGELOG.md> <Cargo.toml>" >&2
    exit 2
fi

changelog_version="$(tr -d '\r' < "$1" \
    | sed -n -E 's/^## v([0-9]+\.[0-9]+\.[0-9]+)([^0-9.].*)?$/\1/p' | head -n 1)"

workspace_version="$(tr -d '\r' < "$2" | awk '
    /^\[/ { in_package = ($0 == "[workspace.package]"); next }
    in_package && /^version[[:space:]]*=/ {
        if (match($0, /"[^"]*"/)) { print substr($0, RSTART + 1, RLENGTH - 2); exit }
    }')"

if [[ -z "$workspace_version" ]]; then
    echo "✗ no version in [workspace.package] of $2" >&2
    exit 1
fi

if [[ -z "$changelog_version" ]]; then
    echo "✓ no numbered CHANGELOG entry yet — workspace $workspace_version not compared"
    exit 0
fi

if [[ "$changelog_version" != "$workspace_version" ]]; then
    echo "✗ CHANGELOG v$changelog_version != workspace $workspace_version" >&2
    echo "  set [workspace.package] version in Cargo.toml to $changelog_version (or fix the CHANGELOG header)." >&2
    exit 1
fi

echo "✓ CHANGELOG v$changelog_version == workspace $workspace_version"
exit 0
