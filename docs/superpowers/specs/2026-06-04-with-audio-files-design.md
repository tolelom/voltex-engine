# RuntimeConfig::with_audio_files (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: `2026-06-04-runtime-audio-hot-reload-design.md` (v1.0.90) 의 비범위 후속

## 목표

v1.0.90 은 게임이 클립을 직접 디코드하고 `watch_audio_clip` 을 수동 호출해야
했다. `with_audio_files(paths)` 하나로 **디코드 + ClipId 부여 + 핫리로드 감시**
가 자동이 되게 한다 — 런타임 오디오 스토리의 마지막 조각.

## 유닛 — `voltex_runtime`

- `RuntimeConfig.audio_files: Vec<PathBuf>` + `with_audio_files(paths)`
  (`enable_audio = true`).
- `RuntimeApp::new`: `audio_clips` 뒤에 파일들을 **순서대로** 디코드해 append —
  ClipId = `with_audio` 클립 수 + 파일 순번 (안정 보장).
  - 디코드 실패: `log_error` + **무음 placeholder 클립**(1 샘플) push —
    인덱스 안정성이 우선 (실패 파일 때문에 뒤 파일들의 ClipId 가 밀리면 안 됨).
    파일이 핫리로드로 고쳐지면 같은 인덱스로 살아남.
  - 성공/실패 무관 모든 경로를 `watch_audio_clip` 자동 등록 (struct 구성 후).

## 검증 (헤드리스, 기존 RuntimeApp::new 테스트 패턴)

- temp wav → `with_audio_files` → audio Some + watched 1건 + 인덱스 0.
- `with_audio(1 clip)` + 파일 1개 → 파일 인덱스 1 (이어붙음).
- 손상 파일 → `new()` 성공 + placeholder + 여전히 감시됨.
- config: `with_audio_files` 가 enable_audio 켬.
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- Vfs 경유 오디오 로드(현재 fs 직접), 씬 파일에 오디오 매니페스트 직렬화,
  비동기 디코드(시작 시 동기 — 클립 수 소량 가정).

## CODEMAP 영향

- `voltex_runtime/CODEMAP.md` — `with_audio_files` 반영.
