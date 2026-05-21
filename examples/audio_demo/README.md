# audio_demo

`voltex_audio` end-to-end — loads a WAV (or OGG/Vorbis) clip, sends it to
the platform backend (WASAPI / ALSA / CoreAudio).

## What it demonstrates

- `voltex_audio::AudioClip` + `parse_wav` / `decode_ogg_vorbis_bytes`.
- `voltex_audio::AudioSystem::play(clip_index, volume, looping)`.
- Platform backend selection via `voltex_audio::backend::make_backend`
  (Windows → WASAPI, Linux → ALSA, headless / fallback → Null).
- 3D spatial audio knobs (`AudioSystem::play_3d` + `set_listener`).
- Mix-group volumes (Master / Bgm / Sfx / Voice) + crossfade.

## Run

```bash
cargo run -p audio_demo --release
```

Tested clips ship under `examples/audio_demo/assets/`. On a headless host
(no audio device), the demo falls back to the Null backend silently.

## Next step

→ [`editor_demo`](../editor_demo) — covers the immediate-mode editor stack.
