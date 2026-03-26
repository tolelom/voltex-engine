# GPU Instancing Design

## Overview

동일 메시를 수천 개 렌더링할 때 단일 draw call로 처리하는 인스턴싱 시스템. per-entity UBO 대신 인스턴스 버퍼에 모델 행렬을 패킹.

## Scope

- InstanceData 구조체 (모델 행렬 + 추가 per-instance 데이터)
- InstanceBuffer 관리 (동적 크기, 매 프레임 업로드)
- 인스턴스드 렌더 파이프라인 (기존 mesh_shader 확장)
- 기존 파이프라인과 병행 (인스턴싱은 옵션)

## InstanceData

```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4],  // 모델 행렬 (64 bytes)
    pub color: [f32; 4],       // per-instance 색상/틴트 (16 bytes)
}
// Total: 80 bytes per instance
```

셰이더에서 vertex attribute로 전달 (location 4~8).

## InstanceBuffer

```rust
pub struct InstanceBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,        // 현재 버퍼 용량 (인스턴스 수)
    pub count: usize,       // 실제 인스턴스 수
}
```

- `new(device, initial_capacity)` — VERTEX | COPY_DST 버퍼
- `update(device, queue, instances: &[InstanceData])` — 용량 초과 시 재생성, write_buffer
- 인스턴스 데이터는 CPU에서 매 프레임 빌드 (Transform 순회)

## Instanced Shader (instanced_shader.wgsl)

기존 mesh_shader.wgsl 기반 + 인스턴스 입력:

```wgsl
struct InstanceInput {
    @location(4) model_0: vec4<f32>,
    @location(5) model_1: vec4<f32>,
    @location(6) model_2: vec4<f32>,
    @location(7) model_3: vec4<f32>,
    @location(8) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(instance.model_0, instance.model_1, instance.model_2, instance.model_3);
    let world_pos = model * vec4<f32>(vertex.position, 1.0);
    // ...
}
```

## Pipeline

기존 create_mesh_pipeline에 instance attributes 추가:
- slot 0: MeshVertex (48 bytes, per-vertex)
- slot 1: InstanceData (80 bytes, per-instance, step_mode: Instance)

```rust
pub fn create_instanced_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    light_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline;
```

## Render

```rust
rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
rpass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));
rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
rpass.draw_indexed(0..mesh.num_indices, 0, 0..instance_buffer.count as u32);
```

## File Structure

- `crates/voltex_renderer/src/instancing.rs` — InstanceData, InstanceBuffer, pipeline, InstanceData::LAYOUT
- `crates/voltex_renderer/src/instanced_shader.wgsl` — 인스턴스드 셰이더
- `crates/voltex_renderer/src/lib.rs` — 모듈 추가

## Testing

- InstanceData 크기: size_of == 80
- InstanceData::LAYOUT: attribute 수, stride 검증
- InstanceBuffer: capacity 확장 로직 (GPU 없이 로직만)
- sort by mesh for batching (같은 메시끼리 모아서 단일 draw call)
