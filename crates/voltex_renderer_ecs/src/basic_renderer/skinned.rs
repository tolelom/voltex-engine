//! Skinned meshes in `BasicRenderer`: GPU vertex skinning (Blinn-Phong or PBR)
//! and skinned shadow casters. The bone palette buffer is shared by the main
//! pass, the shadow casters and the PBR skinned group 0.

use voltex_ecs::{MaterialHandle, World, WorldTransform};
use voltex_math::{Mat4, Vec3};
use voltex_renderer::{
    create_pbr_full_texture_bind_group, create_skinned_shadow_pipeline,
    pipeline::create_skinned_mesh_pipeline, BonePalette, CameraUniform,
    MaterialUniform, SkinnedCameraUniform,
};

use super::{dynamic_uniform_layout, warn_overflow_once, BasicRenderer, MAX_SKINNED};
use crate::assets::RenderAssets;
use crate::skinned::{SkinnedMeshHandle, SkinnedPose};

/// Per-skinned-entity render data gathered for one frame.
struct SkinnedItem {
    model: Mat4,
    handle: SkinnedMeshHandle,
    palette: BonePalette,
    material: MaterialUniform,
    texture: Option<crate::assets::TextureHandle>,
    pbr: Option<crate::assets::PbrTextures>,
}

pub(super) struct SkinnedResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) cam_buffer: wgpu::Buffer,
    pub(super) palette_buffer: wgpu::Buffer,
    pub(super) cam_bind_group: wgpu::BindGroup,
    pub(super) palette_bind_group: wgpu::BindGroup,
    pub(super) cam_aligned: u32,
    pub(super) palette_aligned: u32,
    // Skinned shadow caster: depth-only skinning pipeline + a light-space camera
    // UBO (reuses the palette buffer + skinned_cam/palette layouts). Lets skinned
    // meshes write into the directional shadow map alongside static meshes.
    pub(super) shadow_pipeline: wgpu::RenderPipeline,
    pub(super) shadow_cam_buffer: wgpu::Buffer,
    pub(super) shadow_cam_bind_group: wgpu::BindGroup,
}

impl SkinnedResources {
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        alignment: u32,
    ) -> Self {
        let skinned_cam_aligned =
            (std::mem::size_of::<SkinnedCameraUniform>() as u32).div_ceil(alignment) * alignment;
        let palette_aligned =
            (std::mem::size_of::<BonePalette>() as u32).div_ceil(alignment) * alignment;

        let skinned_cam_layout = dynamic_uniform_layout(
            device,
            "BasicRenderer Skinned Camera BGL",
            std::mem::size_of::<SkinnedCameraUniform>() as u64,
        );
        let palette_layout = dynamic_uniform_layout(
            device,
            "BasicRenderer Bone Palette BGL",
            std::mem::size_of::<BonePalette>() as u64,
        );
        let skinned_pipeline = create_skinned_mesh_pipeline(
            device,
            target_format,
            &skinned_cam_layout,
            &palette_layout,
        );
        let skinned_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Skinned Camera Dynamic UBO"),
            size: (skinned_cam_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palette_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Bone Palette Dynamic UBO"),
            size: (palette_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let skinned_cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Skinned Camera BG"),
            layout: &skinned_cam_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &skinned_cam_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<SkinnedCameraUniform>() as u64),
                }),
            }],
        });
        let palette_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Bone Palette BG"),
            layout: &palette_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &palette_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<BonePalette>() as u64),
                }),
            }],
        });

        // Skinned shadow caster: depth-only pipeline (reuses the skinned camera +
        // palette layouts) and a dedicated light-space camera UBO. The palette
        // buffer is shared — render() uploads identical pose data for the shadow
        // pass and the main skinned pass each frame.
        let skinned_shadow_pipeline =
            create_skinned_shadow_pipeline(device, &skinned_cam_layout, &palette_layout);
        let skinned_shadow_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BasicRenderer Skinned Shadow Camera Dynamic UBO"),
            size: (skinned_cam_aligned as usize * MAX_SKINNED) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let skinned_shadow_cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BasicRenderer Skinned Shadow Camera BG"),
            layout: &skinned_cam_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &skinned_shadow_cam_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<SkinnedCameraUniform>() as u64),
                }),
            }],
        });
        Self {
            pipeline: skinned_pipeline,
            cam_buffer: skinned_cam_buffer,
            palette_buffer,
            cam_bind_group: skinned_cam_bind_group,
            palette_bind_group,
            cam_aligned: skinned_cam_aligned,
            palette_aligned,
            shadow_pipeline: skinned_shadow_pipeline,
            shadow_cam_buffer: skinned_shadow_cam_buffer,
            shadow_cam_bind_group: skinned_shadow_cam_bind_group,
        }
    }
}

impl BasicRenderer {
    /// Upload skinned shadow-caster data (light-space camera + shared palette)
    /// and return the per-entity mesh handles in draw order (capped at
    /// [`MAX_SKINNED`]). Mirrors the query in [`Self::render_skinned`] so the
    /// same entities both cast and receive. The palette buffer is shared with
    /// the main skinned pass; both upload identical pose data each frame, so the
    /// last write (whichever pass runs second) is byte-identical.
    pub(super) fn gather_skinned_shadow(
        &self,
        queue: &wgpu::Queue,
        world: &World,
        light_vp: Mat4,
    ) -> Vec<SkinnedMeshHandle> {
        let items: Vec<(SkinnedMeshHandle, Mat4, voltex_renderer::BonePalette)> = world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(_, wt, handle, pose)| (*handle, wt.0, pose.palette()))
            .collect();
        let n = items.len().min(MAX_SKINNED);
        if n == 0 {
            return Vec::new();
        }
        let cam_aligned = self.skinned.cam_aligned as usize;
        let pal_aligned = self.skinned.palette_aligned as usize;
        let mut cam_staging = vec![0u8; n * cam_aligned];
        let mut pal_staging = vec![0u8; n * pal_aligned];
        for (i, (_, model, palette)) in items.iter().take(n).enumerate() {
            let cam = SkinnedCameraUniform { view_proj: light_vp.cols, model: model.cols };
            let cb = bytemuck::bytes_of(&cam);
            cam_staging[i * cam_aligned..i * cam_aligned + cb.len()].copy_from_slice(cb);
            let pb = bytemuck::bytes_of(palette);
            pal_staging[i * pal_aligned..i * pal_aligned + pb.len()].copy_from_slice(pb);
        }
        queue.write_buffer(&self.skinned.shadow_cam_buffer, 0, &cam_staging);
        queue.write_buffer(&self.skinned.palette_buffer, 0, &pal_staging);
        items.into_iter().take(n).map(|(h, _, _)| h).collect()
    }

    /// Second pass: GPU vertex skinning for entities carrying both a
    /// [`SkinnedMeshHandle`] and a [`SkinnedPose`]. Draws over the 3D scene
    /// with `LoadOp::Load` (color + depth), so it composes with the
    /// non-skinned pass. Capped at [`MAX_SKINNED`] entities per frame.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_skinned(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        world: &mut World,
        assets: &RenderAssets,
        view_proj: Mat4,
        cam_pos: Vec3,
    ) {
        // Per skinned entity: model, mesh, palette, + (PBR only) material maps.
        let pbr = self.pbr_enabled;
        let items: Vec<SkinnedItem> = world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<crate::items::Hidden>(*e).is_none())
            .map(|(e, wt, handle, pose)| {
                let material = world
                    .get::<MaterialHandle>(e)
                    .and_then(|h| assets.material(*h).copied())
                    .unwrap_or_else(|| {
                        MaterialUniform::with_params([0.8, 0.8, 0.82, 1.0], 0.0, 0.6)
                    });
                SkinnedItem {
                    model: wt.0,
                    handle: *handle,
                    palette: pose.palette(),
                    material,
                    texture: world.get::<crate::assets::TextureHandle>(e).copied(),
                    pbr: if pbr { world.get::<crate::assets::PbrTextures>(e).copied() } else { None },
                }
            })
            .collect();
        warn_overflow_once(&self.skinned_overflow_warned, "skinned meshes", items.len(), MAX_SKINNED);
        let n = items.len().min(MAX_SKINNED);
        if n == 0 {
            return;
        }

        // Palettes are shared by both skinned paths.
        let pal_aligned = self.skinned.palette_aligned as usize;
        let mut pal_staging = vec![0u8; n * pal_aligned];
        for (i, it) in items.iter().take(n).enumerate() {
            let pb = bytemuck::bytes_of(&it.palette);
            pal_staging[i * pal_aligned..i * pal_aligned + pb.len()].copy_from_slice(pb);
        }
        queue.write_buffer(&self.skinned.palette_buffer, 0, &pal_staging);

        // Per-entity camera uniform: PBR uses CameraUniform (w/ camera_pos), the
        // colored path uses SkinnedCameraUniform.
        let cam_aligned = if self.pbr_enabled {
            let a = self.pbr_skinned_cam_aligned as usize;
            let mut staging = vec![0u8; n * a];
            for (i, it) in items.iter().take(n).enumerate() {
                let cam = CameraUniform {
                    view_proj: view_proj.cols,
                    model: it.model.cols,
                    camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
                    _padding: 0.0,
                };
                let cb = bytemuck::bytes_of(&cam);
                staging[i * a..i * a + cb.len()].copy_from_slice(cb);
            }
            queue.write_buffer(&self.pbr_skinned_cam_buffer, 0, &staging);
            a
        } else {
            let a = self.skinned.cam_aligned as usize;
            let mut staging = vec![0u8; n * a];
            for (i, it) in items.iter().take(n).enumerate() {
                let cam = SkinnedCameraUniform {
                    view_proj: view_proj.cols,
                    model: it.model.cols,
                };
                let cb = bytemuck::bytes_of(&cam);
                staging[i * a..i * a + cb.len()].copy_from_slice(cb);
            }
            queue.write_buffer(&self.skinned.cam_buffer, 0, &staging);
            a
        };

        // PBR skinned: stage per-entity material + build per-entity texture
        // bind groups (defaults for absent maps), mirroring the static pass.
        let mat_aligned = self.pbr_skinned_material_aligned as usize;
        let skinned_tex_bgs: Vec<Option<wgpu::BindGroup>> = if self.pbr_enabled {
            let mut mat_staging = vec![0u8; n * mat_aligned];
            for (i, it) in items.iter().take(n).enumerate() {
                let mb = bytemuck::bytes_of(&it.material);
                mat_staging[i * mat_aligned..i * mat_aligned + mb.len()].copy_from_slice(mb);
            }
            queue.write_buffer(&self.pbr_skinned_material_buffer, 0, &mat_staging);

            let resolve = |h: Option<crate::assets::TextureHandle>| {
                h.and_then(|h| assets.texture(h)).map(|t| (&t.view, &t.sampler))
            };
            items
                .iter()
                .take(n)
                .map(|it| {
                    let has_maps = it.texture.is_some()
                        || it.pbr.map(|p| p.normal.or(p.orm).or(p.emissive).is_some())
                            == Some(true);
                    if !has_maps {
                        return None;
                    }
                    let (av, asamp) = resolve(it.texture)
                        .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                    let (nv, nsamp) = resolve(it.pbr.and_then(|p| p.normal))
                        .unwrap_or((&self.pbr_flat_normal.1, &self.pbr_flat_normal.2));
                    let (ov, osamp) = resolve(it.pbr.and_then(|p| p.orm))
                        .unwrap_or((&self.default_texture.view, &self.default_texture.sampler));
                    let (ev, esamp) = resolve(it.pbr.and_then(|p| p.emissive))
                        .unwrap_or((&self.pbr_black.view, &self.pbr_black.sampler));
                    Some(create_pbr_full_texture_bind_group(
                        &self.device, &self.pbr_tex_layout, av, asamp, nv, nsamp, ov, osamp, ev,
                        esamp,
                    ))
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BasicRenderer Skinned Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        if self.pbr_enabled {
            pass.set_pipeline(&self.pbr_skinned_pipeline);
            pass.set_bind_group(3, &self.pbr_shadow_bind_group, &[]);
            for (i, it) in items.iter().take(n).enumerate() {
                let Some(mesh) = assets.skinned_mesh(it.handle) else {
                    continue;
                };
                let cam_off = (i * cam_aligned) as u32;
                let pal_off = (i * pal_aligned) as u32;
                let mat_off = (i * mat_aligned) as u32;
                // g0 has two dynamic bindings (camera @0, palette @2).
                pass.set_bind_group(0, &self.pbr_skinned_g0_bind_group, &[cam_off, pal_off]);
                let tex_bg = skinned_tex_bgs[i]
                    .as_ref()
                    .unwrap_or(&self.pbr_default_texture_bind_group);
                pass.set_bind_group(1, tex_bg, &[]);
                pass.set_bind_group(2, &self.pbr_skinned_material_bind_group, &[mat_off]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        } else {
            pass.set_pipeline(&self.skinned.pipeline);
            for (i, it) in items.iter().take(n).enumerate() {
                let Some(mesh) = assets.skinned_mesh(it.handle) else {
                    continue;
                };
                let cam_off = (i * cam_aligned) as u32;
                let pal_off = (i * pal_aligned) as u32;
                pass.set_bind_group(0, &self.skinned.cam_bind_group, &[cam_off]);
                pass.set_bind_group(1, &self.skinned.palette_bind_group, &[pal_off]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        }
    }
}
