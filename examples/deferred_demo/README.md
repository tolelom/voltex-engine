# deferred_demo

Deferred renderer — G-Buffer + Lighting pass + Ray-Traced Shadows + Bloom +
ACES tone mapping + TAA.

## What it demonstrates

- `voltex_renderer::gbuffer::GBuffer` — 4 MRT attachments (position +
  normal + albedo+metallic + roughness+AO).
- `voltex_renderer::deferred_pipeline::create_gbuffer_pipeline` +
  `create_lighting_pipeline` — geometry → lighting split.
- `voltex_renderer::rt_shadow` — BLAS/TLAS + compute shader ray query for
  hard shadows.
- Post-process chain: HDR target → `bloom` → `tonemap::aces_tonemap` → `taa`.
- SSGI (screen-space global illumination via `voltex_renderer::ssgi`).

## Run

```bash
cargo run -p deferred_demo --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Notes

Requires a wgpu adapter advertising ray query support
(`wgpu::Features::EXPERIMENTAL_RAY_QUERY`). There is no fallback path: on
adapters without it, GPU initialization fails and the demo exits.
(`voltex_renderer::experimental::rt_fallback` has a capability check, but
nothing uses it yet.)

## Next step

→ [`audio_demo`](../audio_demo) — covers the audio pipeline (separate from
the renderer subsystem).
