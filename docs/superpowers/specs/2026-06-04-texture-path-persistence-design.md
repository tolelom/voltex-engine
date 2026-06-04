# 텍스처 에셋 경로 직렬화 (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.86 (브라우저→에디터 파이프라인)

## 문제 (데이터 손실)

브라우저 Open 으로 적용한 텍스처는 `TextureHandle` 만 부착된다. 핸들은 런타임
값(RenderAssets 슬롯)이라 씬에 직렬화돼도 다음 실행에서 무의미 — **Save →
Load 하면 텍스처가 사라진다**. 에셋 참조는 경로로 영속화해야 한다.

(MeshHandle/MaterialHandle 도 동일 계열 한계 — 이번 사이클은 텍스처가 첫
사례로 패턴을 세우고, 메시는 후속.)

## 유닛 1 — `voltex_renderer_ecs::TexturePath`

- `TexturePath(pub String)` 컴포넌트 — Open 한 텍스처의 **경로** (씬 영속).
- `impl voltex_ecs::Component` (NAME `"texture_path"`, UTF-8 bytes round-trip).
- `register_renderable_components` 에 등록 추가 (씬 직렬화 가능한 렌더 선언
  그룹과 동일 진입점).

## 유닛 2 — `editor_demo` 배선

- `session.registry.register::<TexturePath>()` (세션 생성 직후).
- 텍스처 적용 시 `TextureHandle` 과 함께 `TexturePath(경로)` 부착.
- **resolve 패스** `resolve_texture_paths(...)` (free fn): `TexturePath` 는
  있는데 핸들이 없거나 stale 인 엔티티마다 — 파일 read → `add_texture_png` →
  핸들 부착 + 핫리로드 watch 등록. **LoadScene 직후** (Open-scene 분기 + 씬
  핫리로드 분기) 호출, 상태 텍스트에 복원 수 표기. 실패는 로그 + skip
  (엔티티는 흰색 기본 텍스처로 렌더 — 기존 fallback).
- World::clear() 가 LoadScene 에서 모든 컴포넌트를 갈아끼우므로 resolve 는
  로드된 TexturePath 만 본다 — 이전 핸들 잔존 걱정 없음.

## 검증

- renderer_ecs 단위: TexturePath 가 registry JSON 직렬화 round-trip 됨
  (declarative round-trip 테스트 패턴).
- editor_demo build/clippy. 전체 build/clippy(-D warnings)/test 그린.

## 비범위 (후속)

- Mesh/Audio 경로 영속화(같은 패턴 반복), 런타임(RuntimeApp) 쪽 자동 resolve,
  상대경로/VFS 정규화(현재 Open 이 준 절대경로 그대로 — 프로젝트 이식성 한계
  문서화), 4-맵 PBR 경로.

## CODEMAP 영향

- `voltex_renderer_ecs/CODEMAP.md` — TexturePath.
