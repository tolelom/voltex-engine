# voltex_audio

## 책임 (한 줄)
오디오 — WAV/OGG 디코더, WASAPI 백엔드, 3D 사운드, 믹서, 이펙트.

## 공개 API 진입점
- `AudioSystem` — 메인 오디오 시스템 (src/audio_system.rs)
- `AudioClip` — PCM 데이터 (src/audio_clip.rs)
- `AudioSource` — ECS 컴포넌트 (src/audio_source.rs)
- `AsyncAudioLoader` — 비동기 로딩 (src/async_loader.rs)
- WAV/OGG 디코더 — `parse_wav`, `decode_ogg` (src/wav.rs, src/ogg.rs, src/vorbis.rs)
- `MixGroup`, `MixGroupManager`, `AudioBus` — 믹싱 (src/mix_group.rs, src/dynamic_groups.rs, src/audio_bus.rs)
- `EffectChain`, `Reverb`, `EchoDelay` — 이펙트 (src/effect_chain.rs, src/reverb.rs)
- spatial — `distance_attenuation`, `stereo_pan`, `doppler_shift`, HRTF (src/spatial.rs, src/hrtf.rs)
- `Occlusion` — 레이 기반 차폐 (src/occlusion.rs)
- `AudioError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `audio_system` — 메인 시스템 (src/audio_system.rs)
- `audio_clip`, `audio_source` — 데이터 + 컴포넌트 (src/audio_clip.rs, src/audio_source.rs)
- `async_loader` — 비동기 로더 (src/async_loader.rs)
- `wav`, `ogg`, `vorbis` — 디코더 (src/wav.rs, src/ogg.rs, src/vorbis/)
- `mixing`, `mix_group`, `dynamic_groups`, `audio_bus` — 믹서 (src/mixing.rs 등)
- `effect_chain`, `reverb`, `fade_curves` — 이펙트 (src/effect_chain.rs 등)
- `spatial`, `hrtf`, `occlusion` — 3D 오디오 (src/spatial.rs 등)
- `wasapi` — Windows 백엔드 (src/wasapi.rs)

## 의존성
- voltex crate: voltex_math (3D 사운드 좌표)
- 외부 crate: 없음 (WASAPI는 Windows API 직접 사용)

## 변경 시 주의점
- WASAPI는 Windows 전용 — macOS/Linux 백엔드 미구현
- vorbis는 src/vorbis/ 디렉토리로 분리됨 (bit_reader, codebook, floor, residue, headers, imdct, decoder)
- wasapi.rs는 521줄 (spec 만족, 분리 대상 아님)
- AudioSource 컴포넌트는 ECS와 함께 사용

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_audio --lib`
- 현재 테스트 개수: 84개
