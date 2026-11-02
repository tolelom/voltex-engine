//! Text/JSON scene (de)serialization. The binary VSCN format lives in
//! [`binary_scene`](crate::binary_scene).

use crate::component_registry::ComponentRegistry;
use crate::entity::Entity;
use crate::hierarchy::{add_child, validate_hierarchy, Parent};
use crate::json::{self, JsonVal};
use crate::transform::Transform;
use crate::world::World;
use std::collections::HashMap;
use voltex_math::{Quat, Vec3};

/// String tag for entity identification.
#[derive(Debug, Clone)]
pub struct Tag(pub String);

impl World {
    /// The first live entity whose [`Tag`] equals `tag`, or `None`. Replaces the
    /// ubiquitous `world.query::<Tag>().find(|(_, t)| t.0 == name).map(..)`
    /// one-liner. O(n) over tagged entities — fine for the handful of named
    /// singletons (player, camera target) games look up each frame.
    pub fn find_by_tag(&self, tag: &str) -> Option<Entity> {
        self.query::<Tag>()
            .find(|(_, t)| t.0 == tag)
            .map(|(e, _)| e)
    }

    /// Every live entity whose [`Tag`] equals `tag`, in iteration order.
    pub fn iter_by_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = Entity> + 'a {
        self.query::<Tag>()
            .filter(move |(_, t)| t.0 == tag)
            .map(|(e, _)| e)
    }
}

/// Parse three space-separated f32 values into a Vec3.
fn parse_vec3(s: &str) -> Option<Vec3> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    let x = parts[0].parse::<f32>().ok()?;
    let y = parts[1].parse::<f32>().ok()?;
    let z = parts[2].parse::<f32>().ok()?;
    Some(Vec3::new(x, y, z))
}

/// Parse a transform line of the form
/// "px py pz | qx qy qz qw | sx sy sz" (current quaternion layout) or
/// "px py pz | rx ry rz       | sx sy sz" (legacy Euler YXZ, accepted for
/// backward-compatible `.vscn` loading).
fn parse_transform(s: &str) -> Option<Transform> {
    let parts: Vec<&str> = s.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let position = parse_vec3(parts[0].trim())?;
    let rotation = parse_rotation(parts[1].trim())?;
    let scale = parse_vec3(parts[2].trim())?;
    Some(Transform {
        position,
        rotation,
        scale,
    })
}

/// Parse rotation as either a 4-float quaternion (current) or a 3-float
/// YXZ Euler triple (legacy `.vscn` files).
fn parse_rotation(s: &str) -> Option<Quat> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.len() {
        4 => {
            let x = parts[0].parse::<f32>().ok()?;
            let y = parts[1].parse::<f32>().ok()?;
            let z = parts[2].parse::<f32>().ok()?;
            let w = parts[3].parse::<f32>().ok()?;
            Some(Quat::new(x, y, z, w))
        }
        3 => {
            let x = parts[0].parse::<f32>().ok()?;
            let y = parts[1].parse::<f32>().ok()?;
            let z = parts[2].parse::<f32>().ok()?;
            Some(Quat::from_euler_yxz(Vec3::new(x, y, z)))
        }
        _ => None,
    }
}

/// Serialize all entities with a Transform component to the `.vscn` text format.
///
/// # Limitations
///
/// This human-readable format only persists the built-in `Transform`, `Parent`,
/// and `Tag` components. It does **not** consult the [`ComponentRegistry`], so
/// `MeshHandle`, `MaterialHandle`, `Bounds`, `ScriptComponent`, and any
/// user-registered components are **silently omitted**. For lossless,
/// registry-driven persistence of arbitrary components use
/// [`serialize_scene_json`] / [`deserialize_scene_json`] or the binary
/// [`serialize_scene_binary`](crate::serialize_scene_binary) path instead.
pub fn serialize_scene(world: &World) -> String {
    // Collect all entities with Transform
    let entities_with_transform: Vec<(Entity, Transform)> =
        world.query::<Transform>().map(|(e, t)| (e, *t)).collect();

    // Build entity -> local index map
    let entity_to_index: HashMap<Entity, usize> = entities_with_transform
        .iter()
        .enumerate()
        .map(|(i, (e, _))| (*e, i))
        .collect();

    let mut output = String::from("# Voltex Scene v1\n");

    for (local_idx, (entity, transform)) in entities_with_transform.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("entity {}\n", local_idx));

        // Transform line — quaternion (4 floats) since E3b.
        let p = transform.position;
        let r = transform.rotation;
        let s = transform.scale;
        output.push_str(&format!(
            "  transform {} {} {} | {} {} {} {} | {} {} {}\n",
            p.x, p.y, p.z, r.x, r.y, r.z, r.w, s.x, s.y, s.z
        ));

        // Parent line (if entity has a Parent)
        if let Some(parent_comp) = world.get::<Parent>(*entity) {
            if let Some(&parent_local_idx) = entity_to_index.get(&parent_comp.0) {
                output.push_str(&format!("  parent {}\n", parent_local_idx));
            }
        }

        // Tag line (if entity has a Tag)
        if let Some(tag) = world.get::<Tag>(*entity) {
            output.push_str(&format!("  tag {}\n", tag.0));
        }
    }

    output
}

/// Maximum entity index we'll honour while parsing a `.vscn` text scene.
/// A malicious or corrupted file declaring `entity 4294967295` would
/// otherwise force the loader to push billions of placeholder slots
/// (OOM). This cap is far above any realistic hand-authored scene.
const MAX_SCENE_ENTITIES: usize = 1_000_000;

/// Parse a .vscn string, create entities in the world, and return the created entities.
///
/// Only `Transform`, `Parent`, and `Tag` are restored — see
/// [`serialize_scene`] for why this text format is lossy for other
/// components, and which paths to use for lossless persistence.
///
/// Robustness: entity indices and parent indices that exceed
/// [`MAX_SCENE_ENTITIES`] are silently skipped; out-of-range parent
/// indices that reference an entity outside the created set are
/// silently ignored. The resulting world is therefore always
/// consistent, even on adversarial input.
pub fn deserialize_scene(world: &mut World, source: &str) -> Vec<Entity> {
    // Intermediate storage: local_index -> (transform, tag, parent_local_index)
    let mut local_transforms: Vec<Option<Transform>> = Vec::new();
    let mut local_tags: Vec<Option<String>> = Vec::new();
    let mut local_parents: Vec<Option<usize>> = Vec::new();

    let mut current_index: Option<usize> = None;

    for line in source.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("entity ") {
            let idx: usize = rest.trim().parse().unwrap_or(local_transforms.len());
            // Reject indices that would expand storage beyond the cap.
            // Without this, an adversarial `entity 4294967295` line forced a
            // multi-GB Vec allocation.
            if idx >= MAX_SCENE_ENTITIES {
                current_index = None;
                continue;
            }
            // Ensure vectors are large enough
            while local_transforms.len() <= idx {
                local_transforms.push(None);
                local_tags.push(None);
                local_parents.push(None);
            }
            current_index = Some(idx);
        } else if let Some(rest) = trimmed.strip_prefix("transform ") {
            if let Some(idx) = current_index {
                if let Some(t) = parse_transform(rest) {
                    local_transforms[idx] = Some(t);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("parent ") {
            if let Some(idx) = current_index {
                if let Ok(parent_idx) = rest.trim().parse::<usize>() {
                    local_parents[idx] = Some(parent_idx);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("tag ") {
            if let Some(idx) = current_index {
                local_tags[idx] = Some(rest.trim().to_string());
            }
        }
    }

    // Create entities
    let mut created: Vec<Entity> = Vec::with_capacity(local_transforms.len());
    for i in 0..local_transforms.len() {
        let entity = world.spawn();
        // Add transform (default if not present)
        let transform = local_transforms[i].unwrap_or_default();
        world.add(entity, transform);
        // Add tag if present
        if let Some(ref tag_str) = local_tags[i] {
            world.add(entity, Tag(tag_str.clone()));
        }
        created.push(entity);
    }

    // Apply parent relationships. A `parent N` line whose N is outside
    // the created entity set is silently ignored — the file referenced
    // an entity that does not exist (corrupted or out-of-order).
    for (child_local_idx, parent_local_opt) in local_parents.iter().enumerate() {
        if let Some(parent_local_idx) = parent_local_opt {
            if *parent_local_idx >= created.len() {
                continue;
            }
            let child_entity = created[child_local_idx];
            let parent_entity = created[*parent_local_idx];
            // add_child silently refuses self-parent and cycles (v1.0.42).
            add_child(world, parent_entity, child_entity);
        }
    }

    created
}

// ── Hex encoding helpers ────────────────────────────────────────────

fn bytes_to_hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for &b in data {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn hex_to_bytes(hex: &str) -> crate::Result<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return Err("Hex string has odd length".into());
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let mut i = 0;
    let chars: Vec<u8> = hex.bytes().collect();
    while i < chars.len() {
        let hi = hex_digit(chars[i])?;
        let lo = hex_digit(chars[i + 1])?;
        bytes.push((hi << 4) | lo);
        i += 2;
    }
    Ok(bytes)
}

fn hex_digit(c: u8) -> crate::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("Invalid hex digit: {}", c as char).into()),
    }
}

// ── JSON scene serialization ────────────────────────────────────────

/// Serialize all entities with a Transform to JSON format using the component registry.
/// Format: {"version":1,"entities":[{"parent":null_or_idx,"components":{"name":"hex",...}}]}
///
/// **Transform anchor**: only entities that have a [`Transform`] are emitted —
/// the Transform is the scene's entity anchor (parent indices reference this
/// list). An entity carrying other components (e.g. `PrimitiveMesh`, `Sprite`,
/// `AudioSource`) but *no* Transform is dropped entirely. In practice every
/// renderable/spatial entity has a Transform, so this is rarely a surprise, but
/// it means a Transform-less entity will not round-trip. The binary
/// ([`serialize_scene_binary`](crate::serialize_scene_binary)) path shares this
/// anchor rule.
pub fn serialize_scene_json(world: &World, registry: &ComponentRegistry) -> String {
    let entities_with_transform: Vec<(Entity, Transform)> =
        world.query::<Transform>().map(|(e, t)| (e, *t)).collect();

    let entity_to_index: HashMap<Entity, usize> = entities_with_transform
        .iter()
        .enumerate()
        .map(|(i, (e, _))| (*e, i))
        .collect();

    // Build entity JSON values
    let mut entity_vals = Vec::new();
    for (entity, _) in &entities_with_transform {
        // Parent index
        let parent_val = if let Some(parent_comp) = world.get::<Parent>(*entity) {
            if let Some(&idx) = entity_to_index.get(&parent_comp.0) {
                JsonVal::Number(idx as f64)
            } else {
                JsonVal::Null
            }
        } else {
            JsonVal::Null
        };

        // Components
        let mut comp_pairs = Vec::new();
        for entry in registry.entries() {
            if let Some(data) = (entry.serialize)(world, *entity) {
                comp_pairs.push((entry.name.clone(), JsonVal::Str(bytes_to_hex(&data))));
            }
        }

        entity_vals.push(JsonVal::Object(vec![
            ("parent".into(), parent_val),
            ("components".into(), JsonVal::Object(comp_pairs)),
        ]));
    }

    let root = JsonVal::Object(vec![
        ("version".into(), JsonVal::Number(1.0)),
        ("entities".into(), JsonVal::Array(entity_vals)),
    ]);

    root.to_json_string()
}

/// Deserialize entities from a JSON scene string.
///
/// Caps entity count at [`MAX_SCENE_ENTITIES`] (mirrors the binary/text
/// formats) so a malicious JSON cannot drive a multi-GB
/// `Vec::with_capacity` allocation post-parse. Note: a separate parser-
/// level array-length cap in `json::json_parse` would block the bomb
/// earlier; this is defense-in-depth at the deserializer boundary.
pub fn deserialize_scene_json(
    world: &mut World,
    json_str: &str,
    registry: &ComponentRegistry,
) -> crate::Result<Vec<Entity>> {
    deserialize_scene_json_with_cap(world, json_str, registry, MAX_SCENE_ENTITIES)
}

/// Test-friendly variant exposing the entity cap. Keeps the production
/// path on `MAX_SCENE_ENTITIES` while letting tests exercise the
/// rejection branch with a small fixture.
pub(crate) fn deserialize_scene_json_with_cap(
    world: &mut World,
    json_str: &str,
    registry: &ComponentRegistry,
    cap: usize,
) -> crate::Result<Vec<Entity>> {
    // A UTF-8 BOM (Notepad / "UTF-8 with signature") survives from_utf8 as
    // U+FEFF and is not JSON whitespace.
    let json_str = json_str.strip_prefix('\u{FEFF}').unwrap_or(json_str);
    let root = json::json_parse(json_str)?;

    let version = root
        .get("version")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| crate::EcsError::from("Missing or invalid 'version'"))?;
    if version as u32 != 1 {
        return Err(format!("Unsupported version: {}", version).into());
    }

    let entities_arr = root
        .get("entities")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::EcsError::from("Missing or invalid 'entities'"))?;

    if entities_arr.len() > cap {
        return Err(format!(
            "JSON scene entity count {} exceeds limit {}",
            entities_arr.len(),
            cap
        )
        .into());
    }

    // First pass: create entities and deserialize components
    let mut created: Vec<Entity> = Vec::with_capacity(entities_arr.len());
    let mut parent_indices: Vec<Option<usize>> = Vec::with_capacity(entities_arr.len());

    for entity_val in entities_arr {
        let entity = world.spawn();

        // Parse parent index
        let parent_idx = match entity_val.get("parent") {
            Some(JsonVal::Number(n)) => Some(*n as usize),
            _ => None,
        };
        parent_indices.push(parent_idx);

        // Deserialize components
        if let Some(comps) = entity_val.get("components").and_then(|v| v.as_object()) {
            for (name, hex_val) in comps {
                if let Some(hex_str) = hex_val.as_str() {
                    let data = hex_to_bytes(hex_str)?;
                    if let Some(entry) = registry.find(name) {
                        (entry.deserialize)(world, entity, &data)?;
                    }
                }
            }
        }

        created.push(entity);
    }

    // Second pass: apply parent relationships
    for (child_idx, parent_idx_opt) in parent_indices.iter().enumerate() {
        if let Some(parent_idx) = parent_idx_opt {
            if *parent_idx < created.len() {
                let child_entity = created[child_idx];
                let parent_entity = created[*parent_idx];
                add_child(world, parent_entity, child_entity);
            }
        }
    }

    // Defense-in-depth: a user-registered Parent component would have been
    // written directly via the ComponentRegistry path above, bypassing
    // `add_child`'s cycle guard. Strip any cycles before returning.
    let _ = validate_hierarchy(world);

    Ok(created)
}

#[cfg(test)]
mod tests;
