# 메시 에셋 경로 직렬화 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: `2026-06-04-texture-path-persistence-design.md` (v1.0.96) 패턴 반복

## 목표

v1.0.96 이 텍스처에 세운 경로-영속화 패턴을 메시에 적용. Open 으로 적용한
메시(`MeshHandle`)도 런타임 슬롯이라 Save → Load 에서 사라진다 — `MeshPath`
로 경로를 영속하고 로드 후 재해석한다.

## 유닛 1 — `voltex_renderer_ecs::MeshPath`

- `MeshPath(pub String)` — `TexturePath` 와 동일 형태 (NAME `"mesh_path"`,
  UTF-8 round-trip, invalid UTF-8 거부). `register_renderable_components`
  등록 + re-export. (두 impl 은 의도적 평문 중복 — 3번째 사례(Audio)가 오면
  매크로 추출 검토.)

## 유닛 2 — `editor_demo`

- 세션 registry 등록, Mesh Open 적용 시 핸들/Bounds 와 함께 `MeshPath` 부착.
- `resolve_mesh_paths` — `resolve_texture_paths` 와 동일 구조: `MeshPath`
  엔티티마다 경로별 1회 `load_mesh_from_path` → `add_mesh` → 핸들 재부착 +
  `watched_meshes` watch. 같은 파일 공유 엔티티는 핸들 공유. **Bounds 는
  씬에 이미 직렬화돼 복원되므로 resolve 는 건드리지 않음** (파일이 저장 후
  바뀌었으면 stale — 핫리로드 v1 한계와 동일, 문서화).
- LoadScene 직후 두 분기(Open + 씬 핫리로드)에서 텍스처 resolve 와 나란히 호출.

## 검증

- renderer_ecs 단위 +2 (씬 JSON round-trip / invalid UTF-8 — 텍스처 테스트 대칭).
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- Audio 경로 영속화(AudioSource 는 ClipId 기반 — 별도 설계 필요),
  런타임 자동 resolve, 상대경로/VFS 정규화.

## CODEMAP 영향

- `voltex_renderer_ecs/CODEMAP.md` — MeshPath.
