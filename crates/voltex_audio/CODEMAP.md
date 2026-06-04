# voltex_audio

## 책임 (한 줄)
오디오 — WAV/OGG 디코더, AudioBackend (WASAPI/ALSA/CoreAudio/Null), 3D 사운드, 믹서, 이펙트.

## 공개 API 진입점
- `AudioSystem` — 메인 오디오 시스템. `add_clip(&mut, AudioClip) -> usize`(런타임 핫로드, FIFO 로 인덱스 lockstep) / `replace_clip(idx, AudioClip)`(핫리로드, out-of-range 는 무시) (src/audio_system.rs)
- `AudioClip` — PCM 데이터 (src/audio_clip.rs)
- `AudioSource` / `ClipId` — per-entity 재생 디스크립터 + 타입화된 clip 식별자. ECS 컴포넌트로 등록 가능하나 audio crate 자체는 ECS 무의존 (src/audio_source.rs)
- `AsyncAudioLoader` — 비동기 로딩 (src/async_loader.rs)
- WAV/OGG 디코더 — `parse_wav`, `decode_ogg` (src/wav.rs, src/ogg.rs, src/vorbis.rs)
- `load_clip_file(path) -> Result<AudioClip>` — 파일 read + `decode_clip_bytes` 원콜 디코드 (src/lib.rs; 에디터/핫리로드 공용)
- `decode_clip_bytes(path, bytes) -> Result<AudioClip>` — in-memory bytes 를 path 확장자(wav/ogg) 분기로 디코드. 경로는 확장자/에러 메시지에만 사용 — Vfs(.vpkg) 소스 지원 (v1.0.103) (src/lib.rs)
- `MixGroup`, `MixGroupManager`, `AudioBus` — 믹싱 (src/mix_group.rs, src/dynamic_groups.rs, src/audio_bus.rs)
- `EffectChain`, `Reverb`, `EchoDelay` — 이펙트 (src/effect_chain.rs, src/reverb.rs)
- spatial — `distance_attenuation`, `stereo_pan`, `doppler_shift`, HRTF (src/spatial.rs, src/hrtf.rs)
- `Occlusion` — 레이 기반 차폐 (src/occlusion.rs)
- `AudioBackend` trait + impls (P1+B3): `WasapiBackend` (Windows), `AlsaBackend` (Linux), `CoreAudioBackend` (macOS, B3), `NullBackend` (헤드리스/CI), `make_backend()` (OS 자동 분기) (src/backend/)
- `BackendError` — backend-level error type
- `AudioError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `audio_system` — 메인 시스템 (src/audio_system.rs)
- `audio_clip`, `audio_source` — 데이터 + 컴포넌트 (src/audio_clip.rs, src/audio_source.rs)
- `async_loader` — 비동기 로더 (src/async_loader.rs)
- `wav`, `ogg`, `vorbis` — 디코더 (src/wav.rs, src/ogg.rs, src/vorbis/)
- `mixing`, `mix_group`, `dynamic_groups`, `audio_bus` — 믹서 (src/mixing.rs 등)
- `effect_chain`, `reverb`, `fade_curves` — 이펙트 (src/effect_chain.rs 등)
- `spatial`, `hrtf`, `occlusion` — 3D 오디오 (src/spatial.rs 등)
- `wasapi` — Windows 백엔드 FFI (src/wasapi.rs)
- `backend` — `AudioBackend` trait + `null.rs` + `wasapi.rs` (windows) + `alsa.rs` (linux) + `coreaudio.rs` (macos, B3) + `make_backend` (P1+B3)

## 의존성
- voltex crate: voltex_math (3D 사운드 좌표)
- 외부 crate: thiserror (에러 타입)

## 변경 시 주의점
- 오디오 스레드가 클립 `Vec<AudioClip>` 를 **단독 소유(mut)** — `AddClip`/`ReplaceClip` 커맨드로만 변경. `AudioSystem.clip_count` 는 mpsc FIFO 덕에 스레드 쪽 Vec 길이와 lockstep
- 재생 중 클립 교체는 안전: 믹서가 out-of-range `clip_index` skip + `position >= frame_count` 시 자연 종료/루프 (mixing.rs 가드, 회귀 테스트 있음)
- WASAPI 는 Windows 전용 FFI — `backend::WasapiBackend` 가 trait impl 로 래핑
- ALSA 는 Linux 전용 FFI — `backend::AlsaBackend` (cfg(target_os = "linux") only). `#[link(name = "asound")]` → 사용자 시스템에 `libasound2` (apt) 또는 `alsa-lib` (pacman) 설치 필요.
- ALSA / CoreAudio 초기화 실패 시 `make_backend()` 가 stderr 에 경고 출력 후 `NullBackend` 로 fallback — 헤드리스 환경에서도 audio 스레드 panic 없음.
- CoreAudio (`backend::CoreAudioBackend`, cfg(target_os = "macos") only): AudioToolbox 의 AudioQueue API 사용. `#[link(name = "AudioToolbox", kind = "framework")]` — macOS 표준 framework 라 별도 설치 불필요. f32 LE stereo @48kHz 고정. 4 buffer × 1024 frame Condvar 동기화로 push-style trait 에 wrap.
- ALSA / CoreAudio 둘 다 Windows dev 머신에서 작성됨 → 각 OS 에서 실제 cargo build + audio_demo 검증은 향후 (P1 spec § 8.1, B3 spec § 6).
- vorbis는 src/vorbis/ 디렉토리로 분리됨 (bit_reader, codebook, floor, residue, headers, imdct, decoder)
- wasapi.rs는 521줄 (spec 만족, 분리 대상 아님)
- AudioSource 컴포넌트는 ECS와 함께 사용

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_audio --lib`
- 현재 테스트 개수: 89개 (Windows host 기준). B3 의 CoreAudio 테스트 +2 는 macOS host 에서만 컴파일/실행 (cfg(target_os = "macos")).
