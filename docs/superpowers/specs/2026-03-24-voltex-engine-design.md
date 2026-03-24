# Voltex Engine - Design Specification

## Overview

Voltex는 Rust로 작성되는 AAA급 3D 게임 엔진이다.
포트폴리오, 프로덕션, 기술 연구를 모두 목표로 한다.

- **언어**: Rust
- **타겟 플랫폼**: PC (Windows, Linux, macOS) + 콘솔 + 모바일
- **에디터**: 초기에는 코드 기반, 추후 GUI 에디터 추가

---

## Architecture

레이어 구조로 설계한다. 하위 레이어는 상위 레이어에 의존하지 않는다.

```
┌─────────────────────────────────┐
│         Game / App Layer        │  사용자 게임 코드
├─────────────────────────────────┤
│        Scripting Layer          │  추후 구현 (Lua 등)
├─────────────────────────────────┤
│        Engine Systems           │  물리, 오디오, AI, 네트워크
├─────────────────────────────────┤
│     Scene / ECS Framework       │  엔티티, 컴포넌트, 시스템
├─────────────────────────────────┤
│     Rendering Pipeline          │  PBR, GI, 레이트레이싱
├─────────────────────────────────┤
│   Graphics Abstraction (wgpu)   │  Vulkan/DX12/Metal/WebGPU
├─────────────────────────────────┤
│     Platform Layer (winit)      │  윈도우, 입력, OS 추상화
└─────────────────────────────────┘
```

### 의존성 규칙

- 각 레이어는 바로 아래 레이어에만 의존한다.
- 레이어를 건너뛰는 의존은 허용하지 않는다.
- 같은 레이어 내 모듈 간 순환 의존을 금지한다.
- **예외**: `voltex_math`는 기반 유틸리티로서 모든 레이어에서 직접 의존 가능하다.

---

## 외부 라이브러리 vs 자체 구현

### 외부 라이브러리 (Rust crate 의존: 2개만 사용)

| 라이브러리 | 버전 | 역할 | 라이선스 |
|-----------|------|------|---------|
| winit | 0.30.x | 크로스플랫폼 윈도우 생성, 키보드/마우스/터치 입력 | Apache 2.0 |
| wgpu | 24.x | Vulkan/DX12/Metal/WebGPU 그래픽스 API 추상화 | MIT / Apache 2.0 |

- 라이선스 의무: 출처 표기, 변경 명시 (Apache 2.0)
- 상업적 사용 자유, 로열티 없음, 소스 공개 의무 없음
- 개발 전용 의존(테스트, 벤치마크 등: criterion, glam 등)은 런타임에 포함되지 않으므로 이 제한에서 제외
- OS/시스템 API (Win32, CoreAudio, ALSA 등)와 C 라이브러리 (liblua)는 FFI를 통해 직접 호출하며, Rust crate 의존에 포함하지 않는다

### 자체 구현

| 시스템 | 설명 |
|--------|------|
| 수학 라이브러리 | Vec2, Vec3, Vec4, Mat3, Mat4, Quaternion, Transform, AABB, Ray, Plane |
| 렌더링 파이프라인 | 포워드/디퍼드 렌더링, PBR, GI, 레이트레이싱, 포스트 프로세싱 |
| ECS 프레임워크 | 엔티티, 컴포넌트 스토리지, 시스템 스케줄링, 쿼리 |
| 씬 관리 | 씬 그래프, 부모-자식 트랜스폼 계층, 씬 직렬화/역직렬화 |
| 에셋 시스템 | 에셋 로딩, 캐싱, 핫 리로드, 포맷 변환 |
| 물리 엔진 | 충돌 감지 (AABB, GJK/SAT), 리지드바디 시뮬레이션, 레이캐스팅 |
| 오디오 시스템 | 사운드 로딩/재생, 3D 공간 오디오, 믹서 |
| AI | 내비메시 생성, A* 패스파인딩 |
| 네트워킹 | 클라이언트-서버 아키텍처, UDP 기반 |
| 스크립팅 | Lua 바인딩 (자체 FFI 래퍼로 Lua C API 호출) |

---

## 구현 단계

### Phase 1: 기반 (Foundation)

**목표**: 윈도우에 삼각형 하나를 렌더링한다.

#### 1-1. 수학 라이브러리 (`voltex_math`)

**타입**:
- `Vec2`, `Vec3`, `Vec4` — 벡터 (f32 기반)
- `Mat3`, `Mat4` — 행렬
- `Quat` — 쿼터니언 (회전)
- `Transform` — Position + Rotation(Quat) + Scale
- `AABB` — Axis-Aligned Bounding Box
- `Ray` — 원점 + 방향
- `Plane` — 법선 + 거리

**연산**:
- 벡터: 덧셈, 뺄셈, 스칼라 곱, 내적, 외적, 정규화, 길이, lerp
- 행렬: 곱셈, 전치, 역행렬, look_at, perspective, orthographic
- 쿼터니언: 곱셈, slerp, from_axis_angle, from_euler, to_mat4
- Transform: 로컬→월드 변환, 부모-자식 결합

**최적화**:
- 초기에는 순수 Rust 구현 (정확성 우선)
- 추후 SIMD 최적화 (`std::arch` 또는 `core::simd`)

**테스트**:
- 모든 연산에 대해 단위 테스트
- glam 라이브러리 결과와 교차 검증

#### 1-2. 윈도우 + 입력 (`voltex_platform`)

**윈도우**:
- winit을 래핑하는 `Window` 구조체
- 생성 파라미터: 제목, 크기, 풀스크린 여부, vsync
- 리사이즈 이벤트 처리

**입력**:
- `InputState` — 현재 프레임의 입력 상태
- 키보드: `is_key_pressed(key)`, `is_key_just_pressed(key)`, `is_key_released(key)`
- 마우스: 위치, 델타, 버튼 상태, 스크롤
- 이벤트 큐 방식이 아닌 폴링 방식 (게임 루프에 적합)

**게임 루프**:
- 고정 타임스텝 + 가변 렌더링
- `fixed_update(dt)` — 물리/로직 (고정 간격, 기본 60Hz)
- `update(dt)` — 프레임별 로직
- `render(alpha)` — 보간 기반 렌더링

```
while running {
    accumulator += frame_time;
    while accumulator >= FIXED_DT {
        fixed_update(FIXED_DT);
        accumulator -= FIXED_DT;
    }
    let alpha = accumulator / FIXED_DT;
    update(frame_time);
    render(alpha);
}
```

#### 1-3. 렌더러 초기화 (`voltex_renderer`)

**wgpu 초기화**:
- Instance → Adapter → Device + Queue 생성
- Surface 설정 (윈도우와 연결)
- 기본 렌더 패스 구성

**첫 렌더링**:
- 하드코딩된 삼각형 정점 데이터
- 기본 버텍스/프래그먼트 셰이더 (WGSL)
- 파이프라인 생성 및 렌더링

**Phase 1 완료 기준**: 윈도우가 열리고, 색이 있는 삼각형이 보이며, ESC로 종료할 수 있다.

---

### Phase 2: 렌더링 기초

**목표**: 3D 모델을 로딩하고 카메라로 돌려볼 수 있다.

#### 2-1. 메시 시스템

- `Mesh` 구조체: 정점 버퍼 (position, normal, uv, tangent) + 인덱스 버퍼
- OBJ 파서 자체 구현 (간단한 포맷부터)
- 추후 glTF 파서 추가
- GPU 버퍼 업로드 및 관리

#### 2-2. 카메라

- `Camera` 구조체: position, rotation, fov, near, far
- Perspective / Orthographic 프로젝션
- FPS 스타일 카메라 컨트롤러 (마우스 + WASD)
- View-Projection 행렬 계산 (자체 수학 라이브러리 사용)

#### 2-3. 기본 라이팅

- Blinn-Phong 셰이딩 모델
- Directional Light 1개
- Ambient + Diffuse + Specular

#### 2-4. 텍스처

- PNG 디코더 자체 구현 (deflate + 필터링)
- JPG 디코더 자체 구현 (Huffman + DCT, baseline JPEG만 지원)
- GPU 텍스처 업로드
- 샘플러 설정 (필터링, 래핑)

**Phase 2 완료 기준**: .obj 모델을 로딩하고, 텍스처를 입히고, 카메라로 돌려볼 수 있다.

---

### Phase 3: ECS + 씬

**목표**: 데이터 주도 아키텍처로 다수의 오브젝트를 관리한다.

#### 3-1. ECS 프레임워크

**엔티티**:
- 경량 ID (u32 또는 u64)
- 생성/삭제, 세대(generation) 관리로 dangling 참조 방지

**컴포넌트 스토리지**:
- Archetype 기반 스토리지 (같은 컴포넌트 조합을 가진 엔티티를 연속 메모리에 배치)
- 캐시 친화적 순회
- 동적 컴포넌트 추가/제거 시 아키타입 마이그레이션

**시스템**:
- 시스템 = 특정 컴포넌트 조합을 쿼리하여 처리하는 함수
- 실행 순서 지정 (의존성 기반 스케줄링)
- 추후 병렬 실행 지원

**쿼리**:
- `Query<(&Position, &mut Velocity)>` 스타일
- 필터: With, Without, Changed

#### 3-2. 씬 그래프

- 부모-자식 트랜스폼 계층
- 로컬 트랜스폼 → 월드 트랜스폼 자동 계산
- 씬 직렬화/역직렬화 (JSON 또는 바이너리)

#### 3-3. 에셋 매니저

- 에셋 타입별 로더 등록
- 핸들 기반 참조 (경로 → 핸들 → 에셋)
- 참조 카운팅으로 메모리 관리
- 비동기 로딩 지원
- 핫 리로드 (파일 변경 감지 → 자동 리로드)

**Phase 3 완료 기준**: ECS로 수백 개의 엔티티를 생성하고, 각각 위치/메시/머티리얼 컴포넌트를 가지며, 씬을 저장/로드할 수 있다.

---

### Phase 4: PBR 렌더링

**목표**: 물리 기반 렌더링으로 사실적인 머티리얼을 표현한다.

#### 4-1. PBR 머티리얼

- Metallic-Roughness 워크플로 (glTF 표준)
- 텍스처 맵: Albedo, Normal, Metallic, Roughness, AO, Emissive
- Cook-Torrance BRDF (GGX 분포, Smith 기하, Fresnel-Schlick)

#### 4-2. 다중 라이트

- Directional Light (태양)
- Point Light (감쇠 포함)
- Spot Light (각도 + 감쇠)
- 라이트 컬링 (타일/클러스터 기반)

#### 4-3. 섀도우

- Directional Light: Cascaded Shadow Maps (CSM)
- Point Light: Omnidirectional Shadow Maps (큐브맵)
- Spot Light: 단일 Shadow Map
- PCF (Percentage Closer Filtering) 소프트 섀도우

#### 4-4. IBL (Image-Based Lighting)

- 환경맵 (HDR 큐브맵)
- Diffuse Irradiance Map
- Specular Pre-filtered Environment Map
- BRDF LUT

**Phase 4 완료 기준**: 금속, 나무, 돌 등 다양한 머티리얼이 사실적으로 보이며, 여러 종류의 광원과 그림자가 작동한다.

---

### Phase 5: 물리 엔진

**목표**: 기본적인 물리 시뮬레이션이 동작한다.

#### 5-1. 충돌 감지

**Broad Phase**:
- AABB 기반 BVH (Bounding Volume Hierarchy)
- SAP (Sweep and Prune) 알고리즘

**Narrow Phase**:
- GJK (Gilbert-Johnson-Keerthi) 알고리즘
- EPA (Expanding Polytope Algorithm) — 침투 깊이
- 지원 콜라이더: Sphere, Box, Capsule, Convex Hull

#### 5-2. 리지드바디 시뮬레이션

- 질량, 관성 텐서, 중력
- 속도/각속도 적분 (Semi-implicit Euler)
- 충돌 응답: 임펄스 기반 해결
- 마찰 (Coulomb)
- 제약 조건 솔버 (Sequential Impulse)

#### 5-3. 레이캐스팅

- Ray vs AABB, Sphere, Plane, Triangle, Mesh
- BVH 가속 구조를 이용한 빠른 레이캐스트

**Phase 5 완료 기준**: 상자와 구가 떨어지고, 바닥과 충돌하고, 서로 밀어내며, 레이캐스트로 오브젝트를 선택할 수 있다.

---

### Phase 6: 오디오 시스템

**목표**: 게임에 사운드를 추가할 수 있다.

#### 6-1. 오디오 백엔드

- 플랫폼별 오디오 출력 자체 구현
  - Windows: WASAPI
  - macOS: CoreAudio
  - Linux: ALSA / PulseAudio
- 오디오 스레드 분리

#### 6-2. 사운드 재생

- WAV 파서 자체 구현
- OGG/Vorbis 디코더 자체 구현 (추후)
- 재생, 일시정지, 정지, 볼륨, 루프

#### 6-3. 3D 오디오

- 거리 기반 감쇠
- 패닝 (스테레오)
- 도플러 효과 (추후)

#### 6-4. 믹서

- 채널/그룹 기반 믹싱 (BGM, SFX, Voice 등)
- 볼륨 페이드 인/아웃

**Phase 6 완료 기준**: 3D 공간에서 소리가 위치에 따라 다르게 들리며, BGM과 SFX를 독립적으로 제어할 수 있다.

---

### Phase 7: 고급 렌더링 (Stretch Goal)

**목표**: AAA급 비주얼 퀄리티에 도달한다.

#### 7-1. 디퍼드 렌더링

- G-Buffer: Position, Normal, Albedo, Metallic-Roughness, Depth
- 라이팅 패스 분리
- 다수의 라이트를 효율적으로 처리

#### 7-2. Global Illumination

단계적으로 구현한다:
1. **Light Probes** (먼저) — 베이크 기반, 구현 난이도 낮음, 정적 씬에 효과적
2. **SSGI** (다음) — 스크린 스페이스 기반, 동적 GI, 포스트 프로세싱 파이프라인과 자연스럽게 통합

#### 7-3. 레이트레이싱

- wgpu의 레이트레이싱 확장 활용
- RT Shadows, RT Reflections, RT AO
- 하이브리드 렌더링 (래스터 + RT)

#### 7-4. 포스트 프로세싱

- HDR 렌더링 + 톤매핑 (ACES, Filmic 등)
- Bloom
- SSAO (Screen-Space Ambient Occlusion)
- SSR (Screen-Space Reflections)
- Motion Blur
- Depth of Field
- Anti-Aliasing (TAA)

**Phase 7 완료 기준**: 디퍼드 렌더링으로 수백 개의 라이트를 처리하고, GI/RT/포스트 프로세싱이 적용된 씬이 AAA 수준으로 보인다.

> **Core Deliverable 경계**: Phase 1~6이 핵심 산출물이다. Phase 7~8은 핵심이 완성된 후 진행하는 확장 목표이며, 구현 계획은 Phase 6 완료 시점에 재평가한다.

---

### Phase 8: 확장 (Stretch Goal)

**목표**: 엔진을 완전한 게임 개발 도구로 확장한다.

#### 8-1. AI

- 내비메시 생성 (Recast 스타일 알고리즘)
- A* 패스파인딩
- 스티어링 행동 (Seek, Flee, Arrive, Wander)

#### 8-2. 네트워킹

- UDP 소켓 기반
- 클라이언트-서버 아키텍처
- 상태 동기화 (스냅샷 + 보간)
- 지연 보상

#### 8-3. 스크립팅

- Lua 바인딩 (자체 FFI 래퍼로 Lua C API 호출)
- 핫 리로드 지원
- 엔진 API 노출 (엔티티 생성/삭제, 컴포넌트 접근 등)

#### 8-4. 에디터 (GUI)

- 자체 Immediate Mode UI 시스템 (voltex_renderer 위에 구축)
- 씬 뷰포트 (3D 뷰)
- 엔티티 인스펙터
- 에셋 브라우저
- 실시간 프리뷰

**Phase 8 완료 기준**: 에디터에서 씬을 편집하고, AI 에이전트가 내비메시를 따라 이동하며, 멀티플레이어 테스트가 가능하다.

---

## 프로젝트 구조

```
voltex/
├── Cargo.toml              # 워크스페이스
├── crates/
│   ├── voltex_math/        # 수학 라이브러리
│   ├── voltex_platform/    # 윈도우, 입력, 게임 루프
│   ├── voltex_renderer/    # 렌더링 파이프라인
│   ├── voltex_ecs/         # ECS 프레임워크
│   ├── voltex_scene/       # 씬 관리
│   ├── voltex_asset/       # 에셋 시스템
│   ├── voltex_physics/     # 물리 엔진
│   ├── voltex_audio/       # 오디오 시스템
│   ├── voltex_ai/          # AI 시스템
│   ├── voltex_net/         # 네트워킹
│   ├── voltex_script/      # 스크립팅
│   └── voltex_editor/      # 에디터 (추후)
├── examples/               # 예제/데모
├── assets/                 # 테스트용 에셋
└── docs/                   # 문서
```

Cargo 워크스페이스로 관리하며, 각 crate는 독립적으로 빌드/테스트 가능하다.

---

## 테스트 전략

- **단위 테스트**: 각 crate 내 `#[cfg(test)]` 모듈
- **통합 테스트**: `examples/` 디렉토리의 데모 앱
- **벤치마크**: `criterion` crate로 성능 측정 (수학, ECS 쿼리, 렌더링 등)
- **교차 검증**: 수학 라이브러리는 glam 결과와 비교

---

## 성능 목표

- 60 FPS 이상 (1080p, 복잡한 씬)
- ECS: 100,000 엔티티 순회 < 1ms
- 물리: 1,000 리지드바디 실시간 시뮬레이션
- 렌더링: 디퍼드 파이프라인에서 1,000+ 라이트

---

## 라이선스

- Voltex 엔진 코드: MIT / Apache 2.0 이중 라이선스 (Rust 생태계 관례)
- 외부 의존: winit (Apache 2.0), wgpu (MIT / Apache 2.0)
- 라이선스 고지 파일을 `THIRD_PARTY_LICENSES` 에 유지한다
