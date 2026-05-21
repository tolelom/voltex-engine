# Voltex Release Process

This document covers how to cut a new Voltex release and what the release
pipeline (`.github/workflows/release.yml`) produces.

## TL;DR

```bash
# 1. Make sure master is green and the new CHANGELOG entry is committed.
git checkout master && git pull --ff-only

# 2. Sanity-check the workspace.
cargo build --workspace
cargo test --workspace --lib
cargo clippy --workspace --all-targets -- -D warnings

# 3. Tag the release. The tag triggers the release workflow.
git tag v1.0.x
git push origin v1.0.x
```

GitHub Actions then runs the `release` workflow, which builds `voltex` (the
CLI scaffolder) for three triples and uploads the archives as a Release.

## What the workflow produces

For each tagged push, the [release workflow](../.github/workflows/release.yml)
runs three matrix jobs (Linux x64, Windows x64, macOS aarch64) plus a
publish job that aggregates the artifacts onto a GitHub Release.

Each archive contains:

```
voltex-<version>-<triple>/
├── voltex(.exe)         # the CLI binary
├── LICENSE-MIT
├── LICENSE-APACHE
└── README.md
```

Triples and archive formats:

| OS              | Triple                          | Archive                                       |
|-----------------|---------------------------------|-----------------------------------------------|
| Linux           | `x86_64-unknown-linux-gnu`      | `voltex-<ver>-x86_64-unknown-linux-gnu.tar.gz`|
| Windows         | `x86_64-pc-windows-msvc`        | `voltex-<ver>-x86_64-pc-windows-msvc.zip`     |
| macOS (Apple)   | `aarch64-apple-darwin`          | `voltex-<ver>-aarch64-apple-darwin.tar.gz`    |

## Versioning

Voltex follows SemVer. The CI `semver-check` job (added v1.0.33) compares
each push against the `v1.0.0` tag. Promote to a blocking gate once
tag-per-cycle is universal.

- **Patch** (`v1.0.X`): bug fixes, missing-docs sweeps, additive features
  that don't expand the API surface.
- **Minor** (`v1.X.0`): additive API. New crates, new public types, new
  builder methods.
- **Major** (`vX.0.0`): breaking API changes. Avoid until v2.

## Pre-release checklist

Before tagging:

- [ ] `CHANGELOG.md` has an entry for the new version (the `pre-push` hook
      already enforces this for code changes, but double-check).
- [ ] `docs/STATUS.md` reflects the latest cycle.
- [ ] `cargo build --workspace` clean.
- [ ] `cargo test --workspace --lib` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo test -p voltex_cli --test e2e_new_build -- --ignored` passes
      (heavy ~60s; the nightly cron also runs this, but a manual run before
      a release tag is cheap insurance).

## Manually triggering a release build

For testing the workflow without cutting a real tag, use the
`workflow_dispatch` trigger from the Actions tab. Pass an optional tag
name in the `tag` input; the workflow will build for it but will NOT
publish a Release (the publish step is gated on `refs/tags/v*`).

## Rolling back

GitHub Releases can be deleted via the Releases UI or `gh release delete
v1.0.x`. To re-tag with the same name:

```bash
git tag -d v1.0.x
git push origin :refs/tags/v1.0.x
git tag v1.0.x <new_sha>
git push origin v1.0.x
```

This is destructive — only do it when no one has downloaded the original
artifact (or when the original was broken).
