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
#[derive(Debug, Clone, Copy)]
pub struct TextureHandle(pub Handle<GpuTexture>);

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
}
