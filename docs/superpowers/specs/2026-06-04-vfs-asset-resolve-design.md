# Vfs 경유 에셋 resolve (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.98 (런타임 resolve) + v1.0.99 (상대경로) 의 비범위 후속

## 목표

런타임 에셋 resolve 가 씬/스크립트와 동일하게 **Vfs 우선 + fs fallback** 으로
읽게 한다 (`load_scene_via_vfs_or_disk` 규약). 이로써 `.vpkg` 아카이브에
에셋이 들어 있으면 디스크 없이도 동작 — v1.0.99 의 root-상대 forward-slash
형식이 그대로 아카이브 키.

## 유닛 — `voltex_runtime::asset_paths`

- `read_asset(vfs: Option<&Vfs>, path: &Path) -> Result<Vec<u8>, String>`
  (pub(crate)) — vfs 가 있고 `exists(키)` 면 vfs read, 아니면 fs read.
  키 = 경로의 forward-slash 문자열화 (Windows `\` 정규화).
- `resolve_asset_paths(..., vfs: Option<&Vfs>)` 시그니처 확장 — 텍스처/메시
  로더가 `read_asset` 사용. 메시 디코드를 bytes 기반으로 정리 (obj =
  UTF-8 검증 후 from_obj, gltf/glb = from_gltf).
- handler 호출부 2곳에 `app.config.vfs.as_ref()` 전달.

## 검증 (헤드리스)

- `read_asset`: Vfs::fs 루트에만 있는 파일 → vfs 로 읽힘 / vfs 에 없으면 fs
  fallback / 둘 다 없으면 Err. (+2~3)
- 코어(`resolve_asset_paths_with`)는 무변경 — 기존 테스트 유지.
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- `voltex_cli` 가 `.vpkg` 에 씬 참조 에셋을 자동 수집/동봉 (별도 사이클 —
  씬 파싱해 TexturePath/MeshPath 추출 필요).

## CODEMAP 영향

- `voltex_runtime/CODEMAP.md` — asset_paths 의 vfs 규약 갱신.
