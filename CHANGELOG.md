# Changelog

All notable changes to the Voltex Engine are recorded here. Until v1.0 the
project is still **breaking-change-friendly**; from v1.0 onward each entry
flags whether it is breaking, additive, or fix-only.

The format groups changes by release tag (when assigned), with the most
recent on top. v0.x entries are organised by Cycle (see
`docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md`).

---

## [v1.0.11] — 2026-05-21 — `voltex.toml` strict schema + path validation

Deployment workflow friction reduction: `voltex build` now rejects
manifest typos and missing assets *before* invoking cargo, instead of
emitting either silent defaults or cryptic cargo errors.

### Added

- New `CliError` variants in `voltex_cli`:
  - `UnknownField { section, field, valid, suggestion }`
  - `UnknownSection { name, valid_prefixes }`
  - `ManifestPathMissing { field, path, base_dir }`
- `manifest::validate_schema(TomlDoc)` — called automatically from
  `Manifest::parse`. Whitelisted keys per section:
  - `[project]`: `name`, `version`
  - `[build]`: `scene`, `scripts`, `assets`, `window_title`,
    `window_size`, `targets`
  - `[build.<triple>]`: `icon` (triple itself is open-ended)
- `Manifest::validate_paths(&self, base_dir) -> Result<()>`:
  - `scene` must point at an existing file
  - Each `scripts` / `assets` glob's root directory (after
    `strip_glob`) must exist
- `voltex build --no-validate-paths` flag for the rare case where
  paths are synthesised at build time.
- Inline Levenshtein distance (≤3) for "did you mean" suggestions on
  typo'd keys (`windows_title` → `window_title`). No external dep.

### Tests

- `voltex_cli`: 30 → **43** (+13). Unknown key/section 5, path
  validation 3, helpers 2, error Display 3.

### Known limits

- `ManifestParse` errors still only report line, not column.
- `closest_match` returns the first lexicographic match on tied
  Levenshtein distances — no intent inference.

---

## [v1.0.10] — 2026-05-21 — Crash-log panic hook in `voltex_runtime`

Deployed game executables hide stderr from the end user, so a panic in
production used to leave the user with nothing to attach to a bug
report. v1.0.10 closes that gap.

### Added

- New module `crates/voltex_runtime/src/crash.rs`:
  - `install_panic_hook(app_name, dir)` wraps the existing
    `std::panic::set_hook` so the default stderr behaviour still runs
    *and* a `<app>-crash-YYYYMMDD-HHMMSS.txt` file is written
    alongside. `std::sync::Once` makes it idempotent within a process.
  - `format_crash_report(...)` — pure function (no side effects) so
    the format is unit-testable. Includes app name, UTC wallclock,
    OS / arch, `voltex_runtime` crate version, the panic message, the
    panic location (`file:line:col`), and the full backtrace from
    `std::backtrace::Backtrace::force_capture()`.
  - `crash_filename(app, ts)` — sanitises the app name (whitespace,
    slashes, non-ASCII → `_`) so the filename is always valid on
    Windows + Linux + macOS.
  - Inline Howard-Hinnant `days_from_civil` algorithm for UNIX-seconds
    → calendar conversion (no `chrono` / `time` dep, in keeping with
    the runtime crate's external-dep policy).
- `RuntimeConfig` extensions:
  - `enable_crash_log: bool` (defaults to `true`)
  - `crash_log_dir: Option<PathBuf>` (defaults to `None` = current
    working directory at hook-fire time)
  - Builder methods `.with_crash_log_dir(p)` and
    `.disable_crash_log()` — the latter for headless / fuzz harnesses.
- `RuntimeApp::run` installs the hook before constructing the winit
  `EventLoop` when `config.enable_crash_log` is true.

### Tests

- `voltex_runtime`: 13 → **27** (+14). Crash module 11 (epoch,
  2026-05-21 noon UTC, 2000-02-29 leap, pre-epoch negative seconds,
  filename well-formed / sanitised, report content with / without
  location / trailing newline, file write + dir creation, idempotent
  install). Config 3 (default enables, disable toggle, with_dir).
- Workspace lib: 968 → **~982**.

### Known limits

- The hook itself is not exercised via a live `panic!` in tests —
  global panic-hook state is messy to isolate per-test. A separate-
  process integration test is deferred.
- Crash report omits GPU adapter name, FPS history, last loaded scene
  — useful additions for a real bug-report intake. Deferred.

---

## [v1.0.9] — 2026-05-21 — OGG/Vorbis lib.rs wire + clippy 27→0 + AudioClip entry point

First cycle of the **service-readiness roadmap**
(`docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md`).
Resolves the dead-module regression discovered in v1.0.8.

### Fixed

- `crates/voltex_audio/src/lib.rs` was missing `pub mod ogg;` and
  `pub mod vorbis;`, leaving ~1.5k lines of Phase 6-1 source code out
  of the compilation graph. STATUS's "OGG/Vorbis decoder 자체 구현 완료"
  claim was therefore false. Now wired.
- `voltex_audio` clippy warnings: 24 lib + 3 test → **0** (with
  `-D warnings`). Combination of `cargo clippy --fix` (5 auto),
  per-file `#![allow(clippy::needless_range_loop)]` on the 6 Vorbis
  sub-modules (each with a one-line spec-traceability justification),
  and 4 substantive fixes (`needless_late_init`, `manual_memcpy`,
  `enumerate_discard_index`, `same_item_push`).
- Two latent test regressions surfaced by re-enabling compilation:
  - `ogg::tests::invalid_capture_pattern` — short inputs went down
    the size-truncation branch and returned the generic "No OGG
    pages found" instead of "Invalid OGG capture pattern". Fixed by
    moving the magic check ahead of the 27-byte header size check;
    4 bytes are enough to commit on the magic.
  - `vorbis::tests::bitreader_basics` — assertion arithmetic was
    wrong (author over-counted the bits remaining in byte 0).
    Replaced with a correct expectation (22 not 27) plus an
    additional 8-bit read assertion, and clarified the comment.

### Added

- `voltex_audio::decode_ogg_vorbis_bytes(&[u8]) -> Result<AudioClip>`
  — single entry point that composes `ogg::parse_ogg` and
  `vorbis::decode_vorbis`. External callers can decode `.ogg` bytes
  in one call.
- Re-exports: `voltex_audio::{parse_ogg, decode_vorbis}` at crate
  root (previously only reachable via the now-public `ogg` /
  `vorbis` modules).

### Tests

- `voltex_audio` lib: 89 → **119** (+30) — ogg ×11 (4 original +
  4 fuzz + 1 invalid_capture rewrite + 2 entry-point), vorbis ×17
  (11 original + 1 BitReader 8-bit read + 5 more headers/imdct/
  decoder/floor coverage), `decode_ogg_vorbis_bytes` ×2.
- Workspace lib: 942 → **~968**.

### Known limits (deferred)

- Decoding real `.ogg` files end-to-end is **not** verified — the
  Vorbis decoder is simplified (scalar codebooks only, no coupled
  channels, residue type 2 only). `audio_demo` does not yet load
  OGG. Both deferred to a later v1.0.x cycle.

---

## [v1.0.8] — 2026-05-21 — parser panic-resistance fuzz tests + OGG/Vorbis dead-module discovery

### Added (21 new panic-resistance tests across 5 parsers)

Every public parser entry point now has a `fuzz_parse_*` test
suite that feeds it the same four classes of input:

1. empty `&[]`
2. every truncation of a representative valid sample / known magic
   header
3. 128 seeds × 6 lengths of deterministic xorshift32-pseudo-random
   bytes (0..2048 typical range)
4. valid magic prefix + xorshift32 tail (catches the "magic OK but
   then garbage" path, which is the classic source of unwrap /
   index panics)

| Parser            | File                                                    | New tests |
|-------------------|---------------------------------------------------------|-----------|
| `parse_png`       | `voltex_renderer/src/png.rs`                            | 4 |
| `parse_jpg`       | `voltex_renderer/src/jpg/mod.rs`                        | 4 |
| `parse_gltf`      | `voltex_renderer/src/gltf/mod.rs`                       | 5 |
| `parse_obj`       | `voltex_renderer/src/obj.rs`                            | 4 |
| `parse_wav`       | `voltex_audio/src/wav.rs`                               | 4 |
| **Total**         |                                                         | **21** |

All 21 pass. **Zero panics surfaced** — every parser returns an
`Err` (or, for `parse_obj` which returns `ObjData` directly,
returns an empty/partial `ObjData`) on malformed input. The
engine's dependency policy forbids `rand` / `proptest` / `arbitrary`,
so each test file inlines a tiny xorshift32 PRNG (~10 lines, no
allocations beyond the result Vec).

### Discovered (and deferred to v1.0.9)

While adding `fuzz_parse_ogg_*` to `voltex_audio/src/ogg.rs`, the
tests never showed up in `cargo test --lib --list`. Root cause:
`crates/voltex_audio/src/ogg.rs` and `crates/voltex_audio/src/vorbis/`
exist on disk but are **not declared** in `crates/voltex_audio/src/lib.rs`
(no `pub mod ogg;` / `pub mod vorbis;`). Cargo therefore never
compiles them, and they are effectively dead source files.

This contradicts STATUS.md Phase 6-1 ("OGG/Vorbis 디코더 자체
구현 완료") — the code exists but is invisible to the rest of the
build. Tentatively wiring them in surfaced 4 `dead_code` warnings
(spec-required fields the decoder doesn't yet consume) **and** 27
pre-existing clippy errors inside the modules. Fixing all of that
exceeds the v1.0.8 scope; the lib.rs registration was reverted to
keep this patch atomic. A note in lib.rs records the intentional
omission and points at v1.0.9.

v1.0.8 deliberately preserves the `fuzz_parse_ogg_*` tests inside
the unmounted `ogg.rs` so v1.0.9 can wire the module and the tests
will start running on the first re-registration. The source change
to `ogg.rs` is therefore harmless dead code today and useful
infrastructure tomorrow.

### Not in scope (deferred to v1.0.9+)

- Wire `voltex_audio::ogg` + `voltex_audio::vorbis` into the
  module graph, fix the 27 clippy errors and 4 dead-code warnings
  they bring in, plumb vorbis output through `AudioClip` so OGG
  files are actually loadable, run the OGG fuzz tests.
- Coverage-style fuzz orchestration (cargo-fuzz / honggfuzz /
  AFL.rs) — current tests are deterministic and CI-friendly, which
  is the right v1.0 trade-off. Coverage-guided fuzzing is v1.1+.

### Verified

- 921 → **942** lib tests (+21 fuzz, no behavior changes).
- `cargo clippy --workspace --all-targets -- -D warnings` still
  passes (one unrelated unnecessary_cast in `parse_obj`'s test
  helper fixed in this patch — `(32 + r) as u8 as char` → `as
  char`).
- voltex_cli still 30 tests; workspace build still 0 warnings.

---

## [v1.0.7] — 2026-05-21 — third-party license inventory + license-check CI job

Closes P4 #28 from the post-v1.0.5 audit ("외부 라이선스 고지
미작성") and the README's own "v1.0 release artifact 에 포함 예정"
deferral.

### Added

- **`THIRD-PARTY-LICENSES.md`** (auto-generated, 4,795 lines) —
  every transitive Cargo dependency that ends up in a Voltex build,
  grouped by SPDX license, with the full license text inlined.
  Generated by `cargo about generate about.hbs -o
  THIRD-PARTY-LICENSES.md`. Versioned in the repo so external users
  see it without rebuilding.
- **`about.toml`** — cargo-about policy. Declares the accepted SPDX
  list: MIT, Apache-2.0 (with LLVM exception variant), BSD-2/3,
  BSL-1.0, ISC, Zlib, Unicode-3.0 / Unicode-DFS-2016, MPL-2.0,
  CC0-1.0, OFL-1.1. Any new transitive outside this list will fail
  the CI license-check job.
- **`about.hbs`** — handlebars template producing the markdown
  output. Lists per-license summary + per-crate listing with
  repository links + collapsible license text.
- **`[workspace.package] license = "MIT OR Apache-2.0"`** in the
  root Cargo.toml, with `license.workspace = true` added to all 31
  workspace crate Cargo.toml files (15 lib + 14 example + 1 game +
  1 facade). Without this, `cargo about` warned that every voltex
  crate had no license expression — making the output dishonest.
- **`.github/workflows/ci.yml` — new `license-check` job**
  (Ubuntu, `continue-on-error: true`). Installs cargo-about via
  `taiki-e/install-action`, runs `cargo about generate` against
  `about.toml`. Fails (advisory) on:
  - any transitive crate whose license is outside the accepted list
  - any crate cargo-about cannot determine the license for
  Advisory rather than blocking until the policy stabilizes.

### Audited (license inventory)

`cargo about generate` reports 284 license-bearing crates in the
final dependency graph, broken down as:

| License        | Crates |
|----------------|--------|
| MIT            | 264    |
| Apache-2.0     | 11     |
| Zlib           | 3      |
| BSD-3-Clause   | 2      |
| BSD-2-Clause   | 1      |
| CC0-1.0        | 1      |
| ISC            | 1      |
| Unicode-3.0    | 1      |

**Zero copyleft (GPL/AGPL/LGPL) and zero proprietary** — every
transitive license is compatible with both arms of voltex's MIT OR
Apache-2.0 dual license.

### Documentation

- **README "License" section** rewritten:
  - Removed the deferral note ("외부 의존 라이선스 고지는 작업 후속")
    that was pending since v1.0 release.
  - Links to `THIRD-PARTY-LICENSES.md`, includes the per-license
    count, states the zero-copyleft fact explicitly.
  - Documents the regeneration command for future contributors.
  - References the CI license-check job.
- **README badges**: status 1.0.6 → 1.0.7, new
  `licenses: permissive only` badge.

### Verified (self-feedback at each stage)

- Stage 1 install + generation: 4,795-line file produced.
- Stage 2 analysis: 8 distinct licenses, all permissive, zero
  copyleft.
- Stage 3 README + CI: cosmetic only.
- Stage 4 final gate: `cargo build` / `clippy -D warnings` / `test
  --lib` / `test -p voltex_cli` / `cargo about generate` all green
  with 0 warnings, 0 errors.

### Not in scope (deferred to v1.1+)

- Promoting `license-check` from advisory to blocking — depends on
  the first real CI run.
- Uploading `THIRD-PARTY-LICENSES.md` into release artifacts
  (`dist/<name>.exe` companion file) — `voltex_cli build` does not
  yet copy it.
- `cargo-deny` for combined license + advisory + duplicate scan —
  cargo-audit + cargo-about already cover the high-value cases.

---

## [v1.0.6] — 2026-05-21 — coverage baseline (82%) + advisory CI job + CLI engine-path friendliness

### Measured (coverage baseline)

First time the project has a number for `rules/common/testing.md`'s
"80%+ coverage" target. `cargo install cargo-llvm-cov` + `rustup
component add llvm-tools-preview` was needed once; result:

| metric    | covered / total | percent |
|-----------|-----------------|---------|
| Functions | 34878 / 42505   | **82.06%** |
| Lines     | 18142 / 24849   | **82.74%** |
| Regions   | (see report)    | **73.01%** |

Per-crate breakdown (function coverage), 14 crates:

- **🟢 80%+ (10 crates)**: voltex 100%, voltex_math 95.85%,
  voltex_script 94.33%, voltex_ecs 93.31%, voltex_asset 93.17%,
  voltex_ai 92.85%, voltex_net 92.08%, voltex_physics 89.85%,
  voltex_audio 88.26%, voltex_editor 82.27%.
- **🟡 below 80% (4 crates, all GPU/winit-dependent)**:
  voltex_renderer 65.94% (`shadow.rs` / `shadow_pipeline.rs` /
  `rt_*.rs` / `taa.rs` / `ssgi.rs` / `ssr.rs` / `temporal_accum.rs`
  — GPU compute shader paths, need a `wgpu::Device` to exercise),
  voltex_runtime 61.22% (`handler.rs` 0% — winit `ApplicationHandler`
  callbacks), voltex_renderer_ecs 53.83% (`BasicRenderer::render`,
  GPU pass), voltex_platform 46.38% (`ApplicationHandler` callbacks +
  per-platform key mapping).

→ The four low-coverage crates share a single root cause: their
public surface is **GPU + winit integration code that cannot be
reached from a headless `cargo test --lib`**. Their unit-testable
internals (`app.rs`, `config.rs`, `error.rs`, `RenderItem`
collection, etc.) all sit at 80%+. Closing the gap requires
integration tests against a headless GPU adapter and a stubbed
event loop — scheduled for v1.1+.

### Added (infrastructure)

- **`.github/workflows/ci.yml` — new `coverage` job** (Ubuntu,
  `continue-on-error: true`). Installs llvm-tools + cargo-llvm-cov
  via `taiki-e/install-action`, then runs `cargo llvm-cov
  --workspace --lib --summary-only`. Advisory only — threshold
  enforcement (`--fail-under-lines 80`) postponed until GPU paths
  are covered by integration tests.
- **README "Contributing" → "Coverage" subsection**: documents the
  one-time install commands, the current baseline number, and why
  the 4 low-coverage crates are low.
- **README badge**: `coverage 82% lib`. Status badge bumped 1.0.5
  → 1.0.6.

### Improved (CLI friendliness — P1 #5)

- **`voltex build` engine-path missing error** rewritten as a
  multi-line help block. Old (1 line): `"engine path unknown — set
  VOLTEX_REPO or pass --engine-path <repo-root>"`. New (7 lines):
  scaffolded-project example + both shell-specific export commands
  (bash/zsh + PowerShell), so new users get a copy-pasteable fix in
  the error itself.
- **README 5-min quickstart**: split into "예제 실행" + "새 게임
  만들기" subsections. The second one walks through `cargo install
  --path crates/voltex_cli` → `export VOLTEX_REPO=$(pwd)` → `voltex
  new` → `voltex build`. Closes the discoverability gap where users
  saw `cargo run --example triangle` but could not figure out how
  to scaffold their own project.

### Verified (self-feedback at each stage)

- Stage 1: cargo-llvm-cov install + baseline measurement (82.06%).
- Stage 2: per-crate analysis, low crates identified and justified.
- Stage 3: README + CI yml updated, no behavior change.
- Stage 4: CLI tweak — empty-directory repro of `voltex new && cd &&
  voltex build` (without VOLTEX_REPO env) now prints the 7-line
  help block instead of the 1-line one.
- Stage 5: `cargo build --workspace` clean, `cargo test --workspace
  --lib` = 921, `cargo test -p voltex_cli` = 30. clippy still 0.

### Not in scope (deferred to v1.1+)

- Lifting the 4 low-coverage crates above 80% — requires headless
  GPU adapter testing + winit event-loop stubbing.
- Enabling `--fail-under-lines 80` as a CI gate — depends on the
  above.
- HTML coverage report upload (codecov / coveralls) — depends on
  the CI workflow actually running on a runner.

---

## [v1.0.5] — 2026-05-21 — clippy clean (warnings 84 → 0) + cargo audit + CI workflow

This is the "quality gate" patch. v1.0.3 (Windows build P0) and v1.0.4
(clippy deny errors) both slipped past internal verification because
`cargo build` + `cargo test --lib` was the only standing check. v1.0.5
closes the loop: zero clippy warnings, zero open advisories, and a CI
workflow that enforces both on every push.

### Fixed (clippy warnings 84 → 0)

Baseline measurement before this patch: `cargo clippy --workspace
--all-targets` produced **84 lines of warnings/errors** (51 unique
diagnostics across 13 lint categories).

**Auto-fixed** (`cargo clippy --fix --workspace --all-targets`, 43
suggestions across `voltex_math`, `voltex_audio`, `voltex_platform`,
`voltex_ecs`, `voltex_cli`, `voltex_renderer`, and 3 examples):
- `manual_div_ceil` ×13 → `.div_ceil(...)`
- `manual RangeInclusive::contains` ×4 → `.contains(&x)`
- `excessive_precision` ×4 (audio wav constants)
- `manual_is_multiple_of` ×1 → `.is_multiple_of(2)`
- `collapsible_if` ×1, `identity_op` ×1, `len_zero` ×1,
  `unwrap_or_else_default` ×1
- subset of `needless_range_loop` (iterator-convertible cases)

**Manually fixed** (24 residual diagnostics, semantic intent required):
- `voltex_audio::mixing::make_stereo_clip` and
  `voltex_ecs::world::tests::Name` — both genuinely dead, deleted
  outright.
- `voltex_script::state` ×5 — `b"name\0".as_ptr() as *const _`
  replaced with C-string literal `c"name".as_ptr()` (Rust 1.77+).
- `voltex_renderer::jpg::mod` ×4 — `for _ in 0..N { vec.push(byte); }`
  collapsed to `vec.extend_from_slice(&[byte; N])` for JPEG marker
  padding.
- `voltex_math::mat4`, `voltex_renderer::sh` ×4 — `needless_range_loop`
  cases where `r`/`c`/`k`/`ch` indices appear in both lookup AND
  assert message: annotated `#[allow(clippy::needless_range_loop)]`
  with a comment.
- `voltex_editor::panels::hierarchy::draw_entity_node`,
  `voltex_editor::picking::pick_entity`,
  `deferred_demo::create_shadow_bind_group` — `too_many_arguments`
  cases where args are intrinsic (recursive draw / viewport rect +
  mouse + camera / 9-binding GPU layout): annotated `#[allow]`.
- `voltex_cli::manifest::extract_build` — `very_complex_type` (6-tuple
  return) refactored to return `BuildConfig` directly; the destructure
  at the call site collapsed to single `build` field. Net code
  shorter, type signature self-documenting.
- `examples/{shadow_demo,deferred_demo,asset_demo}` — small loop
  refactors and 1 `#[allow]` where index threads multiple parallel
  arrays.

### Added (CI infrastructure)

- **`.github/workflows/ci.yml`** — GitHub Actions matrix that runs
  `cargo build --workspace`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace --lib`, and
  `cargo test -p voltex_cli` on **ubuntu-latest + windows-latest**
  (blocking) and **macos-latest** (advisory, `continue-on-error`).
  Linux job installs `libasound2-dev` before build to satisfy
  `voltex_audio`'s ALSA FFI link. Cache layer keys on `Cargo.lock`.
  File is benign on non-GitHub remotes — gitea/forgejo also accept
  the syntax.
- **README "Contributing" section** — documents the exact four-command
  local verify recipe that mirrors CI, so PR authors can reproduce
  the gate before pushing.

### Audited (cargo audit)

- Installed `cargo-audit` (one-time tool, not a runtime dep).
- Scan of 298 `Cargo.lock` crates against 1,096 advisories:
  **0 vulnerabilities**.
- **1 warning** — `paste 1.0.15` flagged unmaintained
  (RUSTSEC-2024-0436). Transitive: `paste ← metal ← wgpu-hal ←
  wgpu`. Cannot be addressed at voltex level; resolved by future
  wgpu upgrade. Documented for awareness, no action required.

### Verified (self-feedback at each stage)

- Stage 1 baseline: 84 warnings counted.
- Stage 2 post-auto-fix: 41 warnings counted, 921 lib tests pass.
- Stage 3 post-manual-fix: **0 warnings**, 921 lib tests pass,
  30 voltex_cli bin tests pass.
- Stage 4 cargo audit: 0 vulns, 1 advisory unrelated to voltex code.
- Stage 5 CI gate re-run locally: `RUSTFLAGS="-D warnings"
  cargo {build,clippy,test}` all green.

### Not in scope (deferred to v1.1+)

- Actually running the CI workflow on a GitHub runner — requires a
  GitHub mirror. The workflow file ships ready-to-use; the first
  green run on a real CI is the next-cycle verification step.
- 3-OS exe runtime verification (Linux ALSA audio output, macOS
  CoreAudio audio output) — still environment-dependent.
- Adding `cargo audit` to the CI workflow — `cargo-audit` itself
  needs an install step; postponed pending a CI-friendly setup.

---

## [v1.0.4] — 2026-05-21 — clippy `approx_constant` clean + README dependency honesty

### Fixed
- **clippy `approx_constant` errors (9 → 0)** across
  `voltex_physics::integrator`, `voltex_renderer::animation`,
  `voltex_ecs::world::tests`, `voltex_ecs::json`, and
  `voltex_renderer::json_parser`.
  - `integrator.rs` test used `3.14159` for `rb.angular_velocity` /
    expected_rot_y → replaced with `std::f32::consts::PI` (real intent
    was 1 PI rad/s — comment already said so).
  - `animation.rs::test_quat_slerp_endpoints` used `0.7071` for 90°
    quaternion components → replaced with `std::f32::consts::FRAC_1_SQRT_2`
    (more precise + self-documenting).
  - `world/tests.rs::test_clear_changed_all_storages` used `3.14f32`
    only as "a different-typed value" → replaced with `2.5f32` (no
    semantic dependency on the value at all).
  - `json.rs::test_write_number`, `json.rs::test_parse_negative_number`,
    `json_parser.rs::test_parse_number` use `3.14` / `-3.14` as
    representative decimal payloads for serialization round-trip —
    annotated with `#[allow(clippy::approx_constant)]` + comment
    explaining the intent (the value is data, not π).
  - Net result: `cargo clippy --workspace --all-targets` no longer
    fails on `#[deny(clippy::approx_constant)]`. External contributors
    can now run clippy without a hard error.

### Corrected (documentation honesty)
- **README "외부 crate 의존 단 2개 (winit / wgpu)" claim was inaccurate.**
  Actual measured direct dependencies:
  - **Runtime (3):** `winit`, `wgpu`, `bytemuck` (Pod/Zeroable derive +
    cast — wgpu's de-facto companion, but a real direct dep nonetheless).
  - **Build-only (2, never present in shipped runtime):** `thiserror`
    (`Error` derive macro; no runtime crate after compilation), `cc`
    (`voltex_script`'s Lua 5.4 C source `build.rs`).
  - **Example/binary helpers (not in any library crate):** `log`,
    `env_logger`, `pollster`. Listed separately so future audits don't
    mistake them for engine deps.
  - **Transitive total:** 298 crates in `Cargo.lock` (dominated by
    wgpu's GPU stack and winit's per-OS windowing).
  README header and "의존성 정책" table updated to reflect this with
  rationale per crate.

### Verified
- `cargo clippy --workspace --all-targets` — `approx_constant` count
  3 errors / 5 hits  →  0.
- `cargo test --workspace --lib` — 921 lib tests still pass (the 3
  test files touched are part of the standard suite; `voltex_physics`
  106 / `voltex_ecs` 134 / `voltex_renderer` 200 unchanged).
- No production-code changes — only test bodies and 2 `use std::f32::consts::*`
  imports inside `#[cfg(test)]` modules.

### Not in scope (deferred to v1.1+)
- The remaining ~80 clippy warnings (manual `RangeInclusive::contains`,
  `needless_range_loop`, dead code in `voltex_audio::make_stereo_clip`
  / `voltex_ai::Name`, excessive float precision in `voltex_audio::wav`,
  etc.) are non-deny lints. v1.0.4 deliberately addresses only the
  hard errors that block external `cargo clippy --all-targets` runs.
- CI workflow that enforces `cargo clippy --workspace --all-targets`
  going forward — recommended next step but not added in this patch.

---

## [v1.0.3] — 2026-05-21 — `voltex build` Windows P0 regression fix

### Fixed
- **P0 — `voltex build` failed on every Windows host**.
  `crates/voltex_cli/src/build.rs:96-97` (and `run.rs:20`) called
  `std::fs::canonicalize`, which on Windows always returns the
  extended-length `\\?\C:\...` verbatim form. That prefix landed in the
  generated `dist/wrapper/Cargo.toml` as
  `path = "\\\\?\\C:\\...\\voltex_runtime"`, and Cargo rejected it with
  `invalid path url '//?/C:\\...\\voltex_runtime'`. Result: the
  README's 5-minute quickstart (`voltex new my_game && cd my_game &&
  voltex build`) was 100 % broken for external users. The flagship
  `games/voltex_survivor` and `examples/tutorial_minigame` were
  unaffected because they are workspace members built directly with
  `cargo build`, hiding the regression from internal verification.

### Added
- `voltex_cli::util::canonicalize_friendly(&Path) -> Result<PathBuf>`:
  canonicalize, then strip the Windows `\\?\` verbatim prefix for
  drive-letter paths (`\\?\C:\foo` → `C:\foo`). UNC paths
  (`\\?\UNC\server\share\...`) are intentionally left untouched —
  rewriting them is lossy. No new external dependencies (engine policy
  forbids `dunce` etc.).
- Regression tests (+4): `strip_verbatim_prefix` drive-letter strip,
  UNC passthrough, non-verbatim passthrough (all Windows-gated), and
  cross-platform `canonicalize_friendly_returns_non_verbatim_path`.

### Verified
- End-to-end CLI workflow re-run on Windows host: `voltex new my_game`
  → `voltex build --engine-path <repo>` → `dist/my_game.exe` produced
  in 61 s, wrapper `Cargo.toml` contains
  `voltex_runtime = { path = "C:/Users/<user>/..." }` (no `\\?\`).
- 921 lib tests still pass. `voltex_cli`: 26 → 30 (+4).
- Workspace build: 0 warnings, 0 errors.

### Cleanup
- `examples/many_cubes/src/main.rs:8` — removed unused `Mat4` import
  (only `Vec3` is actually used). Was the lone warning in the
  workspace build.

### Not changed
- No behavior change for `voltex_survivor`, examples, or any engine
  crate. Patch is confined to `voltex_cli` (and the trivial
  many_cubes import cleanup).

---

## [v1.0.2] — 2026-05-21 — 600-line rule re-enforcement + full example runtime sweep

### Changed (refactor only — no behavior change)
- `voltex_editor::session` (1060 lines) → `session/{mod.rs 445, tests.rs
  483, e3_tests.rs 152}`. E3 tests carry their own `temp_path` / `cleanup`
  helpers so each file compiles independently.
- `voltex_script::engine_api` (713) → `engine_api/{mod.rs 356, tests.rs
  357}`.
- `voltex_ecs::world` (665) → `world/{mod.rs 351, tests.rs 314}`.
- `voltex_cli::manifest` (612) → `manifest/{mod.rs 450, tests.rs 162}`.
- After splits, the largest source file is `voltex_physics::solver` at
  574 lines (under the project's 600-line guideline).

### Verified
- Runtime smoke-test on the 8 previously-unverified examples
  (model_viewer / many_cubes / asset_demo / multi_light_demo /
  shadow_demo / ibl_demo / deferred_demo / editor_demo) — all 8 exit 124
  (timeout-clean, no panic). With v1.0.1's voltex_survivor +
  tutorial_minigame + runtime_demo + audio_demo + triangle + pbr_demo +
  hierarchy_demo, **all 15 examples + 1 game are Windows-host runtime
  verified**.

### Documentation sync
- STATUS.md M3 row corrected (was 'no violations' — became 4 violations
  during Cycles 4-13 before re-split).
- STATUS.md voltex_ecs test count fixed (was 131, actual 134).
- DEFERRED.md: gizmo / Despawn-undo / mesh AABB picking marked ✅ (done
  in Cycles 3 and 12 but never reflected in DEFERRED).
- DEFERRED.md: new 'v1.0.x 추가 한계' section enumerates every deferral
  introduced in v1.0 / v1.0.1 / v1.0.2 with its source cycle.
- CLAUDE.md 600-line rule references v1.0.2 re-split.

### Tests
- 0 new tests (pure refactor). 921 --lib tests still pass.

---

## [v1.0.1] — 2026-05-21 — Runtime verification + P0/P1 regression patch

### Fixed
- **handler.rs camera ↔ user-system WASD double-binding** (P0). FPS camera
  movement now requires RMB-held, matching scene-view convention. Games can
  use WASD freely via `add_input_system`.
- **`is_key_just_pressed()` always false in user systems** (P1). Moved
  `app.input.begin_frame()` to end-of-frame so just_pressed survives into
  user-system observation.
- **`Bounds::default_bytes` was zero-volume AABB** (P0). Returned 24 zero
  bytes → frustum culling discarded every Inspector "+ bounds" entity. Now
  encodes the unit cube `min=(-0.5)^3, max=(0.5)^3`.
- **`ScriptComponent::default_bytes` was empty path** (P0). Inspector "+
  script" produced an unrunnable component. Now encodes
  `UNBOUND + "scripts/spinner.lua"` so the placeholder exercises the script
  system end-to-end on every `voltex new` project.
- **voltex_survivor enemies / projectiles never executed scripts**. Lua
  `spawn_entity()` returns a bare entity with no ScriptComponent. The
  game's handle-attach system was extended to attach the matching .lua
  based on Tag (`enemy` / `projectile`).

### Added
- 5 regression tests (`voltex_runtime` ×2 input_system order/reference,
  `voltex_ecs` ×3 default_bytes round-trips and non-degenerate Bounds).

### Verified at runtime (Windows host)
- `cargo run -p voltex_survivor`: 6 waves in 30s, 24 enemy kills, no panics.
- `cargo run -p tutorial_minigame`: score system prints, no panics.
- `cargo run -p runtime_demo`: clean exit.

---

## [v1.0.0] — 2026-05-21

### Added (Cycle 15, D3)
- `games/voltex_survivor/` — flagship game. Wave-based arena shooter with
  4 Lua scripts (game/player/enemy/projectile) + scene + Rust input system.
  Game logic 100% in Lua + scene.
- `voltex_runtime::RuntimeApp::add_input_system(F)` — per-frame system that
  receives `&InputState` alongside `&mut World, f32`. Powers WASD movement
  for voltex_survivor.
- `docs/v1.0-retrospective.md` — what worked, what was deferred, statistics,
  next-priority list for v1.1.
- `STATUS.md`: v1.0 release header + Cycle 15 entry.
- Git tag `v1.0.0` (local — user push activates remote release).

### Added (Cycle 14, D1+D2)
- `README.md` — 5-minute quickstart + code-first / editor-first workflows
  + feature list (14 categories) + crate dependency graph + license +
  contributing + verification-limit notes.
- `docs/tutorial.md` — 7-chapter 30-minute tutorial from scaffold to built exe.
- `examples/tutorial_minigame/` — final artifact of the tutorial, dual-workflow
  compatible (cargo run + voltex build). 1 player + 3 enemies + spinner.lua
  + enemy.lua + score system in stdout.

### Deferred to v1.1
- Lua input API (currently Rust handles input via `add_input_system`).
- `voltex_cli editor` subcommand (tutorial points at `examples/editor_demo`).
- 3-OS exe artifact verification (only Windows build verified locally).
- Screenshots — README + tutorial have placeholder markers; PR welcome.

---

## [Unreleased pre-v1.0]

### Added (Cycle 13, A — 2026-05-21)
- `voltex` facade `prelude` now re-exports `Bounds`, `MeshHandle`,
  `MaterialHandle`, `ScriptComponent`, `RuntimeApp`, `RuntimeConfig`, `Vfs`.
- `voltex::runtime` module re-export (was missing in B0).
- Crate-level rustdoc on `voltex` lib + tier-1 API docstrings on `World`,
  `Entity`, `EditorCommand`, `EditorSession`.

### Fixed (Cycle 13, A — 2026-05-21)
- `cargo doc --workspace --no-deps` now emits zero warnings (intra-doc link
  escapes in `voltex_math`, `voltex_physics`, `voltex_ai`, `voltex_script`,
  `voltex_renderer`; `voltex_cli` bin's `doc = false` skirts cargo issue
  #6313 collision with the `voltex` lib).

### Deferred to v1.1
- Full `#![warn(missing_docs)]` sweep — 1586 missing public-item docstrings
  across the workspace. v1.0 documents prelude + tier-1 API only.
- Public-item `pub(crate)` audit. Candidate list lives in
  `docs/v1.0-pub-audit.md` (notes only, no code changes).

---

## Cycle 12 (E3, flex — 2026-05-21)

### Added
- `EditorCommand::AddComponent { entity, name, data }` and
  `EditorCommand::RemoveComponent { entity, name }`.
- `EntityAllocator::restore_at_generation(id, gen) -> bool` and the
  matching `World::restore_at_generation` delegate.
- `Component` trait gains default-implemented `remove()` and `default_bytes()`
  methods (backward compatible — existing impls auto-pick up the defaults).
  Built-ins override `default_bytes` with sensible values.
- Inspector panel adds a "+ <name>" button row for the six registered
  built-in components.

### Changed (potentially breaking for in-tree callers)
- `Despawn` is no longer history-clearing — it is fully undoable via a new
  `Inverse::RestoreEntity` that snapshots every registered component
  pre-despawn and restores the original `(id, generation)` on undo.
- `ComponentEntry` grows two fn-pointer fields (`remove`, `default_bytes`).
  `register_raw` populates them with no-op stubs.

---

## Cycle 11 (B3 — 2026-05-21)

### Added
- `voltex_audio::backend::CoreAudioBackend` (macOS, `cfg(target_os = "macos")`).
  AudioToolbox AudioQueue via raw FFI, push-style wrap via Condvar-guarded
  free queue.
- `voltex_cli::build::validate_target` enforces native-only build policy
  with friendly cross-compile / manifest-mismatch error messages.
- `docs/build-matrix.md` — 3-OS prerequisites + commands + voltex.toml
  multi-target example + optional GitHub Actions matrix snippet.

---

## Cycle 10 (P1 — 2026-05-21)

### Added
- `voltex_audio::backend` module with `AudioBackend` trait + `BackendError`.
- `WasapiBackend` (Windows), `AlsaBackend` (Linux, raw libasound FFI),
  `NullBackend` (headless / CI fallback).
- `make_backend()` picks the best OS backend, with hard fallback to null.

---

## Cycle 9 (B2 — 2026-05-21)

### Added
- `voltex_asset::vpkg` — custom `.vpkg` archive format (magic + index table
  + concatenated blobs, no external deps).
- `voltex_asset::vfs::Vfs` — `Filesystem(PathBuf)` or `Archive(VpkgReader)`,
  single `read`/`exists`/`list` API.
- `voltex_runtime::RuntimeConfig::with_vfs(Vfs)` — archive-first scene loader
  (filesystem fallback retained for dev mode).
- `voltex build` packs scene + scripts + assets into a `.vpkg` and embeds it
  in the wrapper via `include_bytes!`, producing self-contained executables.

---

## Cycle 8 (B1 — 2026-05-21)

### Added
- `voltex_cli` production CLI with `new` / `build` / `run` subcommands.
- `voltex.toml` manifest (self-contained mini TOML parser, no external deps)
  with `[project]`, `[build]`, `[build.<triple>]` sections.
- Human-friendly error messages (`MissingField`, `ManifestParse`,
  `CargoMissing`, …).

---

## Cycle 7 (B1-spike — 2026-05-21)

### Added
- `voltex_cli` 1-session prototype + `docs/superpowers/notes/2026-05-21-b1-spike-learnings.md`
  documenting target dir, host triple, and packing decisions for Cycle 8.

---

## Cycle 6 (B0 — 2026-05-21)

### Added
- `voltex_runtime` crate (L3): `RuntimeApp` (winit `ApplicationHandler` +
  GPU init + system tick) + `RuntimeConfig` builder + `RuntimeError`/`RuntimeResult`.
- `examples/runtime_demo` — solar-system scene, `main` body 4 lines.

---

## Cycle 5 (S1 — 2026-05-21)

### Added
- `voltex_ecs::ScriptComponent { script_path, instance_id }` as the sixth
  built-in component.
- `voltex_script::ScriptInstancePool` — one `LuaState` per .lua path,
  shared across all entities pointing at it.
- `script_start_all` / `script_update_all` systems (Lua `on_start(id,gen)` /
  `on_update(id,gen,dt)` dispatch).
- Lua `iter_entities()` / `iter_by_tag(tag)` engine API (15 → 17 fns).

---

## Cycle 4 (E2 — 2026-05-20)

### Added
- `EditorMode { Edit, Play }` + `EditorSession::set_mode` with binary-snapshot
  Play→Stop restore.
- `lifecycle` module — `LifecycleEvent { PlayStart, PlayStop }` with callback
  list for systems (physics, scripts) to hook the transitions.
- `EditorError::ModeMismatch` — dispatch is rejected while in Play mode.

---

## Cycle 3 (E1 — 2026-05-20)

### Added
- `voltex_editor::gizmo` — translate / rotate / scale handles, LINE-list wgpu
  pipeline, depth-test Always overlay rendering.
- `picking::pick_entity` upgraded from sphere approximation to AABB picking
  via the `Bounds` component (sphere fallback kept).
- `examples/editor_demo` integrates the gizmo on the toolbar.

---

## Cycle 2 (C0b — 2026-05-20)

### Added
- `voltex_renderer_ecs` crate (L3) — `RenderAssets` (lockstep handle table),
  `BasicRenderer` (per-entity dynamic UBO pipeline), `RenderItem` +
  `collect_render_items` (helper for custom PBR pipelines).
- `editor_demo` + `survivor_game` migrated off the parallel `Vec<Mesh>`
  pattern to ECS-driven rendering.

---

## Cycle 1 (C0a — 2026-05-20)

### Added
- `voltex_ecs::MeshHandle`, `MaterialHandle`, `Bounds` components (with phantom
  `MeshAsset` / `MaterialAsset` marker types to separate handle ID spaces).
- Auto-registered by `ComponentRegistry::register_defaults()`.

---

## Pre-roadmap milestones

### AI-friendly restructure M1–M7 (2026-05-20)
- CODEMAP.md per crate + CLAUDE.md dependency graph (M1).
- File-split: every `.rs` ≤ 600 lines (M2 + M3).
- Error standardization on `thiserror`-based `<Crate>Error` + `Result<T>` aliases (M4).
- `Component` trait + `register::<T>()` standard registration (M5).
- `voltex` facade crate + `prelude` (M6).
- Final verification + `hierarchy_demo` migration (M7).

### Phases 1–8 (2026-03-24 → 2026-05-19)
Foundation through editor command machinery. See `docs/STATUS.md` for the
per-phase breakdown (Foundation → Rendering → ECS+Scene → PBR → Physics →
Audio → Advanced rendering → AI/Net/Lua/Editor + Editor Command).

---

[Unreleased]: https://git.tolelom.xyz/tolelom/game_engine/compare/main...HEAD
