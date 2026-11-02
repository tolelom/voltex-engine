# 타일맵 배치 렌더 설계

- 날짜: 2026-10-12
- 출처: `docs/dev/dogfood-friction-2.md` #5 (P2) — 타일맵 개념이 없어 솔리드 셀마다 `Sprite` 엔티티를 하나씩 만들었다.
- 상태: 확정. B3a(엔진) → B3b(voltex_leap 마이그레이션) 순서로 구현.

## 문제

leap은 솔리드 셀마다 `Transform + Sprite` 엔티티를 하나씩 스폰한다. 지금 레벨은 약 100개라 문제가 없지만 구조적 한계가 있다.

- 스프라이트 패스는 엔티티마다 dynamic UBO 슬롯을 하나 쓰고, 드로우 콜도 하나씩이다. `MAX_SPRITES = 256`이라 큰 맵은 타일만으로 상한을 넘는다. 넘은 만큼은 v1.0.114 컬링 규칙에 따라 먼 것부터 사라진다.
- 타일, 플레이어, 코인이 같은 정렬·컬링 예산을 나눠 쓰므로 타일이 많아지면 게임 오브젝트까지 잘린다.

## 결정

### 컴포넌트 (`voltex_renderer_ecs::Tilemap`)

```rust
pub struct Tilemap {
    pub cols: u32,
    pub rows: u32,
    pub tile_size: f32,              // 한 셀의 월드 크기 (정사각)
    pub tiles: Vec<u16>,             // 행 우선, len = cols*rows. 0 = 빈 셀, n = 아틀라스 프레임 n-1
    pub texture: Option<TextureHandle>, // None → 흰 기본 텍스처 (솔리드 색)
    pub atlas: [u32; 2],             // 아틀라스 [cols, rows]. 프레임 번호는 Sprite::set_sheet_frame과 같은 규약(좌상단부터 행 우선)
    pub color: [f32; 4],             // 맵 전체 틴트
}
```

- 좌표: 셀 `(gx, gy)`는 로컬 XY 평면의 `[gx*ts, (gx+1)*ts] × [gy*ts, (gy+1)*ts]`를 덮는다. **gy는 +y 위쪽**이다(leap의 `cell_center`와 같은 규약). `tiles` 인덱스는 `gy * cols + gx`.
- 엔티티의 `WorldTransform` 행렬을 로컬 꼭짓점에 그대로 곱한다. 그래서 위치, 회전, 스케일이 다 적용되고 `Hidden`이면 그리지 않는다. 스프라이트와 달리 빌보드가 아니다. 맵은 월드에 고정된 평면이다.
- 셀별 색, 애니메이션 타일, 충돌 데이터는 넣지 않는다(YAGNI). 충돌은 `voltex_physics::grid2d`가 게임의 `is_solid` 클로저로 이미 처리하고 있다. 렌더 데이터와 충돌 데이터를 하나로 합치는 건 두 번째 사용처가 생길 때 판단한다.

### 렌더 (`BasicRenderer` 타일맵 패스)

- 꼭짓점: `TilemapVertex { position: [f32; 3], uv: [f32; 2], color: [f32; 4] }` (36바이트). CPU에서 월드 좌표로 펼친다. 빈 셀은 건너뛴다.
- 유니폼: 프레임당 `view_proj` 하나. dynamic offset은 쓰지 않는다.
- 버퍼: 기존 패턴(`MAX_SPRITES`)과 맞춰 고정 용량을 쓴다(`BasicRenderer`는 device를 보관하므로 늘리는 것도 가능하지만, 상한 + 경고 한 번이 스프라이트와 같은 계약이라 더 단순하다). `MAX_TILE_QUADS = 16384` 쿼드(VB 약 2.4MB, u32 IB 약 393KB). 넘으면 앞에서부터 상한까지만 그리고 경고는 한 번만 낸다(`warn_overflow_once`).
- 드로우: 타일맵마다 꼭짓점 구간을 이어 붙여 **텍스처 바인드 1회 + `draw_indexed` 1회**로 그린다. 타일 수와 드로우 콜 수가 무관하다.
- 파이프라인: 스프라이트와 같은 상태다(알파 블렌딩, depth test LessEqual, depth write 없음, 컬링 없음). 그래야 블렌딩과 겹침이 기존 스프라이트 타일과 같아진다. 셰이더는 `tilemap.wgsl`이다(텍스처 × 꼭짓점 색).
- 순서: 스킨드 패스 다음, **스프라이트 패스 앞**. 같은 깊이(z=0)에서 스프라이트(플레이어·코인)가 타일 위에 합성된다.
- 매 프레임 다시 펼친다. 16384쿼드 기준 CPU 쓰기는 약 2.4MB/프레임으로 2D 게임 규모에서 문제가 없다. 변경 감지 캐시는 측정으로 필요가 확인되면 넣는다.

### 검증

- 단위(GPU 없음): `tilemap_vertices()`. 빈 셀 제외, 셀 꼭짓점의 월드 위치(원점·`tile_size`·`WorldTransform` 이동/스케일), 아틀라스 프레임 UV, 틴트 전달, `tiles.len()` 불일치 처리(짧으면 있는 만큼, 길면 무시).
- 단위: 꼭짓점·유니폼 크기와 정렬.
- GPU 캡처(`voltex_capture`, lavapipe/실 GPU, 없으면 graceful skip): 직교 카메라로 2×1 타일맵(셀 하나는 빈 칸)을 그려 채운 셀 픽셀은 틴트 색이고 빈 셀 픽셀은 배경색인지 단언. PNG는 `docs/visual/tilemap_basic.png`.
- B3b: leap `spawn_level`이 솔리드 셀 대신 `Tilemap` 엔티티 하나를 스폰한다. 기존 헤드리스 플레이스루, 단위 테스트, GPU visual 회귀(`docs/visual/leap_level.png` 구도)가 통과해야 한다. visual 회귀는 타일 색 픽셀 단언이 그대로 성립해야 하고, 스크린샷은 갱신한다.

## 범위 밖

- 씬 직렬화 등록: 지금 `Sprite`도 직렬화하지 않고 게임 코드가 스폰한다. 에디터 저작 흐름이 생기면 같이 다룬다.
- 셀 단위 수정 API: `tiles`가 `pub`이라 직접 고치면 다음 프레임에 반영된다.
