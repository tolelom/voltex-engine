# Scene Viewport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Embed a 3D scene viewport inside the editor's docking panel with orbit camera controls and Blinn-Phong rendering.

**Architecture:** Render 3D scene to an offscreen texture (Rgba8Unorm + Depth32Float), then blit that texture to the surface as a textured quad within the viewport panel's rect. OrbitCamera provides view/projection. Reuse existing mesh_shader.wgsl pipeline for 3D rendering.

**Tech Stack:** Rust, wgpu 28.0, voltex_math (Vec3/Mat4), voltex_renderer (Mesh/MeshVertex/CameraUniform/LightUniform/pipeline), voltex_editor (docking/IMGUI)

**Spec:** `docs/superpowers/specs/2026-03-26-scene-viewport-design.md`

---

### Task 1: OrbitCamera (pure math, no GPU)

**Files:**
- Create: `crates/voltex_editor/src/orbit_camera.rs`
- Modify: `crates/voltex_editor/src/lib.rs`
- Modify: `crates/voltex_editor/Cargo.toml`

- [ ] **Step 1: Add voltex_math dependency**

In `crates/voltex_editor/Cargo.toml`, add under `[dependencies]`:
```toml
voltex_math.workspace = true
```

- [ ] **Step 2: Write failing tests**

Create `crates/voltex_editor/src/orbit_camera.rs` with tests:

```rust
use voltex_math::{Vec3, Mat4};

use std::f32::consts::PI;

const PITCH_LIMIT: f32 = PI / 2.0 - 0.01;
const MIN_DISTANCE: f32 = 0.5;
const MAX_DISTANCE: f32 = 50.0;
const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_FACTOR: f32 = 0.1;
const PAN_SENSITIVITY: f32 = 0.01;

pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_position() {
        let cam = OrbitCamera::new();
        let pos = cam.position();
        // Default: yaw=0, pitch=0.3, distance=5
        // pos = target + (d*cos(p)*sin(y), d*sin(p), d*cos(p)*cos(y))
        // yaw=0 → sin=0, cos=1 → x=0, z=d*cos(p)
        assert!((pos.x).abs() < 1e-3);
        assert!(pos.y > 0.0); // pitch > 0 → y > 0
        assert!(pos.z > 0.0); // cos(0)*cos(p) > 0
    }

    #[test]
    fn test_orbit_changes_yaw_pitch() {
        let mut cam = OrbitCamera::new();
        let old_yaw = cam.yaw;
        let old_pitch = cam.pitch;
        cam.orbit(100.0, 50.0);
        assert!((cam.yaw - old_yaw - 100.0 * ORBIT_SENSITIVITY).abs() < 1e-6);
        assert!((cam.pitch - old_pitch - 50.0 * ORBIT_SENSITIVITY).abs() < 1e-6);
    }

    #[test]
    fn test_pitch_clamped() {
        let mut cam = OrbitCamera::new();
        cam.orbit(0.0, 100000.0); // huge pitch
        assert!(cam.pitch <= PITCH_LIMIT);
        cam.orbit(0.0, -200000.0); // huge negative
        assert!(cam.pitch >= -PITCH_LIMIT);
    }

    #[test]
    fn test_zoom_changes_distance() {
        let mut cam = OrbitCamera::new();
        let d0 = cam.distance;
        cam.zoom(1.0); // scroll up → zoom in
        assert!(cam.distance < d0);
    }

    #[test]
    fn test_zoom_clamped() {
        let mut cam = OrbitCamera::new();
        cam.zoom(1000.0); // massive zoom in
        assert!(cam.distance >= MIN_DISTANCE);
        cam.zoom(-10000.0); // massive zoom out
        assert!(cam.distance <= MAX_DISTANCE);
    }

    #[test]
    fn test_pan_moves_target() {
        let mut cam = OrbitCamera::new();
        let t0 = cam.target;
        cam.pan(10.0, 0.0); // pan right
        assert!((cam.target.x - t0.x).abs() > 1e-4 || (cam.target.z - t0.z).abs() > 1e-4);
    }

    #[test]
    fn test_view_matrix_not_zero() {
        let cam = OrbitCamera::new();
        let v = cam.view_matrix();
        // At least some elements should be non-zero
        let sum: f32 = v.cols.iter().flat_map(|c| c.iter()).map(|x| x.abs()).sum();
        assert!(sum > 1.0);
    }

    #[test]
    fn test_projection_matrix() {
        let cam = OrbitCamera::new();
        let p = cam.projection_matrix(16.0 / 9.0);
        // Element [0][0] should be related to fov and aspect
        assert!(p.cols[0][0] > 0.0);
    }

    #[test]
    fn test_view_projection() {
        let cam = OrbitCamera::new();
        let vp = cam.view_projection(1.0);
        let v = cam.view_matrix();
        let p = cam.projection_matrix(1.0);
        let expected = p.mul_mat4(&v);
        for i in 0..4 {
            for j in 0..4 {
                assert!((vp.cols[i][j] - expected.cols[i][j]).abs() < 1e-4,
                    "mismatch at [{i}][{j}]: {} vs {}", vp.cols[i][j], expected.cols[i][j]);
            }
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p voltex_editor --lib orbit_camera`
Expected: FAIL (methods don't exist)

- [ ] **Step 4: Implement OrbitCamera**

```rust
impl OrbitCamera {
    pub fn new() -> Self {
        OrbitCamera {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.3,
            fov_y: PI / 4.0,
            near: 0.1,
            far: 100.0,
        }
    }

    pub fn position(&self) -> Vec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        Vec3::new(
            self.target.x + self.distance * cp * sy,
            self.target.y + self.distance * sp,
            self.target.z + self.distance * cp * cy,
        )
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * ORBIT_SENSITIVITY;
        self.pitch += dy * ORBIT_SENSITIVITY;
        self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance *= 1.0 - delta * ZOOM_FACTOR;
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let forward = (self.target - self.position()).normalize();
        let right = forward.cross(Vec3::Y);
        let right = if right.length() < 1e-4 { Vec3::X } else { right.normalize() };
        let up = right.cross(forward).normalize();
        let offset_x = right * (-dx * PAN_SENSITIVITY * self.distance);
        let offset_y = up * (dy * PAN_SENSITIVITY * self.distance);
        self.target = self.target + offset_x + offset_y;
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at(self.position(), self.target, Vec3::Y)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(self.fov_y, aspect, self.near, self.far)
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect).mul_mat4(&self.view_matrix())
    }
}
```

- [ ] **Step 5: Add module to lib.rs**

```rust
pub mod orbit_camera;
pub use orbit_camera::OrbitCamera;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p voltex_editor --lib orbit_camera -- --nocapture`
Expected: all 9 tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/voltex_editor/Cargo.toml crates/voltex_editor/src/orbit_camera.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): add OrbitCamera with orbit, zoom, pan controls"
```

---

### Task 2: ViewportTexture (offscreen render target)

**Files:**
- Create: `crates/voltex_editor/src/viewport_texture.rs`
- Modify: `crates/voltex_editor/src/lib.rs`

- [ ] **Step 1: Implement ViewportTexture**

Create `crates/voltex_editor/src/viewport_texture.rs`:

```rust
pub const VIEWPORT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const VIEWPORT_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct ViewportTexture {
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl ViewportTexture {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let w = width.max(1);
        let h = height.max(1);

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Color"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VIEWPORT_COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VIEWPORT_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        ViewportTexture { color_texture, color_view, depth_texture, depth_view, width: w, height: h }
    }

    /// Recreate textures if size changed. Returns true if recreated.
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let w = width.max(1);
        let h = height.max(1);
        if w == self.width && h == self.height {
            return false;
        }
        *self = Self::new(device, w, h);
        true
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

```rust
pub mod viewport_texture;
pub use viewport_texture::ViewportTexture;
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p voltex_editor`
Expected: compiles (no GPU tests possible, but struct/logic is trivial)

- [ ] **Step 4: Commit**

```bash
git add crates/voltex_editor/src/viewport_texture.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): add ViewportTexture offscreen render target"
```

---

### Task 3: ViewportRenderer (blit shader + pipeline)

**Files:**
- Create: `crates/voltex_editor/src/viewport_renderer.rs`
- Create: `crates/voltex_editor/src/viewport_shader.wgsl`
- Modify: `crates/voltex_editor/src/lib.rs`

- [ ] **Step 1: Write the WGSL shader**

Create `crates/voltex_editor/src/viewport_shader.wgsl`:

```wgsl
struct RectUniform {
    rect: vec4<f32>,    // x, y, w, h in pixels
    screen: vec2<f32>,  // screen_w, screen_h
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: RectUniform;
@group(0) @binding(1) var t_viewport: texture_2d<f32>;
@group(0) @binding(2) var s_viewport: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // 6 vertices for 2 triangles (fullscreen quad within rect)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 0.0),  // top-right
        vec2<f32>(1.0, 1.0),  // bottom-right
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 1.0),  // bottom-right
        vec2<f32>(0.0, 1.0),  // bottom-left
    );

    let p = positions[idx];
    // Convert pixel rect to NDC
    let px = u.rect.x + p.x * u.rect.z;
    let py = u.rect.y + p.y * u.rect.w;
    let ndc_x = (px / u.screen.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / u.screen.y) * 2.0;  // Y flipped

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = p;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_viewport, s_viewport, in.uv);
}
```

- [ ] **Step 2: Implement ViewportRenderer**

Create `crates/voltex_editor/src/viewport_renderer.rs`:

```rust
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct RectUniform {
    rect: [f32; 4],    // x, y, w, h
    screen: [f32; 2],  // screen_w, screen_h
    _pad: [f32; 2],
}

pub struct ViewportRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
}

impl ViewportRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Viewport Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("viewport_shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Viewport Blit BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Viewport Blit PL"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Viewport Blit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // no vertex buffer
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Viewport Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Viewport Rect Uniform"),
            size: std::mem::size_of::<RectUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ViewportRenderer { pipeline, bind_group_layout, sampler, uniform_buffer }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        viewport_color_view: &wgpu::TextureView,
        screen_w: f32,
        screen_h: f32,
        rect_x: f32,
        rect_y: f32,
        rect_w: f32,
        rect_h: f32,
    ) {
        let uniform = RectUniform {
            rect: [rect_x, rect_y, rect_w, rect_h],
            screen: [screen_w, screen_h],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        // Create bind group per frame (texture may change on resize)
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewport Blit BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(viewport_color_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Viewport Blit Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        // Scissor to panel area
        rpass.set_scissor_rect(rect_x as u32, rect_y as u32, rect_w.ceil() as u32, rect_h.ceil() as u32);
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..6, 0..1);
    }
}
```

- [ ] **Step 3: Add module to lib.rs**

```rust
pub mod viewport_renderer;
pub use viewport_renderer::ViewportRenderer;
```

- [ ] **Step 4: Build**

Run: `cargo build -p voltex_editor`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_editor/src/viewport_shader.wgsl crates/voltex_editor/src/viewport_renderer.rs crates/voltex_editor/src/lib.rs
git commit -m "feat(editor): add ViewportRenderer blit pipeline and shader"
```

---

### Task 4: Integrate into editor_demo

**Files:**
- Modify: `crates/voltex_editor/Cargo.toml`
- Modify: `examples/editor_demo/src/main.rs`

This is the largest task. The editor_demo needs:
- voltex_renderer dependency in voltex_editor for Mesh/MeshVertex/CameraUniform/LightUniform/pipeline
- A simple 3D scene (cubes + ground + light)
- Render scene to ViewportTexture, blit to surface, orbit camera input

- [ ] **Step 1: Add voltex_renderer dependency**

In `crates/voltex_editor/Cargo.toml`:
```toml
voltex_renderer.workspace = true
```

- [ ] **Step 2: Update editor_demo imports and state**

Read `examples/editor_demo/src/main.rs` first. Then add:

Imports:
```rust
use voltex_editor::{
    UiContext, UiRenderer, DockTree, DockNode, Axis, Rect, LayoutState,
    OrbitCamera, ViewportTexture, ViewportRenderer,
};
use voltex_renderer::{Mesh, MeshVertex, CameraUniform, LightUniform, GpuTexture};
```

Add to `AppState`:
```rust
// Viewport state
orbit_cam: OrbitCamera,
viewport_tex: ViewportTexture,
viewport_renderer: ViewportRenderer,
// 3D scene resources
scene_pipeline: wgpu::RenderPipeline,
camera_buffer: wgpu::Buffer,
light_buffer: wgpu::Buffer,
camera_light_bg: wgpu::BindGroup,
scene_meshes: Vec<(Mesh, [[f32; 4]; 4])>, // mesh + model matrix
dummy_texture: GpuTexture,
// Mouse tracking for orbit
prev_mouse: (f32, f32),
left_dragging: bool,
middle_dragging: bool,
```

- [ ] **Step 3: Initialize 3D scene in `resumed`**

After UI initialization, create:
1. OrbitCamera
2. ViewportTexture (initial size 640x480)
3. ViewportRenderer
4. Camera/Light bind group layout, buffers, bind group
5. Texture bind group layout + white 1x1 dummy texture
6. Mesh pipeline with Rgba8Unorm target format
7. Scene meshes: ground plane + 3 cubes (using MeshVertex)

The scene pipeline must use `VIEWPORT_COLOR_FORMAT` (Rgba8Unorm) not `surface_format`.

Helper to generate a cube mesh:
```rust
fn make_cube(device: &wgpu::Device) -> Mesh {
    // 24 vertices (4 per face, 6 faces), 36 indices
    // Each face: position, normal, uv=(0,0), tangent=(1,0,0,1)
    // ... standard unit cube centered at origin
}

fn make_ground(device: &wgpu::Device) -> Mesh {
    // 4 vertices forming a 10x10 quad at y=0
    // normal = (0,1,0)
}
```

- [ ] **Step 4: Handle mouse input for orbit camera**

In `window_event`:
- Track `prev_mouse` position for dx/dy calculation
- Track `left_dragging` and `middle_dragging` state from mouse button events
- Feed scroll delta for zoom

In the render loop, after getting viewport rect:
```rust
if rect.contains(mx, my) {
    let dx = mx - prev_mouse.0;
    let dy = my - prev_mouse.1;
    if left_dragging { orbit_cam.orbit(dx, dy); }
    if middle_dragging { orbit_cam.pan(dx, dy); }
    if scroll != 0.0 { orbit_cam.zoom(scroll); }
}
```

- [ ] **Step 5: Render 3D scene to viewport texture**

In the render loop, for viewport panel:
```rust
1 => {
    // Ensure viewport texture matches panel size
    viewport_tex.ensure_size(&gpu.device, rect.w as u32, rect.h as u32);

    // Update camera uniform
    let aspect = rect.w / rect.h;
    let vp = orbit_cam.view_projection(aspect);
    let cam_pos = orbit_cam.position();

    // Render each mesh to viewport texture
    // For each mesh: update model matrix in camera_uniform, write_buffer, draw
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &viewport_tex.color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.15, g: 0.15, b: 0.2, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &viewport_tex.depth_view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        rpass.set_pipeline(&scene_pipeline);
        rpass.set_bind_group(0, &camera_light_bg, &[]);
        rpass.set_bind_group(1, &dummy_texture.bind_group, &[]);

        for (mesh, _model) in &scene_meshes {
            rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }

    // Blit viewport texture to surface
    viewport_renderer.render(
        &gpu.device, &gpu.queue, &mut encoder, &surface_view,
        &viewport_tex.color_view,
        screen_w, screen_h,
        rect.x, rect.y, rect.w, rect.h,
    );
}
```

Note: For multiple meshes with different model matrices, we need to update the camera_buffer between draws. Since we can't write_buffer inside a render pass, we have two options:
- (A) Use a single model matrix (identity) and position cubes via vertex data
- (B) Use dynamic uniform buffer offsets

For simplicity, option (A): pre-transform cube vertices at different positions. The model matrix in CameraUniform stays identity.

- [ ] **Step 6: Build and test**

Run: `cargo build -p editor_demo`
Expected: compiles

Run: `cargo test -p voltex_editor -- --nocapture`
Expected: all tests pass (orbit_camera + dock tests)

- [ ] **Step 7: Commit**

```bash
git add crates/voltex_editor/Cargo.toml examples/editor_demo/src/main.rs
git commit -m "feat(editor): integrate 3D viewport into editor_demo"
```

---

### Task 5: Update docs

**Files:**
- Modify: `docs/dev/STATUS.md`
- Modify: `docs/dev/DEFERRED.md`

- [ ] **Step 1: Update STATUS.md**

Add to Phase 8-4:
```
- voltex_editor: ViewportTexture, ViewportRenderer, OrbitCamera (offscreen 3D viewport)
```

Update test count.

- [ ] **Step 2: Update DEFERRED.md**

```
- ~~**씬 뷰포트**~~ ✅ 오프스크린 렌더링 + 오빗 카메라 완료. Blinn-Phong forward만 (디퍼드/PBR 미지원).
```

- [ ] **Step 3: Commit**

```bash
git add docs/dev/STATUS.md docs/dev/DEFERRED.md
git commit -m "docs: update STATUS.md and DEFERRED.md with scene viewport"
```
