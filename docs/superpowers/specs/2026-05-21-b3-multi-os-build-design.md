# Cycle 11 (B3) — macOS CoreAudio + 멀티 OS 네이티브 빌드 디스패치

**작성일**: 2026-05-21
**범위**: voltex_audio 에 CoreAudio backend 추가 + voltex_cli 의 `[build.<target>]` 디스패치 정책 정립 + 3 OS 빌드 매트릭스 문서화
**선행 조건**: P1 (`AudioBackend` trait + Wasapi/Alsa/Null impl), B1 (`voltex_cli` + `voltex.toml`), B2 (`.vpkg` archive + Vfs)
**로드맵 참조**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 11

---

## 1. 목표

Slice 2 "플랫폼 broaden" 의 마지막 cycle. 이 cycle 종료 시점에 외부 사용자가 **3 OS 모두 native 머신에서 `voltex build`** 로 단일 실행파일을 산출할 수 있어야 한다.

구체적 산출물:

1. `voltex_audio::backend::coreaudio::CoreAudioBackend` — macOS PCM 출력. AudioToolbox.framework 의 AudioQueue API 를 raw FFI 로 호출 (외부 crate 의존 0).
2. `voltex_cli::build::cmd` 의 target 검증 로직 — host_triple ↔ 사용자가 요청한 target 의 정합성 검사. 불일치 시 친화 에러로 즉시 거부 (cross-compile 안 함).
3. `docs/build-matrix.md` — 3 OS 각각의 사전 요구 + `voltex build --target <triple>` 명령 + 산출물 위치.

## 2. 비포함 (Out of Scope)

- code signing / notarization (macOS), Authenticode (Windows)
- installer 산출 (`.dmg`, `.msi`, `.deb`)
- cross-compile (요청 시 명시적 에러)
- macOS x86_64 (Intel) 별도 검증 — 코드 경로는 자동 지원하나 1차 검증 대상은 `macos-arm64`
- PipeWire / PulseAudio fallback (ALSA 가 default 디바이스 라우팅)
- audio backend 자동 hot-swap (디바이스 변경 시 재생성 요구)
- GitHub Actions 실제 CI 통합 (matrix yaml 스니펫만 doc 에 포함)

## 3. 설계

### 3.1 CoreAudio backend

#### 3.1.1 API 선택: AudioQueue

AudioQueue 는 push-based 인터페이스로, 현재 trait `AudioBackend::write(&mut self, samples: &[f32])` 와 자연스럽게 부합한다. AUHAL (AudioUnit) 은 pull-based 라 trait 자체를 리팩토링해야 하므로 P1 회귀 위험. AVAudioEngine 은 Objective-C 브리징이 필요해 "외부 crate 0" 정책 위반.

#### 3.1.2 포맷

ALSA backend 와 동일하게 고정:

- `mFormatID = kAudioFormatLinearPCM` (0x6c70636d, FourCC `'lpcm'`)
- `mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked` (0x09)
- `mSampleRate = 48000.0`
- `mChannelsPerFrame = 2`
- `mBitsPerChannel = 32`
- `mBytesPerFrame = 8` (2ch × 4byte)
- `mFramesPerPacket = 1`
- `mBytesPerPacket = 8`

#### 3.1.3 동기화 모델

AudioQueue 는 콜백 기반이지만 push 패턴으로 wrapping 한다:

1. 초기화 시 **N=4 개**의 `AudioQueueBuffer` 를 `AudioQueueAllocateBuffer` 로 미리 만들고, 모두 "free" 큐에 푸시.
2. `AudioQueueStart` 호출 직전, 무음 (0.0) 으로 채운 1 개를 enqueue 해 starter 로 사용 (안 그러면 첫 콜백이 안 옴).
3. **`write(&mut self, samples)`**:
   - free 큐에서 버퍼를 pop (없으면 Condvar 로 대기, 단 worst-case timeout 100ms → `BackendError::WriteFailed`).
   - `samples` 의 첫 `mAudioDataBytesCapacity / 4` (= frame × 2ch) 만큼만 복사. 남으면 caller 가 다음 호출에서 처리 (현 mixer 가 buffer_frames 단위로 호출하므로 정확히 맞을 것).
   - `mAudioDataByteSize` 설정 후 `AudioQueueEnqueueBuffer`.
   - 반환: 복사한 frame 수.
4. **콜백 (`output_callback`)**: 돌려받은 빈 버퍼를 free 큐에 다시 push + Condvar notify. 콜백 컨텍스트는 OS 스레드이므로 panic 금지 (catch_unwind 로 래핑).
5. **`shutdown`**: `AudioQueueStop(immediate=false)` → `AudioQueueDispose`. Drop 에서도 동일 호출.

`buffer_frames` = 1024 (ALSA 와 동일). `sample_rate=48000`, `channels=2`, 4 buffer × 1024 frames = 약 21 ms 라텐시 — 게임 오디오에 적합.

#### 3.1.4 FFI 시그니처

```rust
#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioQueueNewOutput(
        in_format: *const AudioStreamBasicDescription,
        in_callback: AudioQueueOutputCallback,
        in_user_data: *mut c_void,
        in_callback_run_loop: *mut c_void,        // CFRunLoopRef, NULL = internal thread
        in_callback_run_loop_mode: *mut c_void,   // CFStringRef, NULL = default
        in_flags: u32,
        out_aq: *mut AudioQueueRef,
    ) -> i32;
    fn AudioQueueAllocateBuffer(
        in_aq: AudioQueueRef,
        in_buffer_byte_size: u32,
        out_buffer: *mut AudioQueueBufferRef,
    ) -> i32;
    fn AudioQueueEnqueueBuffer(
        in_aq: AudioQueueRef,
        in_buffer: AudioQueueBufferRef,
        in_num_packet_descs: u32,
        in_packet_descs: *const c_void,
    ) -> i32;
    fn AudioQueueStart(in_aq: AudioQueueRef, in_start_time: *const c_void) -> i32;
    fn AudioQueueStop(in_aq: AudioQueueRef, in_immediate: u8) -> i32;
    fn AudioQueueDispose(in_aq: AudioQueueRef, in_immediate: u8) -> i32;
}

type AudioQueueRef = *mut c_void;
type AudioQueueBufferRef = *mut AudioQueueBuffer;
type AudioQueueOutputCallback = extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef);

#[repr(C)]
struct AudioQueueBuffer {
    audio_data_bytes_capacity: u32,
    audio_data: *mut c_void,
    audio_data_byte_size: u32,
    user_data: *mut c_void,
    packet_description_capacity: u32,
    packet_descriptions: *mut c_void,
    packet_description_count: u32,
}

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}
```

#### 3.1.5 모듈 배치 + 컴파일 분기

```
crates/voltex_audio/src/backend/
├── mod.rs        # 트레잇 + make_backend (분기 추가)
├── null.rs       # 기존
├── wasapi.rs     # 기존 (cfg windows)
├── alsa.rs       # 기존 (cfg linux)
└── coreaudio.rs  # 신규 (cfg macos) ← 본 cycle
```

`mod.rs` 의 `make_backend()` 에 cfg(macos) 분기 추가, init 실패 시 NullBackend fallback (ALSA 패턴 동일).

#### 3.1.6 안전성

- AudioQueue 핸들은 단일 backend instance 가 소유. Send 마커는 unsafe impl Send (ALSA 와 동일).
- 콜백은 자체 OS 스레드에서 실행 → 콜백 → Mutex / Condvar 통한 통신만 허용. Lua state 등 다른 모듈 접근 없음.
- AudioQueueBuffer 의 `mAudioData` 는 raw pointer. SAFETY 주석: 콜백이 우리에게 돌려준 직후 → enqueue 직전 사이에만 접근, capacity 내 byte 만 write.

### 3.2 CLI multi-target 디스패치

#### 3.2.1 정책: native-only

`voltex_cli::build::cmd` 의 시작부에 다음 검증 추가:

```
let host = host_triple();   // "windows-x64" | "linux-x64" | "macos-arm64" | "macos-x64"
let requested = target_arg.or(manifest.targets.first().cloned());

match requested {
    Some(t) if t == host => /* OK, proceed */,
    Some(t) /* mismatch */ => return Err(CrossCompileNotSupported {
        host: host.into(),
        requested: t,
        manifest_targets: manifest.targets.clone(),
    }),
    None => /* manifest has no targets[] field at all — fall back to host */,
}

// Manifest 가 host 를 안 포함하면 명시적 에러
if !manifest.build.targets.is_empty() && !manifest.build.targets.contains(&host) {
    return Err(HostNotInManifestTargets { host, listed: manifest.build.targets });
}
```

#### 3.2.2 새 에러 variant

`voltex_cli::error::CliError`:

```rust
CrossCompileNotSupported { host: String, requested: String },
HostNotInManifestTargets { host: String, listed: Vec<String> },
```

Display:

```
voltex: cross-compile not supported.
  current host: macos-arm64
  requested target: windows-x64
  hint: run `voltex build --target windows-x64` on a Windows machine.

voltex: manifest targets do not include host triple.
  current host: linux-x64
  manifest targets = ["windows-x64", "macos-arm64"]
  hint: add "linux-x64" to the targets array, or pass --target linux-x64 explicitly.
```

#### 3.2.3 `[build.<target>]` icon 활용 (deferred)

본 cycle 의 정식 scope 는 검증/디스패치까지. 실제 icon 임베드는 v1.1 후속 (Mac 은 `.icns`, Windows 는 `.rc` + `winres` 등 OS 별 메커니즘 상이). 매니페스트 파서는 이미 `target_overrides` 를 읽으므로 데이터는 보존됨.

### 3.3 docs/build-matrix.md

한 페이지짜리. 구성:

```
# Voltex Build Matrix

## 사전 요구
| OS | Rust | 추가 의존 |
|---|---|---|
| Windows 10/11 x64 | stable | Visual Studio 2022 (MSVC linker) |
| Ubuntu 22.04+ x64 | stable | `libasound2-dev` |
| macOS 13+ arm64 | stable | Xcode Command Line Tools |

## 빌드 명령 (각 native OS 에서 실행)
- Windows: `voltex build --target windows-x64`
- Linux:   `voltex build --target linux-x64`
- macOS:   `voltex build --target macos-arm64`

산출물: `dist/<project_name>(.exe)` + 인접 폴더 자원 (B2 에서는 archive 임베드).

## (옵션) GitHub Actions 3-OS matrix
[yaml 스니펫, runs-on: [windows-latest, ubuntu-latest, macos-latest]]
```

## 4. 파일 변경 / 신규

| 파일 | 변경 |
|---|---|
| `crates/voltex_audio/src/backend/coreaudio.rs` | 신규 |
| `crates/voltex_audio/src/backend/mod.rs` | cfg(macos) 분기 추가 |
| `crates/voltex_audio/CODEMAP.md` | backend/coreaudio 라인 추가 |
| `crates/voltex_cli/src/error.rs` | 2 variants 추가 |
| `crates/voltex_cli/src/build.rs` | host_triple 검증 로직 + 단위 테스트 +2 |
| `docs/build-matrix.md` | 신규 |
| `docs/STATUS.md` | Cycle 11 entry 추가 |
| `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` | Cycle 11 마커 `[ ]` → `[✅]` |

## 5. 테스트 (+3 최소)

| # | 위치 | 시나리오 |
|---|---|---|
| 1 | `voltex_audio::backend::coreaudio::tests` (cfg macos 전용, 빈 placeholder 한 개라도 둠 — 호스트 미스매치 빌드에서 회귀 방지) | macOS 머신 외에서는 skip. macOS 에서: `CoreAudioBackend::new()` 가 init 성공 또는 NotSupported 반환 — 패닉 없음 |
| 2 | `voltex_cli::build::tests::rejects_cross_compile` | `--target windows-x64` 가 host=linux-x64 환경에서 `CrossCompileNotSupported` 반환 |
| 3 | `voltex_cli::build::tests::accepts_host_target_from_manifest` | manifest targets=[host] 일 때 검증 통과 (실제 cargo build 까지는 안 가고, 검증 helper 함수 분리해서 그 함수만 테스트) |
| 4 | `voltex_cli::build::tests::rejects_when_manifest_excludes_host` | manifest targets=[다른 OS] 일 때 `HostNotInManifestTargets` |

검증 helper 분리: `fn validate_target(manifest: &Manifest, target_arg: Option<&str>) -> Result<String>` — 검증 로직만 떼어내서 단위 테스트 가능하게.

## 6. Done 기준 + 검증 한계

### 충족 가능 (이번 cycle 종료 시)

- [x] `cargo build --workspace` Windows 환경에서 깨끗 (CoreAudio 는 cfg-skip 이라 컴파일 안 됨)
- [x] `cargo test --workspace --lib` 통과 (CoreAudio 모듈 외)
- [x] CLI 가 cross-target 요청 시 친화 에러
- [x] `docs/build-matrix.md` 존재
- [x] +3 단위 테스트 (CLI 2~3 + CoreAudio placeholder 1)
- [x] STATUS / roadmap 업데이트

### 검증 한계 (사용자 후속)

- macOS 실제 머신에서 `cargo build --target aarch64-apple-darwin` + `audio_demo` 가 실제 소리 출력하는지 확인 → 본 spec § 6.1 에 명시. ALSA backend 도 동일 상황 (Windows 머신에서 작성, Linux 실 검증은 후속) — 일관된 처리.
- Linux 실제 머신에서 audio_demo 소리 출력 (P1 의 후속 항목 그대로 유지)

이 두 후속 검증은 Cycle 11 의 spec done 기준에는 포함되지만, "코드 작성 + 단위 테스트 통과" 는 Windows 호스트에서 완료 가능.

## 7. 세션 분할

| 세션 | 작업 |
|---|---|
| **1** | `coreaudio.rs` (FFI + AudioQueue impl + Condvar 동기화) + `backend/mod.rs` cfg 분기 + null fallback 통합 + macOS cfg 전용 단위 테스트 placeholder |
| **2** | `voltex_cli::error` + `voltex_cli::build::validate_target` 분리 + 친화 에러 메시지 + 단위 테스트 ×3 |
| **3** | `docs/build-matrix.md` + `CODEMAP.md` 갱신 + `STATUS.md` Cycle 11 entry + roadmap 마커 `[✅]` |

세션 1~2 는 commit 단위로 분리. 세션 3 은 docs 한 묶음.

## 8. 리스크

| # | 위험 | 완화 |
|---|---|---|
| 1 | CoreAudio AudioQueue 의 콜백이 OS 스레드 → 패닉 시 정의되지 않은 동작 | `std::panic::catch_unwind` 로 콜백 본체 래핑, 패닉은 stderr 만 출력하고 콜백 정상 종료 |
| 2 | `#[link(name = "AudioToolbox", kind = "framework")]` 가 Windows/Linux 호스트에서 컴파일 실패 | 모듈 전체 `#![cfg(target_os = "macos")]` 로 차폐 |
| 3 | host_triple = "macos-x64" (Intel Mac) 인 사용자가 manifest targets=["macos-arm64"] 만 적은 경우 → reject. 실제 사용자 운영체제와 manifest 의 미세한 차이 | `[build.macos]` 같은 OS-only 약식 매칭을 v1.1 후속으로 명시 |
| 4 | AudioQueueEnqueueBuffer 가 빠르게 호출되면 free 큐가 비고 caller 가 무한 대기 | 100ms timeout → `BackendError::WriteFailed`. mixer 가 fallback 처리 (null backend 동작과 비슷) |
| 5 | CoreAudio 검증을 Windows 머신에서 못 함 → 사용자 Mac 에서 첫 실행 시 처음 발견되는 버그 | spec § 6.1 에 후속 검증 항목으로 명시. ALSA 도 동일 처지. 코드 리뷰 + 단위 테스트로 명백한 결함만 잡고 진행 |

## 9. 관련 문서

- `docs/superpowers/specs/2026-05-21-p1-audio-backend-design.md` — P1 의 trait + Wasapi/Alsa/Null 패턴
- `docs/superpowers/specs/2026-05-21-b1-cli-manifest-design.md` — B1 의 manifest + build.rs 구조
- `docs/superpowers/specs/2026-05-21-b2-asset-packing-vfs-design.md` — B2 의 wrapper 생성 패턴 (target 검증 위치)
- `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 11
