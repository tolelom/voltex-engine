# glTF Animation/Skin Parser Extension Design

## Overview

기존 gltf.rs 파서를 확장하여 animations, skins, nodes 데이터를 파싱한다. 렌더링/애니메이션 시스템은 별도 구현.

## Scope

- nodes 배열 파싱 (스켈레톤 계층, transform)
- skins 배열 파싱 (joints, inverse bind matrices)
- animations 배열 파싱 (channels, samplers, keyframes)
- JOINTS_0/WEIGHTS_0 버텍스 어트리뷰트 추출
- 기존 메시/머티리얼 파싱에 영향 없음

## Data Structures

### GltfNode
```rust
pub struct GltfNode {
    pub name: Option<String>,
    pub children: Vec<usize>,
    pub translation: [f32; 3],  // 기본 [0,0,0]
    pub rotation: [f32; 4],     // 쿼터니언 [x,y,z,w], 기본 [0,0,0,1]
    pub scale: [f32; 3],        // 기본 [1,1,1]
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
}
```

### GltfSkin
```rust
pub struct GltfSkin {
    pub name: Option<String>,
    pub joints: Vec<usize>,
    pub inverse_bind_matrices: Vec<[[f32; 4]; 4]>,
    pub skeleton: Option<usize>,
}
```

### GltfAnimation
```rust
pub struct GltfAnimation {
    pub name: Option<String>,
    pub channels: Vec<GltfChannel>,
}

pub struct GltfChannel {
    pub target_node: usize,
    pub target_path: AnimationPath,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub values: Vec<f32>,  // flat: vec3이면 len = times.len()*3, quat이면 *4
}

pub enum AnimationPath {
    Translation,
    Rotation,
    Scale,
}

pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}
```

### GltfData 확장
```rust
pub struct GltfData {
    pub meshes: Vec<GltfMesh>,
    pub nodes: Vec<GltfNode>,
    pub skins: Vec<GltfSkin>,
    pub animations: Vec<GltfAnimation>,
}
```

### GltfMesh 확장
```rust
pub struct GltfMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub name: Option<String>,
    pub material: Option<GltfMaterial>,
    pub joints: Option<Vec<[u16; 4]>>,   // JOINTS_0 (스킨드 메시만)
    pub weights: Option<Vec<[f32; 4]>>,  // WEIGHTS_0 (스킨드 메시만)
}
```

## Parsing Logic

### nodes
- `"nodes"` JSON 배열 순회
- 각 노드에서 translation(vec3), rotation(vec4 quat), scale(vec3) 추출 (없으면 기본값)
- children(배열), mesh(정수), skin(정수) 추출

### skins
- `"skins"` JSON 배열 순회
- joints: 정수 배열 → 노드 인덱스
- inverseBindMatrices: accessor 인덱스 → mat4 배열 추출 (16 floats per joint)
- skeleton: 정수 (루트 노드)

### animations
- `"animations"` 배열의 각 애니메이션:
  - samplers 배열 파싱: input(시간 accessor), output(값 accessor), interpolation(문자열)
  - channels 배열 파싱: sampler 인덱스, target.node, target.path(문자열)
  - 각 채널을 GltfChannel로 조합: sampler의 input→times, output→values, interpolation

### Accessor 추출 확장
- 기존 `extract_accessor_f32` 재사용 (times, values, inverse bind matrices)
- 새로 `extract_accessor_u16`/`extract_accessor_u8` 추가 (JOINTS_0용)
- WEIGHTS_0: `extract_accessor_f32` 재사용

### JOINTS_0/WEIGHTS_0
- 메시 프리미티브의 attributes에서 "JOINTS_0", "WEIGHTS_0" 키 확인
- JOINTS_0: 보통 UNSIGNED_BYTE(5121) 또는 UNSIGNED_SHORT(5123) 타입
- WEIGHTS_0: FLOAT(5126) 타입
- 결과를 GltfMesh의 joints/weights 필드에 저장

## File Structure

- `crates/voltex_renderer/src/gltf.rs` — 기존 파일 확장

## Testing

실제 스킨드 GLB 파일이 필요하므로 테스트 전략:

### 단위 테스트 (바이트 구성 어려움 → 로직 위주)
- AnimationPath/Interpolation 문자열 파싱 테스트
- 기존 파싱이 깨지지 않는 회귀 테스트 (기존 테스트 GLB 그대로 통과)

### 통합 테스트 (실제 GLB)
- glTF 샘플 파일 사용 (assets/test/ 에 작은 스킨드 GLB 포함)
- 또는 테스트에서 간단한 GLB를 프로그래밍적으로 구성 (기존 테스트 패턴 따라)
- nodes 수, skins 수, animations 수, channels 수 검증
- inverse bind matrices 배열 크기 = joints 수
- times/values 배열이 비어있지 않은지 확인
