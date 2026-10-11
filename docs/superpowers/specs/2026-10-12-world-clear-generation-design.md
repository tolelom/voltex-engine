# World::clear() generation 보증 설계

- 날짜: 2026-10-12
- 출처: `docs/DEFERRED.md` "2026-08-09 감사 사이클 LOW 이월"의 설계 항목 (v1.0.114 audit-runtime 근본 원인 지적)
- 상태: v1.0.120 구현 완료 (구현 중 정정 2건은 아래 "구현 중 정정")

## 문제

`World::clear()`는 `*self = Self::new()`로 구현돼 있어 `EntityAllocator`까지 새로 만든다. clear 뒤 첫 spawn은 다시 `Entity { id: 0, generation: 0 }`를 받는다. clear 전에 받아 둔 `(0,0)` 핸들은 `is_alive`가 true가 되고, **새 엔티티를 가리킨다**.

용어집의 "Entity는 generation 기반으로 재사용이 안전하다"는 보증이 이 경로에서만 깨진다. v1.0.114에서 audio voice 누수로 실제 피해가 확인됐고, 그때는 `AudioVoiceTracker::stop_all`로 그 증상만 막았다.

## 조사: clear()를 거치는 경로와 World 밖에 남는 핸들

clear 호출 위치 (테스트 제외):

| 위치 | 상황 |
|---|---|
| `voltex_runtime::RuntimeApp::load_scene` (`app.rs`) | 씬 교체 |
| `voltex_editor::command` `EditorCommand::LoadScene` | 에디터 씬 로드 |
| `voltex_editor::session` Play → Edit 전환 | 플레이 스냅샷 복원 |

World 밖에서 `Entity`를 들고 있다가 clear를 넘기는 곳:

| 보유자 | clear 뒤 현재 동작 |
|---|---|
| `voltex_runtime::AudioVoiceTracker.voices` | v1.0.114에서 `stop_all`로 봉쇄됨 |
| `RuntimeApp.audio_listener: Option<Entity>` | `(0,0)` 등이 새 씬의 다른 엔티티를 리스너로 잡음 (**실버그**) |
| `EditorSession.selected` | Play → Edit 뒤 선택이 다른 엔티티로 넘어갈 수 있음 (**실버그**) |
| 인스펙터 `last_entity` | 같은 별칭 문제. 인스펙터 버퍼가 다른 엔티티 값을 보여줄 수 있음 |
| Lua 스크립트가 들고 있는 `(id, gen)` | 씬 교체 뒤 stale 핸들이 새 엔티티에 작동 |
| 게임 코드가 자체 보관한 핸들 | 같은 문제 |

영구 포맷은 다음과 같다.

- text `.vscn`, JSON, binary VSCN 셋 다 부모를 **로컬 인덱스**(`parent_index`, `"parent": idx`)로 저장하고 generation은 저장하지 않는다. 로드는 항상 `spawn()`으로 새 핸들을 받고 인덱스를 다시 매핑한다.
- 따라서 **씬 파일 호환은 generation 정책과 무관하다.** DEFERRED가 걱정한 "씬 파일 Entity 참조가 (id,gen)을 담는다"는 사실이 아니었다. 부모 참조는 인덱스다.
- net snapshot의 `EntityState`는 세션 동안의 와이어 데이터라 영구 포맷이 아니다. clear를 넘는 서버-클라이언트 매핑은 연결별 상태여서 이번 범위 밖이다.

## 결정

`World::clear()`는 allocator를 새로 만들지 않고 **살아 있는 모든 슬롯을 해제한다.** 해제할 때마다 generation이 오르므로 clear 전의 모든 핸들은 영구히 stale이 된다. 컴포넌트 저장소, 리소스, 이벤트 등 나머지 상태는 지금처럼 전부 초기화한다.

- `EntityAllocator::clear()` 신설: 살아 있는 entry를 모두 `alive = false`, `generation += 1`로 바꾸고 free_list에 넣는다. free_list는 **id 내림차순으로 쌓아** `pop()`이 0부터 꺼내게 한다. 그러면 clear 뒤 spawn 순서는 이전처럼 id 0, 1, 2…이고 generation만 다르다. id에 기대는 디버그 출력이나 순서 가정은 유지된다.
- ~~이미 해제된 슬롯은 generation이 이미 올라가 있으니 건드리지 않는다.~~ → 구현 중 정정: 해제된 슬롯도 올린다(아래).
- `World::clear()` = 저장소·리소스 초기화 + `allocator.clear()`.

### 기각한 대안

- **clear 뒤 첫 spawn의 generation을 "전역 epoch"로 올리기**: 슬롯마다 따로 추적하는 것보다 단순해 보이지만, `deallocate`/`restore_at_generation`의 "+1" 규약과 어긋나 E3 undo 로직을 다시 증명해야 한다. 기각.
- **보유자마다 clear 훅 달기** (지금의 stop_all 방식): 보유자가 늘 때마다 빠뜨린다. 이번 조사에서만 새 보유자 3곳이 나왔다. 근본 수정이 아니라 기각.

### 비용

- `entries`가 줄지 않는다. 메모리는 그 World가 겪은 최대 동시 엔티티 수에 비례한다(entry 8바이트). 씬을 교체해도 최대치는 비슷해서 무시할 수 있다.
- generation이 `u32` wrap할 때까지 같은 슬롯에서 clear/despawn이 약 43억 번 일어나야 한다. 기존 despawn 경로와 같은 가정이다.

## 동작 변경과 테스트

- 기존 테스트 `clear_then_spawn_reuses_id_zero_with_fresh_generation`은 `(0,0)` 재발급을 단언한다. 이번 결정으로 **의도된 동작이 바뀌므로** 테스트를 `id == 0`, `generation == 1`, 옛 핸들 `!is_alive`로 고친다.
- 새 테스트:
  - clear 전 핸들이 clear + 재spawn 뒤에도 `is_alive == false`이고 `get`이 None
  - 해제된 슬롯과 살아 있는 슬롯이 섞인 상태에서 clear 해도 모든 옛 핸들이 stale
  - clear 뒤 spawn id 순서가 0, 1, 2
  - runtime: `audio_listener`가 이전 씬 핸들이면 load_scene 뒤 리스너로 쓰이지 않음 (stale 판정)
  - editor: Play → Edit 뒤 `selected`의 옛 핸들이 `is_alive == false`
- 회귀: `games/`·`examples/`의 실제 씬 파일 로드 테스트, v1.0.114 voice 누수 테스트(`audio_sync.rs`), editor E3 undo 테스트가 전부 그대로 통과해야 한다.
- `AudioVoiceTracker::stop_all` 봉쇄는 남긴다. 씬 교체 때 이전 씬 소리를 끄는 것은 핸들 안전과 별개로 맞는 동작이다. `load_scene`의 "(0,0)부터 재발급" 주석만 고친다.

## 구현 중 정정

1. **해제된 슬롯도 generation을 올린다.** clear 전에 despawn된 슬롯은 generation이 `g+1`인 채로 남는다. 그래서 E3 undo의 `restore_at_generation(id, g)`이 clear 뒤에도 성공해 옛 엔티티를 새 씬에 되살릴 수 있었다. 지금 clear를 부르는 곳은 모두 undo 스택을 함께 비우지만, 보증은 allocator 안에서 닫는다. 회귀 테스트는 `clear_blocks_restore_of_a_handle_despawned_before_clear`.
2. **에디터 선택 재매핑은 범위 안으로 옮겼다.** Play → Edit 복원은 스냅샷 순서대로 spawn한다. 그래서 예전 `selected`는 별칭 때문에 *우연히* 같은 엔티티를 계속 가리켰다. stale 판정이 생기면 Play를 멈출 때마다 선택이 사라지는 UX 회귀가 난다. 대응으로 Play 시작 때 `Transform` 쿼리 순서(= `serialize_scene_binary` 인덱스 순서)를 기록해 두고, 복원 뒤 같은 인덱스의 새 핸들로 옮긴다. Play 중에 생긴 엔티티를 선택하고 있었으면 대응물이 없으니 None으로 둔다.

## 범위 밖

- `last_entity`·`audio_listener`와 에디터 LoadScene 뒤 `selected`를 None으로 정리하는 UX 처리. stale 판정으로 안전해지니 표시는 다음 프레임에 자연히 비워진다. 따로 필요하면 후속으로 한다.
- net 세션 중 씬 교체 프로토콜.
