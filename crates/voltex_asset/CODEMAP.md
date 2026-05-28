# voltex_asset

## 책임 (한 줄)
에셋 핸들 + 저장소 + 비동기 로더 + 핫 리로드 + `.vpkg` 아카이브 + Vfs 추상 (B2).

## 공개 API 진입점
- `Handle<T>` — generation 기반 에셋 핸들; `id()`, `generation()`, `from_raw_parts()` 공개 (src/handle.rs)
- `AssetStorage<T>` — 타입별 저장소 (ref counting) (src/storage.rs)
- `Assets` — type-erased 다중 저장소 (src/assets.rs)
- `AssetLoader` — 워커 스레드 비동기 로딩 (src/loader.rs)
- `FileWatcher` — mtime 폴링 변경 감지 (src/watcher.rs)
- `VpkgReader`, `write_vpkg`, `VpkgEntry`, `VPKG_MAGIC`, `VPKG_VERSION` — `.vpkg` 아카이브 (src/vpkg.rs)
- `Vfs`, `VpkgReaderOwned` — Filesystem / Archive 백엔드 통합 read API (src/vfs.rs)
- `AssetError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `handle` — Handle<T> 정의 (src/handle.rs)
- `storage` — 단일 타입 저장소 (src/storage.rs)
- `assets` — 다중 타입 저장소 (src/assets.rs)
- `loader` — 비동기 로더 (src/loader.rs)
- `watcher` — 파일 변경 감지 (src/watcher.rs)
- `vpkg` — `.vpkg` writer/reader (B2 신규)
- `vfs` — Vfs 추상 (Filesystem | Archive) (B2 신규)

## `.vpkg` 포맷
```
magic        b"VPKG"           (4)
version      u32 LE = 1         (4)
entry_count  u32 LE             (4)
entries:
  path_len   u32 LE
  path       path_len UTF-8 (forward slashes)
  offset     u32 LE
  size       u32 LE
blobs        concat
```
압축 / 암호화 없음 — v1 단순화. 압축은 v1.1+.

## 의존성
- voltex crate: 없음
- 외부 crate: thiserror (에러 타입)

## 변경 시 주의점
- Handle<T>의 generation 필드 시맨틱 변경 시 모든 자원 추적 깨짐
- replace_in_place는 핫 리로드용 — 호출 시점에 모든 핸들이 새 버전을 보게 됨
- `.vpkg` 포맷 변경 시 voltex_cli build 가 만든 모든 기존 산출물과 호환 깨짐 — `VPKG_VERSION` bump 필수
- `Vfs::archive_from_bytes` 는 `&'static [u8]` 만 받음 (include_bytes! 산출물 호환). 동적 데이터는 `Box::leak` 으로 'static 화.
- `Vfs::Filesystem(base)` 의 read 경로는 항상 `base.join(path)`. base 외부 경로 (e.g. "../") 도 fs 가 받으면 통과 — sandbox 없음. CLI 가 만든 자원 경로는 forward slash 사용 권장.

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_asset --lib`
- 현재 테스트 개수: 36개 (B2 +12: vpkg ×6 [roundtrip 2, bad magic, truncated, empty, unknown path], vfs ×6 [fs read/missing/list, archive read/unknown/exists-matches])
