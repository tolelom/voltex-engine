# 에디터 씬 로드 시 AudioPath 프리뷰 복원 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.103 (AudioPath), v1.0.106 (출하 체인 완결). 비범위 후속.

## 문제

에디터에서 오디오를 적용해 저장한 씬을 다시 열면 `AudioPath` 는 로드되지만
에디터의 프리뷰 `AudioSystem` 에 클립이 없다 — 텍스처/메시는
`resolve_texture_paths`/`resolve_mesh_paths` 가 LoadScene 직후 복원하는데
오디오만 대칭이 빠져 있다. (에디터 registry 는 `audio_source` 를 직렬화하지
않으므로 로드 후 엔티티에는 AudioPath 만 있다.)

## 설계 (editor_demo 한정 — 기존 3종 대칭 완성)

- `resolve_audio_paths(world, audio, watcher, watched_audio) -> usize` —
  texture 버전 미러: `AudioPath` 보유 엔티티마다 경로별 1회
  `load_clip_from_path` → `audio.add_clip` (파일 공유 엔티티는 clip index
  공유, `watched_audio` 가 캐시 겸 핫리로드 맵) + watch 등록 + 엔티티에
  `AudioSource::new(idx)`(stopped — 프리뷰 재생은 브라우저 Open 액션 소관)
  부착. 실패는 로그 + skip.
- 배선: LoadScene 두 분기 (Open / 씬 핫리로드) 에서 texture/mesh 다음 호출.
  상태 텍스트 `({} textures, {} meshes, {} audio)`.
- 복원된 클립은 `watched_audio` 에 들어가므로 기존 핫리로드 poll 분기
  (변경 → `replace_clip`) 가 추가 작업 없이 적용된다.

## 검증

- editor_demo 빌드 + clippy(-D warnings) + workspace test 그린.
  (resolve 3종은 demo 통합 레이어 — 기존 texture/mesh 와 동일하게 단위
  테스트 비대상; 코어 로직은 voltex_runtime::audio_paths 가 이미 커버.)

## 비범위 (후속)

- 프리뷰 자동 재생, AudioSource 플래그(volume/looping) 에디터 편집 UI 와
  직렬화, 인스펙터 audio 요약.

## CODEMAP 영향

- 없음 (editor_demo 는 CODEMAP 비대상). STATUS 만 갱신.
