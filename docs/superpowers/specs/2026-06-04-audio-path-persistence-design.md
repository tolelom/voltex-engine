# Audio 경로 영속화 (AudioPath) 설계

- 날짜: 2026-06-04
- 상태: 승인됨 (자율 구현 진행)
- 선행: v1.0.96~102 (TexturePath/MeshPath 영속화 체인 + E2E). 핸드오프 우선순위 2.

## 문제

`AudioSource` 는 `clip_id`(u32, AudioSystem 클립 테이블 인덱스)를 직렬화하지만,
그 인덱스가 **어떤 파일**인지는 어디에도 영속화되지 않는다. 에디터에서 부여된
clip_id 는 그 런(run)의 `add_clip` 호출 순서일 뿐 — 다른 세션/런타임에서는
무의미하다. TextureHandle 이 런타임 슬롯이라 TexturePath 를 만든 것과 동형의
문제이며, 동형의 해법을 쓴다.

## 설계

### 유닛 0 — `voltex_ecs::path_component!` 매크로 (3번째 사례 도래 — 메모된 추출)

TexturePath/MeshPath 의 `Component` impl 은 "NAME + UTF-8 round-trip" 25줄
복제다 (renderer_ecs CODEMAP: "3번째 경로 컴포넌트가 오면 매크로 추출 검토").
AudioPath 가 3번째 — `#[macro_export] macro_rules! path_component` 를
voltex_ecs 에 추가한다. **기존 tuple struct 에 impl 만 생성** (struct 정의와
doc 은 손으로 — 타입별 문서가 다르므로):

```rust
voltex_ecs::path_component!(TexturePath, "texture_path");
```

renderer_ecs 의 두 컴포넌트를 매크로로 마이그레이션 (round-trip 테스트
불변 = 동작 보존 검증), AudioPath 가 신규 사용.

### 유닛 1 — `voltex_audio::decode_clip_bytes(path, bytes)`

`load_clip_file` 의 확장자 분기 디코드를 bytes 입력으로 추출 —
`load_clip_file` = `fs::read` + `decode_clip_bytes` (DRY). Vfs 소스(.vpkg)
디코드의 전제.

### 유닛 2 — `voltex_runtime::audio_paths` (asset_paths 미러)

- `AudioPath(pub String)` — NAME `audio_path`, 매크로 impl. 앱 registry 에
  `register_audio_source` 옆에서 등록.
- `AudioClipCache` — `HashMap<PathBuf, Option<u32>>` (성공 = 공유 ClipId,
  실패 = None 캐시) + `clear_failures` (load_scene 시 재시도).
- `resolve_audio_paths_with(world, cache, load) -> usize` (주입 로더 코어,
  헤드리스 테스트): `AudioPath` 보유 엔티티마다 경로별 1회 로드 →
  - `AudioSource` 없으면 `AudioSource::new(id)` 부착 (stopped 기본값 —
    재생 여부는 씬의 `playing` 플래그/게임 코드 소관),
  - 있으면 `clip_id` 가 다를 때만 재바인딩 (씬에 저장된 stale id 교정).
  - 변경/부착 건수 반환 — 두 번째 호출부터 0 (멱등, 로그 스팸 없음).
  - 주의 계약: AudioPath 보유 엔티티의 clip_id 는 resolve 가 소유 —
    게임 코드가 직접 clip_id 를 바꾸려면 AudioPath 를 떼야 함 (doc 명시).
- `resolve_audio_paths(world, audio: &mut AudioSystem, cache, vfs)` — 로더 =
  `read_asset`(vfs 우선, v1.0.100 규약) + `decode_clip_bytes` + `add_clip`.
  실패 log_error + None 캐시, 성공 시 1회 `resolved N audio path(s)`
  log_info (v1.0.102 양성 증거 규약).
- 배선: `tick` audio 단계 첫머리 (`self.audio.as_mut()` — GPU 불필요라
  handler 가 아닌 tick; 헤드리스 테스트도 동일 경로). `load_scene` 이
  `audio_clip_cache.clear_failures()`.

### 유닛 3 — voltex_cli 스캔 확장

`scan_scene_asset_paths` 에 `audio_path` 마커 추가 (TexPathRef 패턴) —
씬이 참조하는 .wav/.ogg 도 `.vpkg` 자동 동봉 + 누락 경고.

### 유닛 4 — editor_demo Open 배선

Audio Open 시 선택 엔티티가 있으면 `AudioSource::new(idx)`(stopped) +
`AudioPath(rel)` 부착 (texture/mesh 와 동일 패턴 — 핸들/인덱스는 런타임,
경로가 영속). 선택 없으면 기존 프리뷰-만 동작 유지.

## 검증

- ecs: 매크로 생성 impl round-trip (기존 renderer_ecs 테스트가 회귀 가드).
- audio: `decode_clip_bytes` wav 디코드 / 미지원 확장자 Err. +2.
- runtime: 코어 — AudioSource 부착 / stale clip_id 재바인딩 / 경로 공유
  로더 1회 / 실패 캐시 + clear 재시도 / 멱등 0. 실 래퍼 — vpkg 의 wav 를
  vfs 로 resolve (NullBackend 헤드리스). +5 내외.
- cli: 스캔이 audio_path 추출 (기존 테스트 확장).
- build/clippy(-D warnings)/workspace test 그린.

## 비범위 (후속)

- 텍스처 E2E(`e2e_texture_bundle.rs`)의 오디오 확장 — 다음 사이클 후보.
- AudioPath 핫리로드 자동 watch (fs resolve 시 watch_audio_clip 자동 등록).
- per-source min/max distance 영속화 (현재 전역 상수).

## CODEMAP 영향

- voltex_ecs (path_component! 추가), voltex_audio (decode_clip_bytes),
  voltex_runtime (audio_paths 모듈), voltex_cli (스캔 3종), renderer_ecs
  (매크로 마이그레이션 메모 해소).
