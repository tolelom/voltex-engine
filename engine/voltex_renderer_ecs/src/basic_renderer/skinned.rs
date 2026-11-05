//! Skinned meshes in `BasicRenderer`: GPU vertex skinning (Blinn-Phong or PBR)
//! and skinned shadow casters. The bone palette buffer is shared by the main
//! pass, the shadow casters and the PBR skinned group 0.

use voltex_ecs::{MaterialHandle, World, WorldTransform};
use voltex_math::Mat4;
use voltex_renderer::{
    create_skinned_shadow_pipeline, pipeline::create_skinned_mesh_pipeline, BonePalette,
    MaterialUniform, SkinnedCameraUniform,
};

use super::{
    dynamic_uniform_layout, single_buffer_bind_group, warn_overflow_once, BasicRenderer,
    MAX_SKINNED,
};
use crate::assets::{PbrTextures, RenderAssets, TextureHandle};
use crate::dynamic_uniform::DynamicUniformBuffer;
use crate::frame::{FrameTarget, FrameView};
use crate::items::Hidden;
use crate::scene_pass::begin_overlay_pass;
use crate::skinned::{SkinnedMeshHandle, SkinnedPose};

/// Per-skinned-entity render data gathered for one frame.
pub(super) struct SkinnedItem {
    pub(super) model: Mat4,
    pub(super) handle: SkinnedMeshHandle,
    pub(super) palette: BonePalette,
    pub(super) material: MaterialUniform,
    pub(super) texture: Option<TextureHandle>,
    pub(super) pbr: Option<PbrTextures>,
}

pub(super) struct SkinnedResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) cameras: DynamicUniformBuffer<SkinnedCameraUniform>,
    pub(super) camera_bind_group: wgpu::BindGroup,
    /// One `BonePalette` per skinned entity; shared by the Blinn-Phong pass,
    /// the shadow casters and the PBR skinned pass.
    pub(super) palettes: DynamicUniformBuffer<BonePalette>,
    pub(super) palette_bind_group: wgpu::BindGroup,
    // Skinned shadow caster: depth-only skinning pipeline + a light-space camera
    // UBO (reuses the palette buffer + skinned camera/palette layouts). Lets
    // skinned meshes write into the directional shadow map alongside static meshes.
    pub(super) shadow_pipeline: wgpu::RenderPipeline,
    pub(super) shadow_cameras: DynamicUniformBuffer<SkinnedCameraUniform>,
    pub(super) shadow_camera_bind_group: wgpu::BindGroup,
}

impl SkinnedResources {
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        alignment: u32,
    ) -> Self {
        let camera_layout = dynamic_uniform_layout(
            device,
            "BasicRenderer Skinned Camera BGL",
            std::mem::size_of::<SkinnedCameraUniform>() as u64,
        );
        let palette_layout = dynamic_uniform_layout(
            device,
            "BasicRenderer Bone Palette BGL",
            std::mem::size_of::<BonePalette>() as u64,
        );
        let pipeline =
            create_skinned_mesh_pipeline(device, target_format, &camera_layout, &palette_layout);
        let cameras = DynamicUniformBuffer::<SkinnedCameraUniform>::new(
            device,
            "BasicRenderer Skinned Camera Dynamic UBO",
            alignment,
            MAX_SKINNED,
        );
        let palettes = DynamicUniformBuffer::<BonePalette>::new(
            device,
            "BasicRenderer Bone Palette Dynamic UBO",
            alignment,
            MAX_SKINNED,
        );
        let camera_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer Skinned Camera BG",
            &camera_layout,
            cameras.binding(),
        );
        let palette_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer Bone Palette BG",
            &palette_layout,
            palettes.binding(),
        );

        // Skinned shadow caster: depth-only pipeline (reuses the skinned camera +
        // palette layouts) and a dedicated light-space camera UBO. The palette
        // buffer is shared — render() uploads identical pose data for the shadow
        // pass and the main skinned pass each frame.
        let shadow_pipeline =
            create_skinned_shadow_pipeline(device, &camera_layout, &palette_layout);
        let shadow_cameras = DynamicUniformBuffer::<SkinnedCameraUniform>::new(
            device,
            "BasicRenderer Skinned Shadow Camera Dynamic UBO",
            alignment,
            MAX_SKINNED,
        );
        let shadow_camera_bind_group = single_buffer_bind_group(
            device,
            "BasicRenderer Skinned Shadow Camera BG",
            &camera_layout,
            shadow_cameras.binding(),
        );
        Self {
            pipeline,
            cameras,
            camera_bind_group,
            palettes,
            palette_bind_group,
            shadow_pipeline,
            shadow_cameras,
            shadow_camera_bind_group,
        }
    }
}

/// Material for a skinned entity without a `MaterialHandle`: light grey,
/// non-metallic, roughness 0.6.
fn default_skinned_material() -> MaterialUniform {
    MaterialUniform::with_params([0.8, 0.8, 0.82, 1.0], 0.0, 0.6)
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
        let mut casters: Vec<(SkinnedMeshHandle, Mat4, BonePalette)> = world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<Hidden>(*e).is_none())
            .map(|(_, wt, handle, pose)| (*handle, wt.0, pose.palette()))
            .collect();
        casters.truncate(MAX_SKINNED);
        if casters.is_empty() {
            return Vec::new();
        }
        let cameras: Vec<SkinnedCameraUniform> = casters
            .iter()
            .map(|(_, model, _)| SkinnedCameraUniform {
                view_proj: light_vp.cols,
                model: model.cols,
            })
            .collect();
        let palettes: Vec<BonePalette> = casters.iter().map(|(_, _, palette)| *palette).collect();
        self.skinned.shadow_cameras.upload(queue, &cameras);
        self.skinned.palettes.upload(queue, &palettes);
        casters.into_iter().map(|(handle, _, _)| handle).collect()
    }

    /// Second pass: GPU vertex skinning for entities carrying both a
    /// [`SkinnedMeshHandle`] and a [`SkinnedPose`]. Draws over the 3D scene
    /// with `LoadOp::Load` (color + depth), so it composes with the
    /// non-skinned pass. Capped at [`MAX_SKINNED`] entities per frame.
    pub(super) fn render_skinned(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &FrameTarget<'_>,
        view: &FrameView,
        world: &mut World,
        assets: &RenderAssets,
    ) {
        let mut items = self.collect_skinned_items(world, assets);
        warn_overflow_once(&self.skinned_overflow_warned, "skinned meshes", items.len(), MAX_SKINNED);
        items.truncate(MAX_SKINNED);
        if items.is_empty() {
            return;
        }

        // Palettes are shared by both skinned paths.
        let palettes: Vec<BonePalette> = items.iter().map(|it| it.palette).collect();
        self.skinned.palettes.upload(queue, &palettes);

        if self.pbr_enabled {
            let maps_bind_groups =
                self.prepare_skinned_pbr(queue, assets, &items, view.view_proj, view.camera_pos);
            let mut pass = begin_overlay_pass(
                encoder,
                "BasicRenderer Skinned Pass",
                target.color_view,
                target.depth_view,
            );
            self.draw_skinned_pbr(&mut pass, assets, &items, &maps_bind_groups);
        } else {
            let cameras: Vec<SkinnedCameraUniform> = items
                .iter()
                .map(|it| SkinnedCameraUniform {
                    view_proj: view.view_proj.cols,
                    model: it.model.cols,
                })
                .collect();
            self.skinned.cameras.upload(queue, &cameras);
            let mut pass = begin_overlay_pass(
                encoder,
                "BasicRenderer Skinned Pass",
                target.color_view,
                target.depth_view,
            );
            self.draw_skinned_blinn_phong(&mut pass, assets, &items);
        }
    }

    /// Model, mesh, palette and material of every visible skinned entity
    /// (+ PBR maps when the PBR path is on).
    fn collect_skinned_items(&self, world: &World, assets: &RenderAssets) -> Vec<SkinnedItem> {
        world
            .query3::<WorldTransform, SkinnedMeshHandle, SkinnedPose>()
            .filter(|(e, _, _, _)| world.get::<Hidden>(*e).is_none())
            .map(|(e, wt, handle, pose)| {
                let material = world
                    .get::<MaterialHandle>(e)
                    .and_then(|h| assets.material(*h).copied())
                    .unwrap_or_else(default_skinned_material);
                SkinnedItem {
                    model: wt.0,
                    handle: *handle,
                    palette: pose.palette(),
                    material,
                    texture: world.get::<TextureHandle>(e).copied(),
                    pbr: if self.pbr_enabled {
                        world.get::<PbrTextures>(e).copied()
                    } else {
                        None
                    },
                }
            })
            .collect()
    }

    fn draw_skinned_blinn_phong(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        assets: &RenderAssets,
        items: &[SkinnedItem],
    ) {
        let skinned = &self.skinned;
        pass.set_pipeline(&skinned.pipeline);
        for (i, it) in items.iter().enumerate() {
            let Some(mesh) = assets.skinned_mesh(it.handle) else {
                continue;
            };
            pass.set_bind_group(0, &skinned.camera_bind_group, &[skinned.cameras.offset(i)]);
            pass.set_bind_group(1, &skinned.palette_bind_group, &[skinned.palettes.offset(i)]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }
}
