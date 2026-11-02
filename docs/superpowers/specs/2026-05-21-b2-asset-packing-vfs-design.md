# Cycle 9 (B2) — Asset packing + VFS 설계

**작성일**: 2026-05-21
**상위 로드맵**: `docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md` § Cycle 9
**Sub-project**: B
**세션 수**: 2
**의존**: Cycle 8 (B1) — voltex.toml + build/run ✅

---

## 1. 목적

scene + scripts + assets 를 `.vpkg` 아카이브 1개로 묶고 wrapper 의 `include_bytes!` 로 exe 안에 임베드. 결과 exe 를 다른 폴더로 옮겨도 동작. `AssetLoader` 는 filesystem 경로와 임베디드 아카이브 양쪽을 단일 `Vfs` 추상 위에서 처리.

## 2. Scope

### 2.1 In-scope
- `.vpkg` 포맷 (magic 4B + version 4B + entry count 4B + index table + 연속 blob)
- `voltex_asset::vpkg` 모듈 — writer/reader, 외부 의존 0
- `voltex_asset::vfs::Vfs` enum — `Filesystem(PathBuf)` / `Archive(VpkgReader)`
- `Vfs::read(path) -> Result<Vec<u8>>` 단일 API. `AssetLoader` 가 이 위에서 동작하도록 리팩토링
- voltex_runtime: 임베디드 archive 가 빌드 타임 OUT_DIR 에 있으면 우선 사용, 없으면 fs fallback (개발 모드)
- voltex_cli build: 자원들을 `.vpkg` 으로 묶고 wrapper crate 가 `include_bytes!` 로 임베드 (build.rs 가 OUT_DIR 에 `.vpkg` 작성)
- ≥8 단위 테스트 + voltex_asset 기존 테스트 회귀 0

### 2.2 Out-of-scope
- 압축 (deflate / lz4) — `.vpkg` 는 raw concat 만. 압축은 v1.1+
- 암호화
- 증분 packing (모든 자원 변경 시 풀 재패킹)
- 자원 의존 그래프 추적

## 3. `.vpkg` 포맷

```
offset  size    field
0       4       magic = "VPKG"
4       4       version = u32 LE (= 1)
8       4       entry_count = u32 LE
12      ...     entries: [Entry; entry_count]
?       ...     blobs: concat of file bytes

Entry (per file):
  4   path_len (u32 LE)
  N   path bytes (UTF-8, '/' separator)
  4   blob_offset (u32 LE, from start of file)
  4   blob_size (u32 LE)
```

리더 (memory-only):
```rust
pub struct VpkgReader<'a> {
    bytes: &'a [u8],
    entries: Vec<(String, u32, u32)>, // (path, offset, size)
}
impl<'a> VpkgReader<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self>;
    pub fn list(&self) -> impl Iterator<Item = &str>;
    pub fn read(&self, path: &str) -> Option<&[u8]>;
}
```

라이터:
```rust
pub fn write_vpkg(entries: &[(String, Vec<u8>)]) -> Vec<u8>;
```

writer 는 단순 — 각 entry 의 blob 을 sequential 로 이어 붙이고 index 채움. 메모리 사용은 O(전체 자원 크기). v1 수용.

## 4. `Vfs` 추상

```rust
// voltex_asset::vfs
pub enum Vfs {
    Filesystem(PathBuf),                       // 베이스 디렉토리 — 모든 read 가 join 됨
    Archive(VpkgReaderOwned),                  // exe 안에 임베드된 아카이브
}

impl Vfs {
    pub fn fs(base: impl Into<PathBuf>) -> Self;
    pub fn archive_from_bytes(bytes: &'static [u8]) -> Result<Self>;
    pub fn read(&self, path: &str) -> Result<Vec<u8>>;
    pub fn exists(&self, path: &str) -> bool;
}
```

`VpkgReaderOwned` 는 `&'static [u8]` 을 보관 (include_bytes! 의 출력은 정적 lifetime).

### 4.1 AssetLoader 통합
현재 `voltex_asset::AssetLoader` 가 어떻게 동작하는지 — fs 직접 read 라면 `Vfs::Filesystem` 으로 래핑한 layer 로 옮긴다. **변경 폭 최소화**: 기존 함수 시그너처를 유지하고 내부에 Vfs 의존 추가. 외부 사용자 코드 (editor_demo, examples) 회귀 0.

만약 AssetLoader 의 변경이 너무 크면, 이 cycle 에서는 **Vfs 만 신규 추가** 하고 AssetLoader 리팩토링은 v1.1 로 미룬다 (spec out-of-scope 추가). 단 voltex_runtime 측에서는 새 Vfs API 를 사용해서 scene 로딩한다.

## 5. voltex_runtime 의 임베디드 archive 사용

```rust
// voltex_runtime::app::RuntimeApp::new
// pseudo-code:
let vfs = match include_archive_bytes() {
    Some(bytes) => Vfs::archive_from_bytes(bytes)?,
    None => Vfs::fs(default_assets_root()),
};
```

`include_archive_bytes()` 는 wrapper crate 가 작성한 `build.rs` 가 `OUT_DIR/voltex_archive.vpkg` 를 만들고 wrapper main.rs 가 `include_bytes!(concat!(env!("OUT_DIR"), "/voltex_archive.vpkg"))` 로 가져옴.

voltex_runtime 자체는 archive 가 있는지 모름 → 사용자 (wrapper 의 main) 가 결정. RuntimeConfig 에 `with_vfs(Vfs)` 빌더 메서드 추가.

## 6. voltex_cli build 의 packing step

```
build 흐름:
  parse voltex.toml
  collect files:
    scene.vscn (if exists)
    scripts/**/*.lua
    assets/**/*  (재귀)
  pack to `<dist>/wrapper/voltex_archive.vpkg`
  wrapper main.rs 가 with_vfs(Vfs::archive_from_bytes(include_bytes!("voltex_archive.vpkg")))
  cargo build --release
```

wrapper Cargo.toml 에 build.rs 추가 → OUT_DIR 에 archive 복사. 또는 `src/main.rs` 옆에 직접 두고 `include_bytes!("../voltex_archive.vpkg")`. 단순한 후자 채택.

## 7. 모듈 구조

```
crates/voltex_asset/src/
├── vpkg.rs        # 신규: 포맷 reader/writer
├── vfs.rs         # 신규: Vfs enum
└── lib.rs         # pub use

crates/voltex_runtime/src/
├── config.rs      # + with_vfs(Vfs)
├── app.rs         # RuntimeApp.vfs: Option<Vfs>; scene/asset 로딩 시 사용
└── lib.rs         # re-export Vfs (편의)

crates/voltex_cli/src/
├── build.rs       # + pack_archive(manifest, manifest_dir, wrapper_dir)
└── manifest.rs    # 변경 없음
```

## 8. 테스트 (≥8, 작성 11)

| # | 파일 | 케이스 |
|---|---|---|
| 1 | vpkg.rs | `roundtrip_single_file` |
| 2 | vpkg.rs | `roundtrip_multiple_files_preserves_order` |
| 3 | vpkg.rs | `parse_rejects_bad_magic` |
| 4 | vpkg.rs | `parse_rejects_truncated_index` |
| 5 | vpkg.rs | `empty_archive_has_zero_entries` |
| 6 | vfs.rs | `filesystem_read_returns_bytes` (tmp 파일 작성 후 read) |
| 7 | vfs.rs | `filesystem_missing_path_errors` |
| 8 | vfs.rs | `archive_read_returns_bytes` (in-memory vpkg → Vfs::archive_from_bytes) |
| 9 | vfs.rs | `archive_unknown_path_errors` |
| 10 | vfs.rs | `vfs_exists_matches_read` |
| 11 | build.rs (cli) | `pack_archive_collects_scripts_and_assets` (임시 디렉토리에서 voltex.toml + scripts/* + assets/* 만들고 archive 생성, VpkgReader 로 읽어 검증) |

voltex_asset 기존 24 테스트 회귀 0 확인.

## 9. 세션 분할

### Session 1: vpkg + Vfs + 테스트
- vpkg.rs (writer + reader + 검증)
- vfs.rs (enum + Filesystem + Archive)
- 테스트 #1~10
- voltex_asset/lib.rs re-export
- voltex_asset 의 기존 테스트 회귀 0

### Session 2: voltex_runtime 통합 + voltex_cli packing
- RuntimeConfig::with_vfs
- RuntimeApp 의 scene 로딩 시 Vfs 우선 사용 (없으면 기존 fs 경로)
- voltex_cli build 의 pack_archive
- wrapper main.rs codegen 에 `include_bytes!` + `archive_from_bytes` + `with_vfs` 코드 추가
- 테스트 #11
- CODEMAP / STATUS / roadmap

## 10. 위험 / 미지수

### 10.1 AssetLoader 리팩토링 범위
voltex_asset::AssetLoader 가 internal 로 fs 사용 → Vfs 로 swap 하면 호출 시그너처 어떻게 되는지 봐야 함. 손대지 않는 게 안전한 길. Vfs 는 RuntimeApp 의 scene 로딩 경로에만 적용. AssetLoader 리팩토링은 v1.1 명시.

### 10.2 `&'static` lifetime
`include_bytes!` 의 결과는 `&'static [u8]`. `Vfs::archive_from_bytes(bytes: &'static [u8])` 시그너처가 자연스럽다. 단위 테스트에서 동적 데이터는 `Box::leak` 으로 'static 화.

### 10.3 wrapper main.rs codegen 복잡도 증가
이번 cycle 부터 wrapper 의 main.rs 가 include_bytes + Vfs 까지 끌고 옴. codegen 로직이 60줄 가까이 될 수 있음 → render_wrapper_main 함수 폭 증가. 분리 필요 시 별도 helper.

### 10.4 archive 크기 한계
`include_bytes!` 가 매우 큰 데이터 (수십 MB+) 면 컴파일러가 느려짐. v1 의 예제 자원은 작아서 무관. 큰 자원은 v1.1 의 압축 / streaming 으로.

## 11. 변경 영향도

| Crate / Module | 변경 |
|---|---|
| voltex_asset::vpkg | 신규 모듈 |
| voltex_asset::vfs | 신규 모듈 |
| voltex_asset::lib | re-exports |
| voltex_runtime::config | + with_vfs |
| voltex_runtime::app | RuntimeApp 의 scene 로딩 경로에 Vfs 적용 |
| voltex_runtime::lib | re-export Vfs |
| voltex_cli::build | pack_archive 추가, wrapper main.rs 가 include_bytes 사용 |
| 다른 crate | 변경 없음 |
| docs/dev/STATUS.md, roadmap | Cycle 9 |

## 12. Done 기준 (로드맵 § Cycle 9)
- [ ] `voltex_asset::vpkg` + `vfs` 신규 모듈, 외부 의존 0
- [ ] vfs 단위 테스트 ≥8
- [ ] voltex_asset 기존 24 테스트 회귀 0
- [ ] B1 의 my_game.exe 가 단독 폴더로 옮겨도 동작 (수동 검증)
- [ ] roadmap Cycle 9 `[✅ 2/2]`
