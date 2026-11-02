# voltex-fuzz

Coverage-guided fuzz targets for Voltex parsers and deserializers.

## Setup

Requires nightly Rust + cargo-fuzz (libFuzzer is unsupported on stable):

```bash
rustup install nightly
cargo install cargo-fuzz
```

## Run a target

From the **repo root**:

```bash
cd fuzz
cargo +nightly fuzz run fuzz_png            # PNG decoder
cargo +nightly fuzz run fuzz_jpg            # baseline + progressive JPEG
cargo +nightly fuzz run fuzz_gltf           # glTF 2.0 + GLB
cargo +nightly fuzz run fuzz_obj            # Wavefront OBJ
cargo +nightly fuzz run fuzz_scene_text     # .vscn text deserializer
cargo +nightly fuzz run fuzz_scene_json     # JSON scene deserializer
cargo +nightly fuzz run fuzz_scene_binary   # VSCN binary deserializer
cargo +nightly fuzz run fuzz_snapshot       # voltex_net snapshot deserializer
cargo +nightly fuzz run fuzz_navmesh        # VNAV navmesh deserializer
```

`cargo fuzz run` blocks forever until it hits a crash or you Ctrl+C. Use
`--max_total_time=300` (seconds) to time-box a session, e.g.:

```bash
cargo +nightly fuzz run fuzz_png -- -max_total_time=300
```

## What to expect

Each target is a thin wrapper that feeds raw libFuzzer input into the
corresponding `parse_*` / `deserialize_*` entry point. The targets do
**not** assert anything beyond "no panic / no UB", because every parser
already has hardening for: zero / huge dimensions, missing fields,
truncated payloads, OOB indices, and unsupported version bytes (see
`docs/dev/STATUS.md` v1.0.40 ~ v1.0.46).

A discovered crash gets written to `fuzz/artifacts/<target>/<sha1>`. Use
that file as a regression-test fixture and add a hand-written test that
calls the same entry point with the same bytes.

## CI

The fuzz crate is **excluded from the workspace** so `cargo build
--workspace` / `cargo test --workspace` ignore it. Running the targets
is a separate cycle, not gated on every PR.
