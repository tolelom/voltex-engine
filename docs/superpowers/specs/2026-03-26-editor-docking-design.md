# Editor Docking System Design

## Overview

voltex_editor에 심플 도킹 시스템을 추가한다. 이진 분할 트리 기반으로 패널을 배치하고, 경계선 드래그로 리사이즈하며, 탭으로 같은 영역에 여러 패널을 전환할 수 있다.

## Scope

- 이진 분할 트리 (Split/Leaf 노드)
- 경계선 드래그 리사이즈
- 탭 표시 + 클릭 전환 (드래그 이동 없음)
- 런타임 상태 유지 (파일 저장 없음)
- 플로팅 윈도우 없음

## Data Model

```rust
pub enum Axis {
    Horizontal,  // 좌우 분할
    Vertical,    // 상하 분할
}

pub enum DockNode {
    Leaf {
        tabs: Vec<u32>,   // PanelId 목록
        active: usize,    // 현재 활성 탭 인덱스
    },
    Split {
        axis: Axis,
        ratio: f32,       // 0.1..=0.9, 첫 번째 자식 비율
        children: [Box<DockNode>; 2],
    },
}

pub struct DockTree {
    root: DockNode,
    names: Vec<&'static str>,       // panel_id → 표시 이름 (인덱스 = panel_id)
    cached_layouts: Vec<LeafLayout>, // layout() 결과 캐시 (draw_chrome에서 사용)
    resizing: Option<ResizeState>,   // 현재 리사이즈 중인 Split
    prev_mouse_down: bool,           // 이전 프레임 마우스 상태 (클릭 감지용)
}

struct ResizeState {
    node_path: Vec<usize>,  // 루트에서 해당 Split까지의 child 인덱스 경로
    axis: Axis,
    origin: f32,            // 분할 영역의 시작 좌표
    size: f32,              // 분할 영역의 전체 크기
}

pub struct LeafLayout {
    pub tabs: Vec<u32>,
    pub names: Vec<&'static str>,
    pub active: usize,
    pub tab_bar_rect: Rect,
    pub content_rect: Rect,
}
```

## Rect

레이아웃 계산에 사용할 사각형 구조체:

```rust
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
```

## Layout Algorithm

`DockTree::layout(screen_rect)` — 트리를 재귀 순회하며 각 Leaf에 영역을 배정한다.

- **Split 노드**: axis와 ratio에 따라 사각형을 둘로 분할
  - Horizontal: 좌(w * ratio) / 우(w * (1 - ratio))
  - Vertical: 상(h * ratio) / 하(h * (1 - ratio))
- **Leaf 노드**: 탭 바 높이(20px)를 빼고 나머지가 컨텐츠 영역
- 결과를 `cached_layouts`에 저장 (`draw_chrome`에서 사용)
- 반환: `Vec<(u32, Rect)>` — (활성 탭의 panel_id, 컨텐츠 rect)

## Resize

Split 경계선 ±3px 영역에서 마우스 드래그를 감지한다.

- `update(mouse_x, mouse_y, mouse_down)` 매 프레임 호출
- 클릭 감지: `mouse_down && !prev_mouse_down`으로 just-clicked 판별 (prev_mouse_down 내부 추적)
- 마우스 다운 시 경계선 위에 있으면 해당 Split 노드를 "active resize" 상태로 설정
- 드래그 중: 마우스 위치에 따라 ratio 업데이트
- 마우스 릴리즈 시 해제
- ratio는 0.1~0.9로 클램프 (패널이 너무 작아지지 않도록)

경계선 탐색: 트리를 재귀 순회하며 Split 노드마다 경계선 위치를 계산하고 마우스 위치와 비교한다.

## Tab Bar

각 Leaf 상단 20px에 탭 바를 렌더링한다.

- 탭 너비: 텍스트 길이 * 8px(글리프 폭) + 좌우 패딩 8px
- 클릭 시 active 인덱스 변경
- 활성 탭: 밝은 배경 (60, 60, 60)
- 비활성 탭: 어두운 배경 (40, 40, 40)
- 탭이 1개뿐이면 탭 바는 렌더하되 클릭 처리만 스킵

## Content Area

탭 바 아래 영역에 scissor rect를 적용하고 패널 콜백을 호출한다.

- 기존 `begin_panel`/`end_panel` 대신 rect를 직접 전달
- 패널 내부에서는 기존 위젯(text, button, slider 등)을 그대로 사용

## Visual Feedback

- Split 경계선: 1px 어두운 라인 (30, 30, 30)
- 리사이즈 중 경계선: 밝은 라인 (100, 100, 200)으로 하이라이트
- 탭 바 하단: 1px 구분선

## Invariants

- `tabs`는 반드시 1개 이상의 요소를 가져야 한다
- `active`는 항상 `tabs.len() - 1` 이하로 클램프한다 (out of bounds 방지)
- 리사이즈 핸들은 탭 바보다 우선한다 (겹치는 영역에서 리사이즈 우선)

## API

```rust
impl DockTree {
    /// 트리 생성. names: panel_id → 표시 이름 매핑 (인덱스 = panel_id)
    pub fn new(root: DockNode, names: Vec<&'static str>) -> Self;

    /// 레이아웃 계산 — 결과를 내부에 캐시하고, 활성 패널의 (panel_id, content_rect) 반환
    pub fn layout(&mut self, rect: Rect) -> Vec<(u32, Rect)>;

    /// 리사이즈 + 탭 클릭 처리 (내부에서 prev_mouse_down 추적)
    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool);

    /// 탭 바 + 경계선 렌더링 (cached_layouts 사용)
    pub fn draw_chrome(&self, ui: &mut UiContext);
}
```

## Usage Example

```rust
let names = vec!["Hierarchy", "Viewport", "Inspector", "Console"];
let mut dock = DockTree::new(DockNode::Split {
    axis: Axis::Horizontal,
    ratio: 0.25,
    children: [
        Box::new(DockNode::Leaf { tabs: vec![0], active: 0 }),
        Box::new(DockNode::Split {
            axis: Axis::Vertical,
            ratio: 0.7,
            children: [
                Box::new(DockNode::Leaf { tabs: vec![1], active: 0 }),
                Box::new(DockNode::Leaf { tabs: vec![2, 3], active: 0 }),
            ],
        }),
    ],
}, names);

// frame loop
let areas = dock.layout(Rect { x: 0.0, y: 0.0, w: 1280.0, h: 720.0 });
dock.update(mouse_x, mouse_y, mouse_down);
dock.draw_chrome(&mut ui);

for (panel_id, rect) in &areas {
    match panel_id {
        0 => draw_hierarchy(ui, rect),
        1 => draw_viewport(ui, rect),
        2 => draw_inspector(ui, rect),
        3 => draw_console(ui, rect),
        _ => {}
    }
}
```

## File Structure

- 새 파일: `crates/voltex_editor/src/dock.rs`
- `lib.rs`에 `mod dock; pub use dock::*;` 추가
- 기존 파일 수정 없음

## Testing

- 레이아웃 계산: 알려진 rect에 대해 분할 결과 검증
- 리사이즈: ratio 변경, 클램프 경계값
- 탭: active 인덱스 전환
- 중첩 분할: 3단계 이상 중첩된 트리 레이아웃 정확성
