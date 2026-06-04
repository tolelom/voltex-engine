//! RenderAssets — voltex_renderer::Mesh + MaterialUniform 저장소.
//!
//! C0a 의 phantom-typed `MeshHandle(Handle<MeshAsset>)` 의 id/generation 을
//! 내부 `Handle<voltex_renderer::Mesh>` 와 lockstep 으로 유지한다. add_mesh /
//! add_material 가 유일한 등록 진입점인 한 이 invariant 가 깨지지 않는다.

use voltex_asset::{Assets, Handle};
use voltex_ecs::{MaterialHandle, MeshHandle};
use voltex_renderer::{GpuTexture, MaterialUniform, Mesh, SkinnedMesh};

use crate::skinned::SkinnedMeshHandle;

/// ECS component: the GPU texture an entity samples in `BasicRenderer`'s mesh
/// shader. Absent / stale → the renderer falls back to its white default.
/// In the PBR path this is the **albedo** map.
#[derive(Debug, Clone, Copy)]
pub struct TextureHandle(pub Handle<GpuTexture>);

/// ECS component: the source *file path* of the texture an entity's
/// [`TextureHandle`] was loaded from. Unlike the handle (a runtime
/// `RenderAssets` slot, meaningless across runs), this **persists in scenes**
/// — registered components serialize it, and an integration layer (editor /
/// runtime) re-loads the file and re-attaches a fresh handle after scene load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexturePath(pub String);

impl voltex_ecs::Component for TexturePath {
    const NAME: &'static str = "texture_path";

    fn serialize(world: &voltex_ecs::World, entity: voltex_ecs::Entity) -> Option<Vec<u8>> {
        let p = world.get::<TexturePath>(entity)?;
        Some(p.0.as_bytes().to_vec())
    }

    fn deserialize(
        world: &mut voltex_ecs::World,
        entity: voltex_ecs::Entity,
        data: &[u8],
    ) -> voltex_ecs::Result<()> {
        let s = std::str::from_utf8(data)
            .map_err(|_| "texture_path: invalid UTF-8")?
            .to_string();
        world.add(entity, TexturePath(s));
        Ok(())
    }
}

/// ECS component: extra PBR material maps for the runtime PBR pass (used only
/// when `BasicRenderer` has PBR enabled). Each is optional; an absent map falls
/// back to a sensible default (flat normal, white ORM = unoccluded/rough/
/// non-metal, black emissive). The albedo map is the entity's [`TextureHandle`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PbrTextures {
    /// Tangent-space normal map (linear). Absent → flat `(0,0,1)`.
    pub normal: Option<TextureHandle>,
    /// Packed occlusion(R)/roughness(G)/metallic(B) map (linear). Absent → white.
    pub orm: Option<TextureHandle>,
    /// Emissive map. Absent → black (no emission).
    pub emissive: Option<TextureHandle>,
}

/// GPU-resident mesh + material storage. Hands out the phantom-typed
/// [`MeshHandle`] / [`MaterialHandle`] ECS components while keeping the
/// underlying `Handle<Mesh>` / `Handle<MaterialUniform>` ids in lockstep.
pub struct RenderAssets {
    assets: Assets,
}

impl RenderAssets {
    /// Creates an empty asset store. `add_mesh` / `add_material` populate it.
    pub fn new() -> Self {
        Self { assets: Assets::new() }
    }

    /// Register a GPU mesh and return a C0a `MeshHandle` whose id/generation
    /// match the internal `Handle<Mesh>`.
    pub fn add_mesh(&mut self, mesh: Mesh) -> MeshHandle {
        let h: Handle<Mesh> = self.assets.insert(mesh);
        MeshHandle(Handle::from_raw_parts(h.id(), h.generation()))
    }

    /// Borrows the underlying `voltex_renderer::Mesh` for `h`, or `None` if stale.
    pub fn mesh(&self, h: MeshHandle) -> Option<&Mesh> {
        let raw: Handle<Mesh> = Handle::from_raw_parts(h.0.id(), h.0.generation());
        self.assets.get(raw)
    }

    /// Hot-reload: replace the GPU `Mesh` *behind an existing* [`MeshHandle`]
    /// in place. The handle's id/generation are preserved, so every entity
    /// already holding it draws the new geometry on the next render — no
    /// re-attach needed. Decode with [`Mesh::from_obj`] / [`Mesh::from_gltf`]
    /// first; this only swaps the uploaded mesh. Errors if `handle` is stale
    /// (its slot was freed/reused).
    pub fn replace_mesh(&mut self, handle: MeshHandle, mesh: Mesh) -> crate::Result<()> {
        let raw: Handle<Mesh> = Handle::from_raw_parts(handle.0.id(), handle.0.generation());
        let replaced = self.assets.storage_mut::<Mesh>().replace_in_place(raw, mesh);
        if replaced {
            Ok(())
        } else {
            Err(crate::RendererEcsError::from(
                "replace_mesh: stale MeshHandle".to_string(),
            ))
        }
    }

    /// Register a material (PBR params) and return a C0a `MaterialHandle`.
    pub fn add_material(&mut self, m: MaterialUniform) -> MaterialHandle {
        let h: Handle<MaterialUniform> = self.assets.insert(m);
        MaterialHandle(Handle::from_raw_parts(h.id(), h.generation()))
    }

    /// Borrows the underlying `MaterialUniform` for `h`, or `None` if stale.
    pub fn material(&self, h: MaterialHandle) -> Option<&MaterialUniform> {
        let raw: Handle<MaterialUniform> = Handle::from_raw_parts(h.0.id(), h.0.generation());
        self.assets.get(raw)
    }

    /// Register a GPU [`SkinnedMesh`] and return its [`SkinnedMeshHandle`].
    pub fn add_skinned_mesh(&mut self, mesh: SkinnedMesh) -> SkinnedMeshHandle {
        SkinnedMeshHandle(self.assets.insert(mesh))
    }

    /// Borrows the underlying `SkinnedMesh` for `h`, or `None` if stale.
    pub fn skinned_mesh(&self, h: SkinnedMeshHandle) -> Option<&SkinnedMesh> {
        self.assets.get(h.0)
    }

    /// Register a GPU texture (built against [`GpuTexture::bind_group_layout`])
    /// and return its [`TextureHandle`].
    pub fn add_texture(&mut self, texture: GpuTexture) -> TextureHandle {
        TextureHandle(self.assets.insert(texture))
    }

    /// Decode a PNG and upload it as a diffuse texture in one call, returning a
    /// [`TextureHandle`] ready to attach to a mesh or [`Sprite`](crate::Sprite).
    /// The texture bind-group layout is created internally. Errors if the PNG
    /// fails to decode.
    pub fn add_texture_png(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        png_bytes: &[u8],
    ) -> crate::Result<TextureHandle> {
        let layout = GpuTexture::bind_group_layout(device);
        let tex = GpuTexture::from_png(device, queue, png_bytes, &layout)
            .map_err(crate::RendererEcsError::from)?;
        Ok(self.add_texture(tex))
    }

    /// Hot-reload: decode `png_bytes` and replace the `GpuTexture` *behind an
    /// existing* [`TextureHandle`] in place. The handle's id/generation are
    /// preserved, so every entity already holding it samples the new texture on
    /// the next render — no re-attach needed. Errors if the PNG fails to decode
    /// or `handle` is stale (its slot was freed/reused).
    pub fn replace_texture_png(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: TextureHandle,
        png_bytes: &[u8],
    ) -> crate::Result<()> {
        let layout = GpuTexture::bind_group_layout(device);
        let tex = GpuTexture::from_png(device, queue, png_bytes, &layout)
            .map_err(crate::RendererEcsError::from)?;
        let replaced = self
            .assets
            .storage_mut::<GpuTexture>()
            .replace_in_place(handle.0, tex);
        if replaced {
            Ok(())
        } else {
            Err(crate::RendererEcsError::from(
                "replace_texture_png: stale TextureHandle".to_string(),
            ))
        }
    }

    /// Upload raw RGBA8 pixels (`width * height * 4` bytes, row-major) as a
    /// diffuse texture and return its [`TextureHandle`]. Handy for generated
    /// atlases / solid swatches without a PNG round-trip.
    pub fn add_texture_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> TextureHandle {
        let layout = GpuTexture::bind_group_layout(device);
        let tex = GpuTexture::from_rgba(device, queue, width, height, pixels, &layout);
        self.add_texture(tex)
    }

    /// Like [`Self::add_texture_png`] but uploads as **linear** (no sRGB decode)
    /// — for PBR normal / ORM maps attached via [`PbrTextures`]. Color/albedo
    /// maps should use the sRGB `add_texture_png`.
    pub fn add_texture_png_linear(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        png_bytes: &[u8],
    ) -> crate::Result<TextureHandle> {
        let layout = GpuTexture::bind_group_layout(device);
        let (pixels, w, h) =
            voltex_renderer::parse_png(png_bytes).map_err(|e| crate::RendererEcsError::from(e.to_string()))?;
        let tex = GpuTexture::from_rgba_linear(device, queue, w, h, &pixels, &layout);
        Ok(self.add_texture(tex))
    }

    /// Linear-format variant of [`Self::add_texture_rgba`] for normal / ORM
    /// maps (no sRGB decode applied).
    pub fn add_texture_rgba_linear(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> TextureHandle {
        let layout = GpuTexture::bind_group_layout(device);
        let tex = GpuTexture::from_rgba_linear(device, queue, width, height, pixels, &layout);
        self.add_texture(tex)
    }

    /// Borrows the underlying `GpuTexture` for `h`, or `None` if stale.
    pub fn texture(&self, h: TextureHandle) -> Option<&GpuTexture> {
        self.assets.get(h.0)
    }
}

impl Default for RenderAssets {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: add_mesh roundtrip needs a wgpu::Device → can't be unit-tested
    // headlessly. Visual verification via editor_demo covers that path.

    #[test]
    fn add_material_roundtrip() {
        let mut assets = RenderAssets::new();
        let m = MaterialUniform::with_params([0.5, 0.2, 0.1, 1.0], 0.3, 0.7);
        let h = assets.add_material(m);
        let got = assets.material(h).expect("material present after add_material");
        assert!((got.base_color[0] - 0.5).abs() < 1e-6);
        assert!((got.metallic - 0.3).abs() < 1e-6);
        assert!((got.roughness - 0.7).abs() < 1e-6);
    }

    #[test]
    fn missing_material_returns_none() {
        let assets = RenderAssets::new();
        let stale = MaterialHandle(Handle::from_raw_parts(99, 0));
        assert!(assets.material(stale).is_none());
    }

    #[test]
    fn missing_mesh_returns_none() {
        let assets = RenderAssets::new();
        let stale = MeshHandle(Handle::from_raw_parts(99, 0));
        assert!(assets.mesh(stale).is_none());
    }

    #[test]
    fn texture_path_round_trips_through_scene_json() {
        use voltex_ecs::{deserialize_scene_json, serialize_scene_json, Transform};
        use voltex_math::Vec3;

        let mut registry = voltex_ecs::ComponentRegistry::new();
        registry.register::<Transform>();
        crate::register_renderable_components(&mut registry);

        let mut world = voltex_ecs::World::new();
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
        world.add(e, TexturePath("assets/hero.png".to_string()));

        let json = serialize_scene_json(&world, &registry);
        let mut loaded = voltex_ecs::World::new();
        deserialize_scene_json(&mut loaded, &json, &registry).expect("scene loads");

        let restored: Vec<&TexturePath> = loaded
            .query::<TexturePath>()
            .map(|(_, p)| p)
            .collect();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].0, "assets/hero.png", "path survives save/load");
    }

    #[test]
    fn texture_path_deserialize_rejects_invalid_utf8() {
        use voltex_ecs::Component;
        let mut world = voltex_ecs::World::new();
        let e = world.spawn();
        assert!(TexturePath::deserialize(&mut world, e, &[0xFF, 0xFE]).is_err());
        assert!(world.get::<TexturePath>(e).is_none());
    }
}
