//! # voltex_ecs
//!
//! Entity Component System. Generation-checked entity ids + SparseSet
//! storage + scene (de)serialization + hierarchy + transform propagation.
//!
//! - [`Entity`] / [`EntityAllocator`] — generation-checked ids.
//! - [`World`] — primary container; stores type-erased components.
//! - [`SparseSet`] — single-type packed storage.
//! - [`Transform`] / [`WorldTransform`] / [`propagate_transforms`].
//! - [`Parent`] / [`Children`] hierarchy helpers.
//! - [`Component`] trait + [`ComponentRegistry`] for typed serialization.
//! - [`serialize_scene_text`] / [`deserialize_scene_text`] (`.vscn` text, Transform/
//!   Parent/Tag only) and [`serialize_scene_json`] / [`deserialize_scene_json`]
//!   (registry-driven, all components) +
//!   [`serialize_scene_binary`] / [`deserialize_scene_binary`] (VSCN binary).
//! - [`experimental`] — archetype store and system schedulers; implemented
//!   but not used by the engine runtime.

#![warn(missing_docs)]

pub mod binary_scene;
pub mod bundle;
pub mod component_registry;
pub mod entity;
pub mod error;
pub mod events;
pub mod experimental;
pub mod hierarchy;
pub mod json;
pub mod prefab;
pub mod render_components;
pub mod scene;
pub mod script_component;
pub mod spatial;
pub mod sparse_set;
pub mod transform;
pub mod world;
pub mod world_transform;

pub use binary_scene::{deserialize_scene_binary, serialize_scene_binary};
pub use bundle::Bundle;
pub use component_registry::{Component, ComponentRegistry};
pub use prefab::Prefabs;
pub use entity::{Entity, EntityAllocator};
pub use error::{EcsError, Result};
pub use events::Events;
pub use hierarchy::{
    add_child, despawn_recursive, remove_child, repair_hierarchy_cycles, roots, Children, Parent,
};
#[allow(deprecated)]
pub use hierarchy::validate_hierarchy;
pub use render_components::{Bounds, MaterialAsset, MaterialHandle, MeshAsset, MeshHandle};
pub use scene::{
    deserialize_scene_json, deserialize_scene_text, serialize_scene_json, serialize_scene_text, Tag,
};
#[allow(deprecated)]
pub use scene::{deserialize_scene, serialize_scene};
pub use script_component::ScriptComponent;
pub use sparse_set::SparseSet;
pub use spatial::SpatialGrid;
pub use transform::Transform;
pub use world::World;
pub use world_transform::{propagate_transforms, WorldTransform};

// Old paths of modules/items moved to `experimental` (deprecated). Nothing in
// the engine uses them.
mod deprecated_paths;
#[allow(deprecated)]
pub use deprecated_paths::*;
