# voltex_audio

## 책임 (한 줄)
오디오 — WAV/OGG 디코더, AudioBackend (WASAPI/ALSA/CoreAudio/Null), 3D 사운드(거리 감쇠 + 스테레오 팬), 믹스 그룹. 엔진이 쓰지 않는 로더·버스·이펙트·HRTF·차폐는 `experimental`.

## 공개 API 진입점
- `AudioSystem` — 메인 오디오 시스템. `add_clip(&mut, AudioClip) -> usize`(런타임 핫로드, FIFO 로 인덱스 lockstep) / `replace_clip(idx, AudioClip)`(핫리로드, out-of-range 는 무시) / `play`/`play_3d` → `VoiceId` 반환(AtomicU64 발급) + `stop_voice(VoiceId)` 로 해당 인스턴스만 정지 — clip-index `stop` 은 그 클립의 전 인스턴스 정지 (src/audio_system.rs)
- `VoiceId(u64)` — 재생 인스턴스(voice) 핸들, `VoiceId(0)` = 익명 기본값 (src/mixing.rs)
- `AudioClip` — PCM 데이터 (src/audio_clip.rs)
- `AudioSource` / `ClipId` — per-entity 재생 디스크립터 + 타입화된 clip 식별자. ECS 컴포넌트로 등록 가능하나 audio crate 자체는 ECS 무의존 (src/audio_source.rs)
- WAV/OGG 디코더 — `parse_wav`(PCM 8/16/24/32·float32·EXTENSIBLE, v1.0.144), `parse_ogg`(컨테이너) + `decode_vorbis`(코덱), 합본 진입점 `decode_ogg_vorbis_bytes` (src/wav.rs, src/ogg.rs, src/vorbis/)
- `load_clip_file(path) -> Result<AudioClip>` — 파일 read + `decode_clip_bytes` 원콜 디코드 (src/lib.rs; 에디터/핫리로드 공용)
- `decode_clip_bytes(path, bytes) -> Result<AudioClip>` — in-memory bytes 를 path 확장자(wav/ogg) 분기로 디코드. 경로는 확장자/에러 메시지에만 사용 — Vfs(.vpkg) 소스 지원 (v1.0.103) (src/lib.rs)
- `MixGroup`, `MixerState` — 그룹별 볼륨 믹싱 (src/mix_group.rs)
- spatial — `Listener`, `SpatialParams`, `distance_attenuation`, `stereo_pan`, `compute_spatial_gains` (src/spatial.rs)
- `AudioBackend` trait + impls (P1+B3): `WasapiBackend` (Windows), `AlsaBackend` (Linux), `CoreAudioBackend` (macOS, B3), `NullBackend` (헤드리스/CI), `make_backend()` (OS 자동 분기) (src/backend/). `AudioSystem` 오디오 스레드는 Linux·macOS 에서 `open_device_backend()` 로 이 trait 를 통해 재생하고, Windows 는 `wasapi_ffi::WasapiDevice` 를 직접 구동한다. 장치가 없으면(헤드리스/CI) 무음 모드
- `BackendError` — backend-level error type
- `AudioError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `audio_system` — 메인 시스템 (src/audio_system.rs)
- `audio_clip`, `audio_source` — 데이터 + 컴포넌트 (src/audio_clip.rs, src/audio_source.rs)
- `wav`, `ogg`, `vorbis` — 디코더 (src/wav.rs, src/ogg.rs, src/vorbis/)
- `mixing`, `mix_group` — 믹서 (src/mixing.rs, src/mix_group.rs)
- `spatial` — 3D 오디오 (src/spatial.rs)
- `wasapi_ffi` — Windows WASAPI COM FFI + `WasapiDevice` (src/wasapi_ffi.rs; 구 경로 `voltex_audio::wasapi` 는 re-export 로 유지)
- `backend` — `AudioBackend` trait + `null.rs` + `wasapi.rs` (windows) + `alsa.rs` (linux) + `coreaudio.rs` (macos, B3) + `make_backend` (P1+B3)
- `experimental` — 구현·단위 테스트는 되어 있지만 `AudioSystem` 믹서가 쓰지 않는 모듈. API가 바뀌거나 없어질 수 있다 (src/experimental/). 옛 경로(`voltex_audio::Reverb`, `voltex_audio::hrtf` 등)는 src/deprecated_paths.rs 의 deprecated 별칭
  - `async_loader` — `AsyncAudioLoader` + `AudioLoadState` (구 `LoadState`, deprecated alias — `voltex_asset::LoadState` 와 이름 충돌 회피). 비동기 "읽기 가능 여부" 확인만 (바이트는 버림, 클립 등록 안 함)
  - `dynamic_groups` — `MixGroupManager`, `MixGroupConfig` (런타임 생성 믹스 그룹 트리)
  - `audio_bus` — `AudioBus`, `BusInput` (gain 가중 입력 합산)
  - `effect_chain` — `AudioEffect` trait, `EffectChain`, `GainEffect`
  - `reverb` — `Reverb` (Schroeder), `Echo`, `DelayLine`
  - `fade_curves` — `FadeCurve`, `apply_fade`
  - `hrtf` — `calculate_hrtf`, `azimuth_from_positions`, `HrtfResult` (Woodworth ITD + cosine ILD)
  - `occlusion` — `calculate_occlusion`(차폐 계수 → 볼륨·컷오프), `OcclusionResult`, `LowPassFilter`

## 의존성
- voltex crate: voltex_math (3D 사운드 좌표)
- 외부 crate: thiserror (에러 타입)

## 변경 시 주의점
- 오디오 스레드가 클립 `Vec<AudioClip>` 를 **단독 소유(mut)** — `AddClip`/`ReplaceClip` 커맨드로만 변경. `AudioSystem.clip_count` 는 mpsc FIFO 덕에 스레드 쪽 Vec 길이와 lockstep
- 재생 중 클립 교체는 안전: 믹서가 out-of-range `clip_index` skip + `position >= frame_count` 시 자연 종료/루프 (mixing.rs 가드, 회귀 테스트 있음)
- WASAPI 는 Windows 전용 FFI — `backend::WasapiBackend` 가 trait impl 로 래핑
- ALSA 는 Linux 전용 FFI — `backend::AlsaBackend` (cfg(target_os = "linux") only). `#[link(name = "asound")]` → 사용자 시스템에 `libasound2` (apt) 또는 `alsa-lib` (pacman) 설치 필요.
- ALSA / CoreAudio 초기화 실패 시 `open_device_backend()` 가 stderr 에 경고를 출력하고 `None` → `AudioSystem` 은 무음 모드, `make_backend()` 는 `NullBackend` 로 fallback.
- CoreAudio (`backend::CoreAudioBackend`, cfg(target_os = "macos") only): AudioToolbox 의 AudioQueue API 사용. `#[link(name = "AudioToolbox", kind = "framework")]` — macOS 표준 framework 라 별도 설치 불필요. f32 LE stereo @48kHz 고정. 4 buffer × 1024 frame Condvar 동기화로 push-style trait 에 wrap.
- 명령 적용(`apply_command`)과 믹싱은 모든 OS 의 오디오 스레드가 한다(Windows: WASAPI 루프, Linux·macOS: `audio_thread_backend`). 장치가 없을 때의 무음 모드는 `AddClip`/`ReplaceClip`/`Shutdown` 만 처리한다. `audio_thread_backend` 는 백엔드의 블로킹 `write` 로 속도를 맞추므로 따로 sleep 하지 않는다. 비-Windows 쪽 컴파일 경고는 Windows 호스트에서 보이지 않으니 `scripts/check_linux_clippy.sh` 로 확인한다 (pre-push 가 target 설치 시 실행)
- ALSA / CoreAudio 둘 다 Windows dev 머신에서 작성됨 → 각 OS 에서 실제 cargo build + audio_demo 검증은 향후 (P1 spec § 8.1, B3 spec § 6).
- `parse_ogg_pages` 는 페이지마다 CRC 를 검증하고 불일치는 **에러** — 깨진 바이트를 Vorbis 디코더에 넘기면 손상 프레임이 무음으로 degrade 돼 "파일 손상" 이 "코덱 실패" 로 오진된다. CRC 는 OGG 전용 파라미터(poly 0x04c11db7, init 0, 무반사, xorout 없음)라 zlib/PNG CRC 와 다르다 — 실 libvorbis 파일 9 페이지로 검증, `crc32_matches_the_reference_vector` 가 상시 가드
- vorbis는 src/vorbis/ 디렉토리로 분리됨 (bit_reader, codebook, floor, residue, headers, imdct, decoder)
- `decode_vorbis` 는 개별 audio 패킷 디코드 실패를 **건너뛰지만**(손상된 한 프레임이 클립 전체를 날리지 않도록), 모든 audio 패킷이 실패하면 첫 에러를 `Err` 로 반환한다 — 빈 클립은 호출부에서 "의도된 무음" 과 구분이 안 되기 때문. 패킷 내부의 floor/residue 실패는 여전히 해당 채널 무음으로 degrade (`decoder.rs`)
- wasapi_ffi.rs는 ~550줄 (600줄 제한 이내, 분리 대상 아님)
- AudioSource 컴포넌트는 ECS와 함께 사용

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 통합 테스트: `tests/ogg_vorbis_decode.rs` — 코드로 생성한 실제 Ogg/Vorbis 파일(실 CRC + spec 형태 setup 헤더)을 `decode_ogg_vorbis_bytes` 로 끝까지 디코드하는 회귀 5개. 여기 fixture 를 고칠 땐 디코더에 맞추지 말고 spec 에 맞춰야 한다 (그래야 디코더 drift 를 잡는다)
- 실행: `cargo test -p voltex_audio --lib`
- 현재 테스트 개수: lib 138개 + 통합 5개 (Windows host 기준; VoiceId/stop_voice/apply_command +2, OGG CRC +4 포함. `apply_command` 테스트 1개는 Windows 전용이라 Linux host 는 1개 적다). B3 의 CoreAudio 테스트 +2 는 macOS host 에서만 컴파일/실행 (cfg(target_os = "macos")).
