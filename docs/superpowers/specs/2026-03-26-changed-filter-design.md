# Changed Filter Design

## Overview

ECS SparseSet에 tick 기반 변경 감지를 추가하여 `query_changed::<T>()` 로 수정된 컴포넌트만 순회할 수 있게 한다.

## Scope

- SparseSet에 변경 tick 추적
- get_mut, insert 시 변경 마크
- World::query_changed, clear_changed

## SparseSet 변경

```rust
pub struct SparseSet<T> {
    sparse: Vec<Option<usize>>,
    dense_entities: Vec<Entity>,
    dense_data: Vec<T>,
    // 새로 추가:
    ticks: Vec<u64>,        // dense_data와 동일 인덱스, 마지막 변경 틱
    current_tick: u64,      // 현재 틱 (clear_changed에서 증가)
}
```

### 변경 감지 로직
- `insert(entity, value)`: tick 기록 `ticks.push(current_tick)`
- `get_mut(entity)`: `ticks[index] = current_tick` (접근 시 변경 마크)
- `get(entity)`: tick 변경 없음 (읽기만)
- `remove(entity)`: swap-remove 시 ticks도 동일하게 swap
- `is_changed(entity)`: `ticks[index] == current_tick`
- `increment_tick()`: `current_tick += 1`
- `changed_entities()`: ticks == current_tick인 엔티티 목록

## World 변경

```rust
impl World {
    /// 현재 틱에 변경된 T 컴포넌트를 가진 엔티티 순회
    pub fn query_changed<T: 'static>(&self) -> Vec<(Entity, &T)>;

    /// 모든 컴포넌트 스토리지의 tick 증가 (프레임 끝에 호출)
    pub fn clear_changed(&mut self);
}
```

### clear_changed 구현
ComponentStorage trait에 `increment_tick()` 메서드 추가. World가 모든 스토리지를 순회하며 호출.

## ComponentStorage trait 확장

```rust
pub trait ComponentStorage: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove_entity(&mut self, entity: Entity);
    fn increment_tick(&mut self);  // NEW
}
```

## Testing

- `test_insert_is_changed`: insert 후 is_changed → true
- `test_get_mut_marks_changed`: get_mut 후 is_changed → true
- `test_get_not_changed`: get 후 is_changed → false (이전 틱에 insert한 경우)
- `test_clear_changed`: insert → clear_changed → is_changed → false
- `test_query_changed`: 3개 엔티티 중 1개만 get_mut → query_changed → 1개만
- `test_remove_preserves_ticks`: swap-remove 후 남은 엔티티 tick 정확성
