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

Voltex follows SemVer.

- **Patch** (`v1.0.X`): bug fixes, missing-docs sweeps, additive features
  that don't expand the API surface.
- **Minor** (`v1.X.0`): additive API. New crates, new public types, new
  builder methods.
- **Major** (`vX.0.0`): breaking API changes. Avoid until v2.

## Pre-release checklist

Before tagging:

- [ ] `CHANGELOG.md` has an entry for the new version (the `pre-push` hook
      already enforces this for code changes, but double-check).
- [ ] `cargo build --workspace` clean.
- [ ] `cargo test --workspace --lib` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `scripts/gen_third_party_licenses.sh` leaves `THIRD-PARTY-LICENSES.md`
      unchanged (otherwise commit the regenerated file).
- [ ] `cargo test -p voltex_cli --test e2e_new_build -- --ignored` passes
      (heavy; also available as the manual `e2e-quickstart` CI job).

### GPU-host visual verification (run on a machine with a GPU + display)

CI is headless, so it can verify *no panic* but never *correct pixels* or the
RT/windowed paths. Before a release, run these on a real GPU host (see
[`docs/visual/README.md`](visual/README.md)):

- [ ] `cargo run -p voltex_capture --bin capture` — renders every pipeline
      (BasicRenderer / PBR / shadows / multi-light / gizmos / editor UI) to
      `target/captures/*.png`. Exits non-zero if any frame is blank. **Eyeball
      the PNGs** — blank-detection can't catch subtly-wrong output.
- [ ] `cargo test -p voltex_capture -- --ignored` — GPU-gated scene tests pass.
- [ ] Smoke-run the windowed demos for ~6s each with no panic / wgpu validation
      error (only benign Vulkan loader warnings are expected):
      `RUST_LOG=warn <timeout 6> cargo run -p deferred_demo` (also `ibl_demo`,
      `shadow_demo`, `runtime_demo`, `survivor`). `deferred_demo` is the
      one that exercises ray-tracing feature acquisition + BLAS.
- [ ] Full CLI build→run: `voltex new <dir> && cd <dir> &&
      voltex build --engine-path <repo>` then launch `dist/<name>.exe` — the
      produced game must run without panic (validates the DoD headline
      "build → single exe" workflow end to end).

### Real-hardware audio backend verification (F6)

CI audio coverage stops at "the FFI module compiles" — the actual device
open + write path needs real hardware. Before a release, run the
`#[ignore]`'d real-device smoke test on each OS you support:

- [ ] Windows: `cargo test -p voltex_audio --lib backend::wasapi -- --ignored`
- [ ] Linux: `cargo test -p voltex_audio --lib backend::alsa -- --ignored`
      (requires libasound2-dev + an actual audio device on the runner)
- [ ] macOS: `cargo test -p voltex_audio --lib backend::coreaudio -- --ignored`

Each smoke test opens the default output device, pushes 64 frames of
silence, and tears down. They graceful-skip with `eprintln!("skipping:
no <backend> device")` when the device is missing, so they are safe to
run unattended in a script.

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
