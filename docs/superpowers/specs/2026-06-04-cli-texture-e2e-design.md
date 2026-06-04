# CLI 텍스처 번들 E2E 실검증 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.96~101 (TexturePath 영속화 → Vfs resolve → build 자동 동봉) — 체인의 끝단 실기기 검증

## 배경

v1.0.96~101 은 각 단계를 단위 테스트로 검증했지만, **전체 체인을 한 번에
실행한 적이 없다**: 에디터가 저장하는 형식의 씬(texture_path 컴포넌트)이
`voltex build` 를 통과해 `.vpkg` 에 동봉되고, 산출 exe 가 **루즈 파일이 전혀
없는 디렉토리**에서 텍스처를 vfs 로 resolve 하는지. v1.0.3/1.0.9 의 전례처럼
체인 통합 격차는 직접 흐름을 돌려야 드러난다.

## 검증 시나리오

1. `voltex new <scratch>` 스캐폴드.
2. `textures/checker.png` 생성 — manifest 의 선언 디렉토리(`assets/`) **밖**
   → v1.0.101 자동 동봉 경로를 통과해야만 vpkg 에 들어감.
3. `scene.vscn` 을 JSON 씬으로 교체: 엔티티 1개 = `transform`(identity) +
   `primitive_mesh`(cube) + `texture_path("textures/checker.png")`.
   컴포넌트 페이로드는 hex 인코딩 (직렬화 포맷 문서화된 와이어 그대로).
4. `voltex build --engine-path <repo>` — stderr 에
   `auto-bundled scene asset \`textures/checker.png\`` 확인.
5. `dist/<name>.exe` 를 **빈 디렉토리로 복사** 후 그 cwd 에서 실행.
   - 다른 cwd 라 `textures/checker.png` fs fallback 은 구조적으로 불가능 —
     `failed to resolve` 로그 **부재** = vfs(vpkg) 경유 resolve 의 증거.
   - 수 초 구동 후 종료(스모크) — crash/즉시 종료 없으면 통과.

## 산출물

- **수동 실검증 1회** (이 세션, RTX 4080 실기기) — 결과를 STATUS 에 기록.
- **`e2e_texture_bundle` ignored 테스트** (`crates/voltex_cli/tests/`) —
  기존 `e2e_new_build.rs` 패턴(#[ignore], CI 가 `--ignored` 로 명시 호출).
  씬 JSON 은 `voltex_ecs` 직렬화 + `register_raw` 마커로 생성 (build.rs 단위
  테스트의 `make_scene_json` 패턴), PNG 는 `voltex_image::encode_rgba8`
  (dev-dependency — workspace crate 라 외부 dep 0 정책 유지).
  exe 실행 스모크는 spawn → N초 대기 → kill, stderr 에 `failed to resolve`
  부재 단언.

## 비범위 (후속)

- 픽셀 단위 시각 검증 (voltex_capture 는 별도 하니스 — dist exe 는 실창).
- Audio 경로 영속화 E2E (ClipId 설계 후).
- 메시(.obj) 동봉 E2E — 텍스처와 동일 경로라 단위 테스트로 충분, 필요 시 확장.

## CODEMAP 영향

- `voltex_cli/CODEMAP.md` — e2e 테스트 항목에 텍스처 번들 시나리오 추가.
