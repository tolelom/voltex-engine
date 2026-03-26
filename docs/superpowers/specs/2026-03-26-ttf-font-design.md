# TTF Font Design

## Overview

자체 TTF 파서 + 래스터라이저를 구현하고, 온디맨드 글리프 캐시 아틀라스로 가변폭 유니코드 텍스트를 렌더링한다. 기존 FontAtlas를 유지하면서 TtfFont를 병행 추가.

## Scope

- TTF 바이너리 파싱 (head, hhea, maxp, cmap, loca, glyf, hmtx)
- Quadratic bezier 아웃라인 → 비트맵 래스터라이징
- 온디맨드 글리프 캐시 (1024x1024 R8Unorm 아틀라스)
- 가변폭 텍스트 렌더링 (ASCII + 한글 + 유니코드)
- UiContext에 옵션 통합

## TtfParser

TTF 바이너리에서 필요한 테이블을 파싱한다.

```rust
pub struct TtfParser {
    data: Vec<u8>,
    // 테이블 오프셋 캐시
    head_offset: usize,
    hhea_offset: usize,
    maxp_offset: usize,
    cmap_offset: usize,
    loca_offset: usize,
    glyf_offset: usize,
    hmtx_offset: usize,
    // 헤더 정보
    pub units_per_em: u16,
    pub num_glyphs: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    num_h_metrics: u16,
    loca_format: i16,  // 0=short, 1=long
}
```

### 파싱 순서
1. Offset Table: sfVersion, numTables → 테이블 디렉토리 순회
2. 각 테이블 태그(4바이트 ASCII)와 offset/length 기록
3. head: units_per_em, indexToLocFormat
4. hhea: ascender, descender, lineGap, numberOfHMetrics
5. maxp: numGlyphs
6. cmap: Format 4 (BMP) 또는 Format 12 (full Unicode) 서브테이블

### 주요 메서드
- `parse(data: &[u8]) -> Result<TtfParser, String>`
- `glyph_index(codepoint: u32) -> u16` — cmap 조회
- `glyph_outline(glyph_id: u16) -> GlyphOutline` — glyf 테이블에서 아웃라인 추출
- `glyph_metrics(glyph_id: u16) -> GlyphMetrics` — hmtx에서 advance/bearing

### GlyphOutline
```rust
pub struct GlyphOutline {
    pub contours: Vec<Vec<OutlinePoint>>,
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,
}

pub struct OutlinePoint {
    pub x: f32,
    pub y: f32,
    pub on_curve: bool,
}

pub struct GlyphMetrics {
    pub advance_width: u16,
    pub left_side_bearing: i16,
}
```

### glyf 테이블 파싱
- Simple glyph: numberOfContours >= 0
  - endPtsOfContours, flags, x/y 좌표 (delta 인코딩)
  - on_curve 플래그로 직선/곡선 구분
  - off-curve 점 사이에 암시적 on-curve 점 삽입
- Compound glyph: numberOfContours < 0
  - 여러 simple glyph를 변환(translate/scale)하여 합성
  - 재귀적 처리

### cmap Format 4
BMP(U+0000~U+FFFF) 매핑:
- segCount, endCode[], startCode[], idDelta[], idRangeOffset[] 배열
- 이진 탐색으로 codepoint가 속하는 세그먼트 찾기
- idRangeOffset == 0: glyph = (codepoint + idDelta) % 65536
- idRangeOffset != 0: glyphIdArray에서 조회

## Rasterizer

글리프 아웃라인을 알파 비트맵으로 변환한다.

### 입력
- GlyphOutline (contours of points)
- font_size (pixel height)
- units_per_em

### 스케일링
```
scale = font_size / units_per_em
pixel_x = (point.x - x_min) * scale
pixel_y = (y_max - point.y) * scale  // Y 뒤집기 (TTF는 Y-up)
```

### Bezier Flattening
Quadratic bezier (P0, P1, P2)를 선분으로 분할:
- 재귀 분할: 중간점과 직선 거리가 threshold(0.5px) 이하이면 직선으로 근사
- 결과: Vec<(f32, f32)> 선분 목록

### 스캔라인 래스터라이징
1. 모든 contour를 선분 목록으로 변환
2. 각 스캔라인 (y = 0..height):
   - 모든 선분과 교차점 계산
   - 교차점을 x 기준 정렬
   - winding rule (non-zero): 교차할 때 winding number 증감
   - winding != 0인 구간을 채움

### 안티앨리어싱
- 서브픽셀 없이 단순 coverage: 픽셀 경계에서의 부분 커버리지 계산
- 또는 4x 수직 슈퍼샘플링 (y를 4등분, 평균)

### 출력
```rust
pub fn rasterize(outline: &GlyphOutline, scale: f32) -> RasterResult

pub struct RasterResult {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u8>,  // R8 알파맵
    pub offset_x: f32,    // bearing
    pub offset_y: f32,    // baseline offset
}
```

## GlyphCache

온디맨드 아틀라스를 관리한다.

```rust
pub struct GlyphCache {
    atlas_data: Vec<u8>,       // CPU측 R8 버퍼 (텍스처 업로드용)
    atlas_width: u32,          // 1024
    atlas_height: u32,         // 1024
    glyphs: HashMap<char, GlyphInfo>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    dirty: bool,               // GPU 업로드 필요 여부
}

pub struct GlyphInfo {
    pub uv: [f32; 4],         // u0, v0, u1, v1
    pub width: f32,
    pub height: f32,
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}
```

### 글리프 삽입 로직
1. `glyphs.get(ch)` 로 캐시 확인
2. 없으면: parser.glyph_index → parser.glyph_outline → rasterize
3. 아틀라스에 비트맵 복사 (cursor_x, cursor_y 위치)
4. cursor_x 전진. 줄 넘김 시 cursor_y += row_height, cursor_x = 0
5. GlyphInfo 생성 (UV 좌표 계산)
6. dirty = true

### GPU 업로드
- `upload_if_dirty(queue, texture)` — dirty면 queue.write_texture로 전체 아틀라스 업로드
- 또는 변경된 영역만 부분 업로드 (최적화, 나중에)

## TtfFont

통합 인터페이스.

```rust
pub struct TtfFont {
    parser: TtfParser,
    cache: GlyphCache,
    pub font_size: f32,
    pub line_height: f32,
    pub ascender: f32,
    pub descender: f32,
    scale: f32,
}
```

### 메서드
- `new(data: &[u8], font_size: f32) -> Result<Self, String>`
- `glyph(&mut self, ch: char) -> &GlyphInfo`
- `text_width(&mut self, text: &str) -> f32`
- `atlas_data(&self) -> &[u8]` — CPU 아틀라스 데이터
- `atlas_size(&self) -> (u32, u32)`
- `is_dirty(&self) -> bool`
- `clear_dirty(&mut self)`

## UI 통합

### UiContext 변경
```rust
pub struct UiContext {
    // 기존 필드...
    pub font: FontAtlas,             // 기존 비트맵 폰트 유지
    pub ttf_font: Option<TtfFont>,   // TTF 폰트 (옵션)
}
```

### 텍스트 렌더링 변경
기존 위젯(text, button, slider 등)에서 텍스트를 그릴 때:
- `ttf_font`이 Some이면 → TTF 글리프 사용 (가변폭)
- None이면 → 기존 비트맵 폰트 폴백

이를 위한 헬퍼:
```rust
impl UiContext {
    /// 텍스트 너비 계산 (TTF or 비트맵)
    pub fn text_width(&mut self, text: &str) -> f32;

    /// 텍스트 렌더링 (TTF or 비트맵)
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, color: [u8; 4]);
}
```

기존 위젯 코드에서 인라인 글리프 렌더링을 `draw_text()` 호출로 교체하면 TTF/비트맵 전환이 투명해짐.

### UiRenderer 변경
- TTF 아틀라스 텍스처 생성 + 바인드 그룹
- 매 프레임 dirty 체크 → 업로드
- 기존 ui_shader.wgsl 재사용 가능 (R8 알파 텍스처 동일 패턴)
- DrawCommand에 텍스처 종류 플래그 추가 (bitmap vs ttf) 또는 별도 draw call

## File Structure

- `crates/voltex_editor/src/ttf_parser.rs` — TTF 바이너리 파싱
- `crates/voltex_editor/src/rasterizer.rs` — 아웃라인 → 비트맵
- `crates/voltex_editor/src/glyph_cache.rs` — 온디맨드 아틀라스
- `crates/voltex_editor/src/ttf_font.rs` — 통합 인터페이스
- `crates/voltex_editor/src/lib.rs` — 모듈 추가

## Testing

### TtfParser (순수 바이트 파싱, GPU 불필요)
- 테이블 디렉토리 파싱
- cmap 매핑: 'A'(0x41) → 올바른 glyph index
- 글리프 아웃라인: 알려진 글리프의 contour 수, 점 수 검증
- hmtx: advance width 검증
- 에러 케이스: 잘못된 데이터

### Rasterizer (순수 수학, GPU 불필요)
- 직선 contour (사각형) → 비트맵 검증
- Bezier flatten: 곡선이 선분으로 올바르게 분할되는지
- 빈 글리프(space 등) → 0x0 비트맵
- 스케일링 정확성

### GlyphCache (GPU 불필요 — CPU측만 테스트)
- 글리프 삽입 → UV 좌표 검증
- 중복 삽입 → 캐시 히트
- 아틀라스 가득 참 → 줄 넘김 동작

### TtfFont (통합)
- text_width: "Hello" 너비 > 0
- 한글 글리프 래스터라이즈 성공

### 테스트 TTF 파일
NotoSansMono-Regular.ttf 등 오픈소스 폰트를 `assets/fonts/` 에 포함하거나, 테스트에서 시스템 폰트 경로 사용.
