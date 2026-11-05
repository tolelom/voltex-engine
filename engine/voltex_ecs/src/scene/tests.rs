use super::*;
use crate::component_registry::ComponentRegistry;
use crate::hierarchy::{add_child, roots, Parent};
use voltex_math::{Quat, Vec3};

#[test]
fn find_by_tag_returns_the_tagged_entity() {
    let mut world = World::new();
    let player = world.spawn();
    world.add(player, Tag("player".to_string()));
    let other = world.spawn();
    world.add(other, Tag("enemy".to_string()));

    assert_eq!(world.find_by_tag("player"), Some(player));
    assert_eq!(world.find_by_tag("enemy"), Some(other));
}

#[test]
fn find_by_tag_returns_none_when_absent() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Tag("player".to_string()));
    assert_eq!(world.find_by_tag("missing"), None);
}

#[test]
fn iter_by_tag_yields_all_matching_entities() {
    let mut world = World::new();
    let a = world.spawn();
    world.add(a, Tag("enemy".to_string()));
    let b = world.spawn();
    world.add(b, Tag("enemy".to_string()));
    let c = world.spawn();
    world.add(c, Tag("player".to_string()));

    let enemies: std::collections::HashSet<_> = world.iter_by_tag("enemy").collect();
    assert_eq!(enemies, std::collections::HashSet::from([a, b]));
    assert_eq!(world.iter_by_tag("player").count(), 1);
    assert_eq!(world.iter_by_tag("none").count(), 0);
}

#[test]
fn test_serialize_single_entity() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(
        e,
        Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    world.add(e, Tag("sun".to_string()));

    let output = serialize_scene_text(&world);
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

    let output = serialize_scene_text(&world);
    assert!(
        output.contains("parent"),
        "should contain 'parent' for child entity"
    );
}

#[test]
fn test_roundtrip() {
    let mut world1 = World::new();

    // Entity 0: sun (root)
    let sun = world1.spawn();
    let sun_rot = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.5);
    world1.add(
        sun,
        Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: sun_rot,
            scale: Vec3::ONE,
        },
    );
    world1.add(sun, Tag("sun".to_string()));

    // Entity 1: planet (child of sun)
    let planet = world1.spawn();
    world1.add(
        planet,
        Transform {
            position: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(0.5, 0.5, 0.5),
        },
    );
    world1.add(planet, Tag("planet".to_string()));
    add_child(&mut world1, sun, planet);

    let serialized = serialize_scene_text(&world1);

    let mut world2 = World::new();
    let entities = deserialize_scene_text(&mut world2, &serialized);

    assert_eq!(entities.len(), 2, "should have 2 entities");

    // Verify Transform values
    let sun2 = entities[0];
    let planet2 = entities[1];

    let sun_transform = world2
        .get::<Transform>(sun2)
        .expect("sun should have Transform");
    assert!(
        (sun_transform.position.x - 1.0).abs() < 1e-4,
        "sun position.x"
    );
    assert!(
        (sun_transform.position.y - 2.0).abs() < 1e-4,
        "sun position.y"
    );
    assert!(
        (sun_transform.position.z - 3.0).abs() < 1e-4,
        "sun position.z"
    );
    // Quaternion components round-trip exactly through the text format.
    assert!(
        (sun_transform.rotation.x - sun_rot.x).abs() < 1e-4
            && (sun_transform.rotation.y - sun_rot.y).abs() < 1e-4
            && (sun_transform.rotation.z - sun_rot.z).abs() < 1e-4
            && (sun_transform.rotation.w - sun_rot.w).abs() < 1e-4,
        "sun rotation round-trip"
    );

    let planet_transform = world2
        .get::<Transform>(planet2)
        .expect("planet should have Transform");
    assert!(
        (planet_transform.position.x - 5.0).abs() < 1e-4,
        "planet position.x"
    );
    assert!(
        (planet_transform.scale.x - 0.5).abs() < 1e-4,
        "planet scale.x"
    );

    // Verify Parent relationship
    let parent_comp = world2
        .get::<Parent>(planet2)
        .expect("planet should have Parent");
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
    deserialize_scene_text(&mut world, source);

    let scene_roots = roots(&world);
    assert_eq!(scene_roots.len(), 2, "should have exactly 2 root entities");
}

// ── JSON scene tests ────────────────────────────────────────────

#[test]
fn test_hex_roundtrip() {
    let data = vec![0u8, 1, 15, 16, 255];
    let hex = bytes_to_hex(&data);
    assert_eq!(hex, "00010f10ff");
    let back = hex_to_bytes(&hex).unwrap();
    assert_eq!(back, data);
}

#[test]
fn test_json_roundtrip() {
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(1.0, 2.0, 3.0)));
    world.add(e, Tag("player".into()));

    let json = serialize_scene_json(&world, &registry);
    assert!(json.contains("\"version\":1"));

    let mut world2 = World::new();
    let entities = deserialize_scene_json(&mut world2, &json, &registry).unwrap();
    assert_eq!(entities.len(), 1);
    let t = world2.get::<Transform>(entities[0]).unwrap();
    assert!((t.position.x - 1.0).abs() < 1e-4);
    assert!((t.position.y - 2.0).abs() < 1e-4);
    assert!((t.position.z - 3.0).abs() < 1e-4);
    let tag = world2.get::<Tag>(entities[0]).unwrap();
    assert_eq!(tag.0, "player");
}

#[test]
fn json_roundtrip_keeps_components_with_non_ascii_names() {
    // Component NAMEs are JSON keys; the reader used to push each UTF-8
    // byte as a char, so a non-ASCII name never matched on load and the
    // component was silently dropped.
    #[derive(Debug, PartialEq)]
    struct Hp(u8);
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    registry.register_raw(
        "체력",
        |w, e| w.get::<Hp>(e).map(|h| vec![h.0]),
        |w, e, d| {
            w.add(e, Hp(d[0]));
            Ok(())
        },
    );
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(0.0, 0.0, 0.0)));
    world.add(e, Hp(42));
    let json = serialize_scene_json(&world, &registry);

    let mut loaded = World::new();
    let entities = deserialize_scene_json(&mut loaded, &json, &registry).unwrap();
    assert_eq!(loaded.get::<Hp>(entities[0]), Some(&Hp(42)));
}

#[test]
fn deserialize_scene_json_accepts_leading_utf8_bom() {
    // Windows editors (Notepad, VS "UTF-8 with signature") save a BOM;
    // `read_to_string`/`from_utf8` keep it as U+FEFF.
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Transform::from_position(Vec3::new(1.0, 0.0, 0.0)));
    world.add(e, Tag("bom".into()));
    let json = format!("\u{FEFF}{}", serialize_scene_json(&world, &registry));

    let mut loaded = World::new();
    let entities = deserialize_scene_json(&mut loaded, &json, &registry)
        .expect("BOM-prefixed scene loads");
    assert_eq!(loaded.get::<Tag>(entities[0]).unwrap().0, "bom");
}

#[test]
fn test_json_with_parent() {
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut world = World::new();
    let parent = world.spawn();
    let child = world.spawn();
    world.add(parent, Transform::new());
    world.add(child, Transform::new());
    add_child(&mut world, parent, child);

    let json = serialize_scene_json(&world, &registry);
    let mut world2 = World::new();
    let entities = deserialize_scene_json(&mut world2, &json, &registry).unwrap();
    assert_eq!(entities.len(), 2);
    assert!(world2.get::<Parent>(entities[1]).is_some());
    let parent_comp = world2.get::<Parent>(entities[1]).unwrap();
    assert_eq!(parent_comp.0, entities[0]);
}

#[test]
fn test_json_multiple_entities() {
    let mut registry = ComponentRegistry::new();
    registry.register_defaults();
    let mut world = World::new();
    for i in 0..5 {
        let e = world.spawn();
        world.add(e, Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)));
        world.add(e, Tag(format!("entity_{}", i)));
    }

    let json = serialize_scene_json(&world, &registry);
    let mut world2 = World::new();
    let entities = deserialize_scene_json(&mut world2, &json, &registry).unwrap();
    assert_eq!(entities.len(), 5);
}

#[test]
fn parse_rotation_accepts_legacy_3_float_euler() {
    // Files written before the E3b migration had a 3-float Euler payload.
    // The parser must still consume them and produce the equivalent quat.
    let q = super::parse_rotation("0.3 0.7 -0.4").expect("parsed");
    let expected = Quat::from_euler_yxz(Vec3::new(0.3, 0.7, -0.4));
    assert!((q.x - expected.x).abs() < 1e-6);
    assert!((q.y - expected.y).abs() < 1e-6);
    assert!((q.z - expected.z).abs() < 1e-6);
    assert!((q.w - expected.w).abs() < 1e-6);
}

#[test]
fn parse_rotation_accepts_4_float_quaternion() {
    let q = super::parse_rotation("0.1 0.2 0.3 0.9").expect("parsed");
    assert_eq!((q.x, q.y, q.z, q.w), (0.1, 0.2, 0.3, 0.9));
}

#[test]
fn parse_rotation_rejects_other_arities() {
    assert!(super::parse_rotation("1 2").is_none());
    assert!(super::parse_rotation("1 2 3 4 5").is_none());
}

// ── adversarial / robustness tests ───────────────────────────────

#[test]
fn deserialize_scene_caps_huge_entity_index() {
    // `entity 4294967295` used to push billions of placeholder slots
    // into the intermediate Vec. Now silently ignored above the cap.
    let source = "entity 4294967295\n  transform 0 0 0 | 0 0 0 | 1 1 1\n";
    let mut world = World::new();
    let created = deserialize_scene_text(&mut world, source);
    // Nothing got created — the entity declaration was above the cap.
    assert!(created.is_empty());
}

#[test]
fn deserialize_scene_ignores_out_of_range_parent_index() {
    // `parent 99` references an entity that does not exist. Before the
    // bounds check, this panicked with index-out-of-bounds inside the
    // parent-application loop.
    let source = r#"
entity 0
  transform 0 0 0 | 0 0 0 | 1 1 1
  parent 99
"#;
    let mut world = World::new();
    let created = deserialize_scene_text(&mut world, source);
    assert_eq!(created.len(), 1);
    // No Parent component was attached because the index was out of range.
    assert!(world.get::<Parent>(created[0]).is_none());
}

#[test]
fn deserialize_scene_self_parent_is_silently_refused() {
    // Hierarchy guard from v1.0.42: even if the scene file declares
    // a self-parent edge, `add_child` refuses it. The entity is
    // created without a Parent component.
    let source = r#"
entity 0
  transform 0 0 0 | 0 0 0 | 1 1 1
  parent 0
"#;
    let mut world = World::new();
    let created = deserialize_scene_text(&mut world, source);
    assert_eq!(created.len(), 1);
    assert!(world.get::<Parent>(created[0]).is_none());
}

#[test]
fn deserialize_scene_at_cap_boundary_works() {
    // Index exactly equal to the cap is rejected; one below succeeds
    // but would allocate ~1M Vec entries, which is acceptable.
    let source = format!(
        "entity {}\n  transform 0 0 0 | 0 0 0 | 1 1 1\n",
        MAX_SCENE_ENTITIES
    );
    let mut world = World::new();
    let created = deserialize_scene_text(&mut world, &source);
    assert!(
        created.is_empty(),
        "entity at MAX_SCENE_ENTITIES must be rejected"
    );
}

#[test]
fn deserialize_scene_json_rejects_count_above_cap() {
    // Defense-in-depth: an oversized `entities` array must be rejected
    // before allocating downstream `Vec<Entity>` / parent-index buffers.
    // Uses the `_with_cap` helper to exercise the rejection branch with
    // a small fixture instead of constructing a real ~1M-entity JSON.
    let json = r#"{"version":1,"entities":[{},{},{}]}"#;
    let mut world = World::new();
    let registry = ComponentRegistry::new();
    let err = deserialize_scene_json_with_cap(&mut world, json, &registry, 2)
        .unwrap_err()
        .to_string();
    assert!(err.contains("exceeds limit"), "unexpected error: {err}");
}

#[test]
fn deserialize_scene_json_at_cap_boundary_works() {
    // Count exactly at the cap is accepted (boundary check is strictly >).
    let json = r#"{"version":1,"entities":[{},{}]}"#;
    let mut world = World::new();
    let registry = ComponentRegistry::new();
    let created = deserialize_scene_json_with_cap(&mut world, json, &registry, 2)
        .expect("count at cap must be accepted");
    assert_eq!(created.len(), 2);
}
