# 이어 할 작업

> 2026-10-12 밤샘 작업(v1.0.115 ~ v1.0.171) 뒤에 남은 항목입니다. 세부 사실은 링크한 문서에 있고, 여기에는 무엇을 하고 무엇에 막혔는지만 둡니다. 처리하면 항목을 지우고 결과는 CHANGELOG에 적습니다.

## 1. 창을 띄워 확인

헤드리스 테스트로는 볼 수 없는 체감 확인입니다.

| 항목 | 확인할 것 | 관련 버전 |
|---|---|---|
| 물리 체감 | `survivor_game`·`editor_demo`에서 물체 간 통과, 빠른 물체의 벽 충돌, 미끄러짐·정지 | v1.0.152 브로드페이즈 쌍 누락, v1.0.153 CCD, v1.0.154 마찰 |
| 크래시 리포트 GPU 정보 | 실제 패닉 시 리포트에 어댑터 정보가 들어가는지 | v1.0.127 (컴파일만 확인) |
| 튜토리얼 따라가기 | `docs/tutorial.md`를 처음부터 따라가며 에디터 조작(2~3장)이 문서와 같은지, 4장을 `input.is_key_pressed`·`add_input_system` 입력으로, 5장을 HUD(`with_hud`·`set_hud_fn`)로 바꿀지. 스크린샷도 이때 | 2026-11-02 코드 대조로만 고침 |

물리 세 수정은 버그 수정이지만 손맛이 달라질 수 있습니다. 회귀로 보이면 해당 버전의 CHANGELOG 항목과 속성 테스트(`crates/voltex_physics/tests/*_property.rs`)부터 봅니다.

## 2. 결정이 필요한 것

| 항목 | 현상 | 선택지 |
|---|---|---|
| glTF 머티리얼 텍스처 | 텍스처를 무시하고 색 계수만 로드 ([DEFERRED](DEFERRED.md) "2026-10-12 밤샘 점검") | baseColor·normal부터 임포트 / 실제 glTF 에셋을 쓸 때까지 보류 |

## 3. 설계가 필요한 것

| 항목 | 막힌 이유 | 기록 위치 |
|---|---|---|
| 안에서 시작하는 레이 규약 | Box는 t=0, Sphere·Capsule은 출구 t. 통일하면 동작이 바뀌므로 호출 측이 t=0을 어떻게 쓰는지 확인 필요 | [DEFERRED](DEFERRED.md) "레이캐스트" |
| `.vpkg` 4 GiB 한계 | 인덱스 offset·size가 u32. 포맷 버전을 올릴지, 쓰기 단계에서 에러를 낼지 호환성 결정 필요 | [DEFERRED](DEFERRED.md) "`.vpkg` 4 GiB 한계" |
| 지연 보상 타임스탬프 정밀도 | `HistoryEntry.timestamp`가 f32라 서버 가동 수 시간 뒤 ms 정밀도 손실 가능. 공개 필드라 타입 변경은 API 변경 (`crates/voltex_net/src/lag_compensation.rs`) | CHANGELOG v1.0.161 |
| CI 첫 green run | GitHub 미러 push가 사용자 작업. 그 전엔 CI 게이트가 실제로 돌지 않음 | [AUDIT-BACKLOG](AUDIT-BACKLOG.md) F7, [release.md](../release.md) |

## 4. 동작이 바뀐 것 (게임 코드 점검용)

- 죽은 엔티티나 옛 핸들에 대한 `World::add`는 아무것도 하지 않음 (v1.0.171)
- 법선 하중이 없는 접촉에는 마찰 없음 (v1.0.154)
- 보간 버퍼는 같은 시각의 스냅샷을 교체하고, 순서가 뒤바뀌어 도착해도 시간순으로 넣음 (v1.0.160)
