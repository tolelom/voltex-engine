# voltex_asset

## 책임 (한 줄)
에셋 핸들 + 저장소 + 비동기 로더 + 핫 리로드.

## 공개 API 진입점
- `Handle<T>` — generation 기반 에셋 핸들 (src/handle.rs)
- `AssetStorage<T>` — 타입별 저장소 (ref counting) (src/storage.rs)
- `Assets` — type-erased 다중 저장소 (src/assets.rs)
- `AssetLoader` — 워커 스레드 비동기 로딩 (src/loader.rs)
- `FileWatcher` — mtime 폴링 변경 감지 (src/watcher.rs)
- `AssetError`, `Result` — thiserror 기반 에러 (Generic/Io) (src/error.rs)

## 모듈 구조
- `handle` — Handle<T> 정의 (src/handle.rs)
- `storage` — 단일 타입 저장소 (src/storage.rs)
- `assets` — 다중 타입 저장소 (src/assets.rs)
- `loader` — 비동기 로더 (src/loader.rs)
- `watcher` — 파일 변경 감지 (src/watcher.rs)

## 의존성
- voltex crate: 없음
- 외부 crate: 없음 (std만 사용)

## 변경 시 주의점
- Handle<T>의 generation 필드 시맨틱 변경 시 모든 자원 추적 깨짐
- replace_in_place는 핫 리로드용 — 호출 시점에 모든 핸들이 새 버전을 보게 됨

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_asset --lib`
- 현재 테스트 개수: 22개
