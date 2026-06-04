# CLI 번들 E2E 오디오 확장 + wrapper 오디오 활성화 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.102 (텍스처 E2E), v1.0.103 (AudioPath 체인). v1.0.103 비범위 후속.

## 발견된 격차 — dist exe 에 오디오가 없다

`RuntimeConfig.enable_audio` 기본값은 `false` 이고 `render_wrapper_main`
코드젠은 audio 를 켜지 않는다. **AudioPath resolve 는 tick 의 audio 단계
안에 있으므로, 출하된 exe 에서는 v1.0.103 체인 전체가 실행되지 않는다** —
에디터에서 오디오를 적용해 저장한 씬이 빌드된 게임에서 무음.

### 유닛 1 — wrapper 가 항상 오디오 활성화

`render_wrapper_main` 에 `.with_audio(Vec::new())` 추가. 근거:
- 게임 런타임 출하물은 오디오 믹서 상시 초기화가 표준 (스크립트/씬 어느
  쪽이 소리를 내든 동작해야 — 조건부(씬 스캔 기반) 활성화는 스크립트
  spawn 오디오를 놓침).
- 비용은 스레드 1 + 백엔드 세션. 디바이스 실패는 기존대로 log + None
  (graceful degrade), 빈 클립 테이블 시작은 resolve 의 `add_clip` 이 채움.
- 코드젠 단위 테스트에 `with_audio` 단언 추가.

### 유닛 2 — E2E 오디오 단언 (`e2e_texture_bundle.rs` 확장)

- 씬 엔티티에 `audio_path`("sounds/beep.wav") 추가 — 선언 `assets/` **밖**
  (자동 동봉 경로). wav 는 `voltex_audio::generate_wav_bytes` (dev-dep —
  workspace crate, 외부 dep 0 유지).
- 단언 추가: 빌드 stderr 에 `sounds/beep.wav` 자동 동봉 로그 / `.vpkg` 에
  키 존재 / foreign cwd 런 로그에 `resolved 1 audio path(s)` (tick 첫
  프레임 — GPU 불필요라 텍스처보다 빨리 나옴) + 기존 에러-부재 단언 유지.
- 테스트/파일명은 texture+audio 겸용이 됨 — 파일명 유지 (헤더 doc 갱신).

## 검증

- 코드젠 단위 + E2E 1회 완주 (RTX 4080). build/clippy(-D warnings)/test 그린.

## 비범위 (후속)

- manifest `[build] audio = false` opt-out, 에디터 씬 로드 시 AudioPath
  프리뷰 복원, 메시 동봉 E2E.

## CODEMAP 영향

- voltex_cli CODEMAP — wrapper 코드젠 audio 활성화 + E2E 시나리오 갱신.
