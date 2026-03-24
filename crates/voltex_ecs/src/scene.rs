use std::collections::HashMap;
use voltex_math::Vec3;
use crate::entity::Entity;
use crate::world::World;
use crate::transform::Transform;
use crate::hierarchy::{add_child, Parent};

/// String tag for entity identification.
#[derive(Debug, Clone)]
pub struct Tag(pub String);

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

/// Parse a transform line of the form "px py pz | rx ry rz | sx sy sz".
fn parse_transform(s: &str) -> Option<Transform> {
    let parts: Vec<&str> = s.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let position = parse_vec3(parts[0].trim())?;
    let rotation = parse_vec3(parts[1].trim())?;
    let scale = parse_vec3(parts[2].trim())?;
    Some(Transform { position, rotation, scale })
}

/// Serialize all entities with a Transform component to the .vscn text format.
pub fn serialize_scene(world: &World) -> String {
    // Collect all entities with Transform
    let entities_with_transform: Vec<(Entity, Transform)> = world
        .query::<Transform>()
        .map(|(e, t)| (e, *t))
        .collect();

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

        // Transform line
        let p = transform.position;
        let r = transform.rotation;
        let s = transform.scale;
        output.push_str(&format!(
            "  transform {} {} {} | {} {} {} | {} {} {}\n",
            p.x, p.y, p.z,
            r.x, r.y, r.z,
            s.x, s.y, s.z
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

/// Parse a .vscn string, create entities in the world, and return the created entities.
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
        let transform = local_transforms[i].unwrap_or_else(Transform::new);
        world.add(entity, transform);
        // Add tag if present
        if let Some(ref tag_str) = local_tags[i] {
            world.add(entity, Tag(tag_str.clone()));
        }
        created.push(entity);
    }

    // Apply parent relationships
    for (child_local_idx, parent_local_opt) in local_parents.iter().enumerate() {
        if let Some(parent_local_idx) = parent_local_opt {
            let child_entity = created[child_local_idx];
            let parent_entity = created[*parent_local_idx];
            add_child(world, parent_entity, child_entity);
        }
    }

    created
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::{add_child, roots, Parent};
    use voltex_math::Vec3;

    #[test]
    fn test_serialize_single_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        });
        world.add(e, Tag("sun".to_string()));

        let output = serialize_scene(&world);
        assert!(output.contains("entity 0"), "should contain 'entity 0'");
        assert!(output.contains("transform"), "should contain 'transform'");
        assert!(output.contains("tag"), "should contain 'tag'");
        assert!(output.contains("sun"), "should contain tag value 'sun'");
    }

    #[test]
    fn test_serialize_with_parent() {
        let mut world = World::new();
        let parent = world.spawn();
        let child = world.spawn();

        world.add(parent, Transform::new());
        world.add(child, Transform::new());
        add_child(&mut world, parent, child);

        let output = serialize_scene(&world);
        assert!(output.contains("parent"), "should contain 'parent' for child entity");
    }

    #[test]
    fn test_roundtrip() {
        let mut world1 = World::new();

        // Entity 0: sun (root)
        let sun = world1.spawn();
        world1.add(sun, Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Vec3::new(0.0, 0.5, 0.0),
            scale: Vec3::ONE,
        });
        world1.add(sun, Tag("sun".to_string()));

        // Entity 1: planet (child of sun)
        let planet = world1.spawn();
        world1.add(planet, Transform {
            position: Vec3::new(5.0, 0.0, 0.0),
            rotation: Vec3::ZERO,
            scale: Vec3::new(0.5, 0.5, 0.5),
        });
        world1.add(planet, Tag("planet".to_string()));
        add_child(&mut world1, sun, planet);

        let serialized = serialize_scene(&world1);

        let mut world2 = World::new();
        let entities = deserialize_scene(&mut world2, &serialized);

        assert_eq!(entities.len(), 2, "should have 2 entities");

        // Verify Transform values
        let sun2 = entities[0];
        let planet2 = entities[1];

        let sun_transform = world2.get::<Transform>(sun2).expect("sun should have Transform");
        assert!((sun_transform.position.x - 1.0).abs() < 1e-4, "sun position.x");
        assert!((sun_transform.position.y - 2.0).abs() < 1e-4, "sun position.y");
        assert!((sun_transform.position.z - 3.0).abs() < 1e-4, "sun position.z");
        assert!((sun_transform.rotation.y - 0.5).abs() < 1e-4, "sun rotation.y");

        let planet_transform = world2.get::<Transform>(planet2).expect("planet should have Transform");
        assert!((planet_transform.position.x - 5.0).abs() < 1e-4, "planet position.x");
        assert!((planet_transform.scale.x - 0.5).abs() < 1e-4, "planet scale.x");

        // Verify Parent relationship
        let parent_comp = world2.get::<Parent>(planet2).expect("planet should have Parent");
        assert_eq!(parent_comp.0, sun2, "planet's parent should be sun");

        // Verify Tag values
        let sun_tag = world2.get::<Tag>(sun2).expect("sun should have Tag");
        assert_eq!(sun_tag.0, "sun");

        let planet_tag = world2.get::<Tag>(planet2).expect("planet should have Tag");
        assert_eq!(planet_tag.0, "planet");
    }

    #[test]
    fn test_deserialize_roots() {
        let source = r#"# Voltex Scene v1

entity 0
  transform 0 0 0 | 0 0 0 | 1 1 1
  tag root_a

entity 1
  transform 10 0 0 | 0 0 0 | 1 1 1
  tag root_b

entity 2
  parent 0
  transform 1 0 0 | 0 0 0 | 1 1 1
  tag child_of_a
"#;

        let mut world = World::new();
        deserialize_scene(&mut world, source);

        let scene_roots = roots(&world);
        assert_eq!(scene_roots.len(), 2, "should have exactly 2 root entities");
    }
}
