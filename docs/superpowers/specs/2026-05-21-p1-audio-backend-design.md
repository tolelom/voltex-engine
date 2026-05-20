# Cycle 10 (P1) — AudioBackend trait + ALSA 설계

**작성일**: 2026-05-21
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 10
**Sub-project**: P
**세션 수**: 2
**의존**: 없음 (병렬 가능)

---

## 1. 목적

`voltex_audio` 의 Windows-specific 출력 경로를 `AudioBackend` trait 위로 옮긴다. WASAPI 를 trait impl 로 리팩토링하고, Linux 용 ALSA impl 과 헤드리스용 None 백엔드를 추가. CI/Linux 머신에서 빌드 + run 가능.

## 2. Scope

### 2.1 In-scope
- `voltex_audio::backend::AudioBackend` trait: `new`, `sample_rate`, `channels`, `buffer_frames`, `write(samples)`, `shutdown`
- `WasapiBackend` — 기존 WASAPI 코드 trait impl 로 정리
- `AlsaBackend` (Linux) — `libasound` FFI 로 PCM 출력 (f32 stereo @48kHz 고정)
- `NullBackend` — no-op; 헤드리스 테스트 / CI 환경용
- AudioSystem 의 `audio_thread_*` 가 cfg(target_os) 분기 후 backend new → loop write 패턴으로 통일
- 단위 테스트 ≥4 (trait 인터페이스 / NullBackend 동작 / cfg-gated 컴파일 검증)

### 2.2 Out-of-scope
- macOS CoreAudio (Cycle 11 B3)
- PipeWire / PulseAudio fallback
- 백엔드별 latency 튜닝
- 다중 출력 장치 선택 UI
- ALSA 의 실제 Linux 환경 verification (이 PR 은 Windows 에서 작성됨 — Linux 머신에서 audio_demo run 은 후속 검증)

## 3. Trait 정의

```rust
// crates/voltex_audio/src/backend/mod.rs
pub trait AudioBackend: Send {
    /// Sample rate the backend actually configured (e.g. 48000).
    fn sample_rate(&self) -> u32;
    /// Channel count (1 = mono, 2 = stereo).
    fn channels(&self) -> u16;
    /// Maximum frames per write call.
    fn buffer_frames(&self) -> u32;
    /// Write interleaved f32 samples (length = frames * channels).
    /// Returns the number of frames written or an error.
    fn write(&mut self, samples: &[f32]) -> Result<usize, BackendError>;
    /// Best-effort shutdown. Idempotent.
    fn shutdown(&mut self);
}

#[derive(Debug)]
pub enum BackendError {
    InitFailed(String),
    WriteFailed(String),
    NotSupported,
}
```

## 4. Backend impls

### 4.1 NullBackend
- 항상 성공, write 는 silent drop. sample_rate = 48000, channels = 2, buffer_frames = 1024. CI / 헤드리스용.

### 4.2 WasapiBackend (Windows)
- 기존 `WasapiDevice` 코드 정리해서 trait impl. cfg(target_os = "windows") only.
- new() → IMMDeviceEnumerator → GetDefaultAudioEndpoint → IAudioClient init → IAudioRenderClient
- write(samples) → GetBuffer/ReleaseBuffer
- shutdown() → Stop + Release COM
- 기존 audio_thread_windows 의 로직을 trait 호출로 단순화.

### 4.3 AlsaBackend (Linux)
- cfg(target_os = "linux") only. 외부 crate 의존 0 — `extern "C"` 로 libasound 직접 link.
- 1차 타겟 포맷: f32 stereo @48kHz, plug device "default".
- new():
  - `snd_pcm_open("default", SND_PCM_STREAM_PLAYBACK, 0)`
  - `snd_pcm_set_params(..., SND_PCM_FORMAT_FLOAT_LE, SND_PCM_ACCESS_RW_INTERLEAVED, 2, 48000, 1 /* soft_resample */, 100_000 /* 100ms latency */)`
- write():
  - `snd_pcm_writei(handle, samples.as_ptr(), frames)`
  - `EPIPE` (underrun) 시 `snd_pcm_prepare` 후 재시도
- shutdown(): `snd_pcm_drop` + `snd_pcm_close`

## 5. cfg 분기 정책

```rust
// audio_system.rs
fn make_backend() -> Box<dyn AudioBackend> {
    #[cfg(target_os = "windows")]
    {
        match WasapiBackend::new() {
            Ok(b) => return Box::new(b),
            Err(e) => eprintln!("[voltex_audio] WASAPI init failed: {}", e),
        }
    }
    #[cfg(target_os = "linux")]
    {
        match AlsaBackend::new() {
            Ok(b) => return Box::new(b),
            Err(e) => eprintln!("[voltex_audio] ALSA init failed: {}", e),
        }
    }
    Box::new(NullBackend::new())
}
```

오디오 스레드는 backend trait 만 알게 된다.

## 6. 모듈 구조

```
crates/voltex_audio/src/
├── backend/
│   ├── mod.rs       # trait + BackendError + make_backend
│   ├── null.rs      # NullBackend
│   ├── wasapi.rs    # WasapiBackend (windows only) — wraps existing src/wasapi.rs
│   └── alsa.rs      # AlsaBackend + libasound FFI (linux only)
├── audio_system.rs  # use backend::make_backend
└── wasapi.rs        # FFI 변경 없음 (low-level)
```

기존 `src/wasapi.rs` 는 FFI 바인딩 유지. `backend/wasapi.rs` 가 그 위에서 trait impl.

## 7. 테스트 (≥4, 작성 6)

| # | 파일 | 케이스 |
|---|---|---|
| 1 | backend/mod.rs | `null_backend_reports_sane_defaults` (48000/2/1024) |
| 2 | backend/null.rs | `null_backend_write_succeeds_and_returns_frames` |
| 3 | backend/null.rs | `shutdown_is_idempotent` (두 번 호출 panic 없음) |
| 4 | backend/mod.rs | `backend_error_display_includes_message` |
| 5 | backend/mod.rs | `make_backend_returns_some_backend` (smoke — cfg 분기 적어도 1개 백엔드 반환) |
| 6 | backend/alsa.rs | `alsa_module_compiles_on_linux` — `#[cfg(target_os = "linux")] #[test]` (Windows 빌드에서는 skip, Linux CI 에서만 동작) |

기존 voltex_audio 84 테스트 회귀 0.

## 8. 위험 / 미지수

### 8.1 ALSA FFI 검증 부재
이 PR 은 Windows 에서 작성됨. ALSA backend 는 컴파일은 cfg-gated 라서 Windows 에선 compile 되지 않고, Linux 환경 없이 syntax/link 검증 불가. 후속 작업으로 Linux 또는 WSL2 에서 `cargo build --target x86_64-unknown-linux-gnu` + audio_demo run 검증 필요.

### 8.2 sample_rate 협상
ALSA `snd_pcm_set_params` 의 `soft_resample = 1` 로 48000 강제. 디바이스가 다른 rate 만 지원하면 ALSA 가 in-driver resample. f32 출력은 모든 modern Linux 드라이버 지원.

### 8.3 Linux libasound link
`#[link(name = "asound")]` 로 동적 link. 사용자 시스템에 `libasound2` (apt) 또는 `alsa-lib` (pacman) 설치 필요. README 에 명시.

### 8.4 trait object 비용
Box<dyn AudioBackend> 의 dynamic dispatch. 오디오 write 가 frame 단위 (수십~수백 hz) 라서 무시 가능.

## 9. 세션 분할

### Session 1: AudioBackend trait + NullBackend + WasapiBackend refactor
- backend/{mod.rs, null.rs, wasapi.rs}
- audio_system.rs 의 audio_thread_windows 를 backend trait 호출로 정리
- 단위 테스트 #1~5
- Windows 빌드 + 테스트 깨끗

### Session 2: AlsaBackend (Linux) + 통합 + 문서화
- backend/alsa.rs + libasound FFI
- make_backend 의 cfg(target_os = "linux") 분기 활성
- 단위 테스트 #6
- README / CODEMAP 갱신, Linux 검증 절차 명시
- Linux 환경 부재로 실제 audio_demo run 검증은 향후

## 10. 변경 영향도
| Crate / Module | 변경 |
|---|---|
| voltex_audio::backend (신규 디렉토리) | trait + NullBackend + WasapiBackend + AlsaBackend |
| voltex_audio::audio_system | make_backend 호출, 스레드 단순화 |
| voltex_audio::lib | re-export backend trait + impls |
| voltex_audio::wasapi (기존) | FFI 변경 없음, alias 유지 |
| 다른 crate | 변경 없음 |

## 11. Done 기준 (로드맵 § Cycle 10)
- [ ] `AudioBackend` trait + NullBackend + WasapiBackend
- [ ] AlsaBackend code 작성 (Linux 검증은 향후)
- [ ] make_backend cfg-gated 분기
- [ ] 단위 테스트 +4 이상
- [ ] voltex_audio 기존 84 회귀 0
- [ ] CODEMAP / roadmap 갱신
- [ ] **추적용 후속**: Linux 또는 WSL2 에서 cargo build + audio_demo run + 실제 소리 출력 검증 (이 PR 의 Done 아님, Cycle 11 또는 별 cycle 의 시작점)
