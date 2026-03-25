# Phase 2-3a Deferred Items Spec

## A. JPG 디코더 (`voltex_renderer/src/jpg.rs`)

### 범위
- Baseline JPEG (SOF0) only
- Progressive, Arithmetic coding 미지원
- 외부 의존성 없음 (자체 구현)

### API
```rust
pub fn parse_jpg(data: &[u8]) -> Result<(Vec<u8>, u32, u32), String>
```
- PNG `parse_png`과 동일 패턴: (RGBA pixels, width, height) 반환

### 구현 요소
1. JFIF 마커 파싱: SOI, SOF0, DHT, DQT, SOS, EOI
2. Huffman 디코더: DC/AC 테이블 구축 + 비트스트림 디코딩
3. 역양자화 (dequantization): DQT 테이블 × DCT 계수
4. 8x8 IDCT: 정수 또는 부동소수점
5. YCbCr → RGB 색공간 변환
6. MCU 블록 조립: 4:4:4, 4:2:2, 4:2:0 크로마 서브샘플링
7. Restart marker (DRI/RST) 지원

### 테스트
- 최소 synthetic JPEG 바이트로 roundtrip 검증
- Huffman 테이블 구축 단위 테스트
- IDCT 정확도 테스트
- 서브샘플링별 디코딩 테스트

---

## B. glTF/GLB 파서 (`voltex_renderer/src/gltf.rs`)

### 범위
- glTF 2.0 JSON (embedded base64) + .glb 바이너리
- 메시 지오메트리 + 기본 PBR 머티리얼
- 애니메이션, 스킨, 카메라, 라이트 확장은 미포함

### API
```rust
pub fn parse_gltf(data: &[u8]) -> Result<GltfData, String>

pub struct GltfData {
    pub meshes: Vec<GltfMesh>,
}

pub struct GltfMesh {
    pub vertices: Vec<MeshVertex>,  // 기존 MeshVertex 재사용
    pub indices: Vec<u32>,
    pub name: Option<String>,
    pub material: Option<GltfMaterial>,
}

pub struct GltfMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}
```

### 구현 요소
1. GLB 헤더 파싱: magic(0x46546C67), version 2, JSON chunk, BIN chunk
2. 미니 JSON 파서: 외부 의존성 없음, glTF에 필요한 subset만
3. Accessor/BufferView → 정점 데이터 추출
   - POSITION (vec3), NORMAL (vec3), TEXCOORD_0 (vec2), TANGENT (vec4)
   - indices (u16/u32)
4. 탄젠트 없으면 기존 `compute_tangents()` 재사용
5. Material: pbrMetallicRoughness → GltfMaterial 매핑
6. Embedded base64 버퍼 디코딩

### 테스트
- 미니 JSON 파서 단위 테스트
- GLB 헤더 파싱 테스트
- 최소 삼각형 GLB 바이트 수동 생성 roundtrip
- Accessor 타입별 (u16/u32, f32) 추출 테스트

---

## C. ECS 쿼리 필터 + 시스템 스케줄러

### C-1. 쿼리 필터 (`voltex_ecs/src/world.rs` 확장)

#### API
```rust
// 마커 타입
pub struct With<T>(PhantomData<T>);
pub struct Without<T>(PhantomData<T>);

// World 메서드 확장
impl World {
    // 단일 컴포넌트 + 필터
    pub fn query_with<T: 'static, W: 'static>(&self) -> Vec<(Entity, &T)>
    pub fn query_without<T: 'static, W: 'static>(&self) -> Vec<(Entity, &T)>

    // 2-컴포넌트 + 필터
    pub fn query2_with<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)>
    pub fn query2_without<A: 'static, B: 'static, W: 'static>(&self) -> Vec<(Entity, &A, &B)>
}
```

#### 구현
- 기존 query 결과에서 `has_component::<W>(entity)` 로 필터링
- 기존 query/query2/query3/query4는 그대로 유지 (하위 호환)

### C-2. 시스템 스케줄러 (`voltex_ecs/src/scheduler.rs` 신규)

#### API
```rust
pub trait System {
    fn run(&mut self, world: &mut World);
}

// fn(&mut World) → impl System 자동 변환
impl<F: FnMut(&mut World)> System for F { ... }

pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl Scheduler {
    pub fn new() -> Self
    pub fn add<S: System + 'static>(&mut self, system: S) -> &mut Self
    pub fn run_all(&mut self, world: &mut World)  // 등록 순서대로 실행
}
```

#### 구현
- 시스템은 등록 순서대로 순차 실행
- `fn(&mut World)` 클로저를 System으로 자동 변환
- 의존성 해석이나 병렬 실행 없음 (간단한 순서 기반)

### 테스트
- With/Without 필터 조합 검증
- 필터 + query2 조합
- 스케줄러 실행 순서 검증
- 빈 스케줄러, 단일/다중 시스템
