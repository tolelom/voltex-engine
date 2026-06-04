# 런타임 에셋 경로 자동 resolve (설계)

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.96~97 (TexturePath/MeshPath 영속화) 의 비범위 후속

## 목표

에디터에서 텍스처/메시를 적용해 저장한 씬이 **게임 런타임에서도 그대로
보이게** 한다. 현재 RuntimeApp 은 TexturePath/MeshPath 를 로드만 하고
재해석하지 않아 에디터 밖에서는 흰 큐브가 된다.

## 유닛 1 — `voltex_runtime::asset_paths` (신규 모듈)

- `AssetPathCache` — `textures/meshes: HashMap<PathBuf, Option<Handle>>`.
  성공 핸들 공유(같은 파일 = 같은 핸들) + **실패 None 캐시**(매 프레임 fs
  재시도/로그 스팸 방지). `clear_failures()` — 씬 (재)로드 시 실패만 비워
  재시도 기회 부여.
- `resolve_asset_paths_with(world, cache, load_texture, load_mesh) -> (usize,
  usize)` — **로더 주입형 코어** (GPU 무관 → 헤드리스 단위 테스트 가능).
  TexturePath/MeshPath 가 있는데 핸들이 없는 엔티티만 대상 (있으면 skip —
  멱등, materialize_renderables 계약과 동일). Bounds 불변(씬이 복원).
- `resolve_asset_paths(world, assets, cache, device, queue)` — 실 로더
  (fs read → add_texture_png / Mesh::from_obj·from_gltf → add_mesh) 래퍼.

## 유닛 2 — 배선

- `RuntimeApp.asset_path_cache` 필드.
- `handler.rs` 의 materialize_renderables 2개 호출부 직후에 resolve 호출
  (resumed + per-frame redraw — 런타임 스폰 엔티티 커버, 멱등이라 저렴).
- `load_scene` / initial_scene 로드 직후 `clear_failures()`.

## 검증 (헤드리스 — 주입 로더)

- 경로 있는 엔티티가 fake 로더로 핸들 부착됨 / 핸들 보유 엔티티 skip(멱등)
  / 실패 None 캐시 후 로더 재호출 없음 / clear_failures 후 재시도 /
  같은 경로 2 엔티티 = 로더 1회 + 핸들 공유.
- build / clippy(`-D warnings`) / test 그린.

## 비범위 (후속)

- Vfs 경유 에셋 read(현재 fs 직접 — 씬/스크립트의 vfs_or_disk 패턴 적용은
  후속), `.vpkg` 번들에 에셋 동봉, 핫리로드 watch 자동 등록(에디터 전용 유지).

## CODEMAP 영향

- `voltex_runtime/CODEMAP.md` — asset_paths 모듈.
