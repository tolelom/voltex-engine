use super::*;

#[derive(Debug, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, PartialEq)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[test]
fn clear_drops_all_entities_and_components() {
    let mut world = World::new();
    let e1 = world.spawn();
    world.add(e1, Position { x: 1.0, y: 2.0 });
    let e2 = world.spawn();
    world.add(e2, Position { x: 3.0, y: 4.0 });
    assert_eq!(world.entity_count(), 2);

    world.clear();

    assert_eq!(world.entity_count(), 0);
    // Old handles must read as not alive.
    assert!(!world.is_alive(e1));
    assert!(!world.is_alive(e2));
    // Component lookups return None.
    assert!(world.get::<Position>(e1).is_none());
}

#[test]
fn clear_then_spawn_reuses_id_zero_with_fresh_generation() {
    let mut world = World::new();
    let old = world.spawn();
    world.clear();
    let e_new = world.spawn();
    // Slots are freed, not reset: id 0 comes back with a bumped generation,
    // so the pre-clear handle can never alias the new entity.
    assert_eq!(e_new.id, 0);
    assert_eq!(e_new.generation, 1);
    assert!(!world.is_alive(old));
}

#[test]
fn clear_keeps_pre_clear_handles_stale_after_respawn() {
    let mut world = World::new();
    let a = world.spawn();
    world.add(a, Position { x: 1.0, y: 1.0 });
    let b = world.spawn();
    world.clear();

    let a2 = world.spawn();
    world.add(a2, Position { x: 9.0, y: 9.0 });
    let _b2 = world.spawn();

    assert!(!world.is_alive(a));
    assert!(!world.is_alive(b));
    assert!(world.get::<Position>(a).is_none(), "old handle must not read the new entity");
}

#[test]
fn clear_with_mixed_freed_and_live_slots_stales_every_old_handle() {
    let mut world = World::new();
    let a = world.spawn();
    let b = world.spawn();
    let c = world.spawn();
    world.despawn(b); // slot 1 already freed (generation bumped once)
    world.clear();

    let respawned: Vec<_> = (0..3).map(|_| world.spawn()).collect();
    for old in [a, b, c] {
        assert!(!world.is_alive(old), "{old:?} aliased after clear");
    }
    assert_eq!(world.entity_count(), 3);
    assert!(respawned.iter().all(|e| world.is_alive(*e)));
}

#[test]
fn clear_blocks_restore_of_a_handle_despawned_before_clear() {
    // E3 undo revives a despawned handle at its old generation; that must not
    // reach across a clear() into the new scene.
    let mut world = World::new();
    let e = world.spawn();
    world.despawn(e);
    world.clear();
    assert!(!world.restore_at_generation(e.id, e.generation));
    assert!(!world.is_alive(e));
}

#[test]
fn clear_then_spawn_hands_out_ids_in_order() {
    let mut world = World::new();
    for _ in 0..3 {
        world.spawn();
    }
    world.clear();
    let ids: Vec<u32> = (0..4).map(|_| world.spawn().id).collect();
    assert_eq!(ids, vec![0, 1, 2, 3]);
}

#[test]
fn test_spawn_and_add() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Position { x: 1.0, y: 2.0 });
    let pos = world.get::<Position>(e).unwrap();
    assert_eq!(pos.x, 1.0);
    assert_eq!(pos.y, 2.0);
}

#[test]
fn test_get_missing() {
    let world = World::new();
    let e = Entity {
        id: 0,
        generation: 0,
    };
    assert!(world.get::<Position>(e).is_none());
}

#[test]
fn test_get_mut() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Position { x: 0.0, y: 0.0 });
    {
        let pos = world.get_mut::<Position>(e).unwrap();
        pos.x = 42.0;
    }
    assert_eq!(world.get::<Position>(e).unwrap().x, 42.0);
}

#[test]
fn test_remove_component() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Position { x: 5.0, y: 6.0 });
    let removed = world.remove::<Position>(e);
    assert_eq!(removed, Some(Position { x: 5.0, y: 6.0 }));
    assert!(world.get::<Position>(e).is_none());
}

#[test]
fn test_despawn() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Position { x: 1.0, y: 2.0 });
    world.add(e, Velocity { dx: 3.0, dy: 4.0 });
    assert!(world.despawn(e));
    assert!(!world.is_alive(e));
    assert!(world.get::<Position>(e).is_none());
    assert!(world.get::<Velocity>(e).is_none());
}

#[test]
fn test_query_single() {
    let mut world = World::new();
    let e0 = world.spawn();
    let e1 = world.spawn();
    let _e2 = world.spawn(); // no Position
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e1, Position { x: 2.0, y: 0.0 });

    let results: Vec<(Entity, &Position)> = world.query::<Position>().collect();
    assert_eq!(results.len(), 2);
    let entities: Vec<Entity> = results.iter().map(|(e, _)| *e).collect();
    assert!(entities.contains(&e0));
    assert!(entities.contains(&e1));
}

#[test]
fn test_query2() {
    let mut world = World::new();
    let e0 = world.spawn();
    let e1 = world.spawn();
    let e2 = world.spawn(); // only Position, no Velocity
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    world.add(e2, Position { x: 3.0, y: 0.0 });

    let results: Vec<_> = world.query2::<Position, Velocity>().collect();
    assert_eq!(results.len(), 2);
    let entities: Vec<Entity> = results.iter().map(|(e, _, _)| *e).collect();
    assert!(entities.contains(&e0));
    assert!(entities.contains(&e1));
    assert!(!entities.contains(&e2));
}

#[test]
fn test_query3() {
    #[derive(Debug, PartialEq)]
    struct Health(i32);

    let mut world = World::new();
    let e0 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e0, Health(100));

    let e1 = world.spawn();
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    // e1 has no Health

    let results: Vec<_> = world.query3::<Position, Velocity, Health>().collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e0);
}

#[test]
fn test_query4() {
    #[derive(Debug, PartialEq)]
    struct Health(i32);

    #[derive(Debug, PartialEq)]
    struct Tag(u8);

    let mut world = World::new();
    let e0 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e0, Health(100));
    world.add(e0, Tag(1));

    let e1 = world.spawn();
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    world.add(e1, Health(50));
    // e1 has no Tag

    let e2 = world.spawn();
    world.add(e2, Position { x: 3.0, y: 0.0 });
    world.add(e2, Velocity { dx: 3.0, dy: 0.0 });
    world.add(e2, Tag(2));
    // e2 has no Health

    let results: Vec<_> = world.query4::<Position, Velocity, Health, Tag>().collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e0);
}

#[test]
fn test_has_component() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, Position { x: 1.0, y: 2.0 });
    assert!(world.has_component::<Position>(e));
    assert!(!world.has_component::<Velocity>(e));
}

#[test]
fn test_query_with() {
    let mut world = World::new();
    let e0 = world.spawn();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e1, Position { x: 2.0, y: 0.0 });
    // e1 has Position but no Velocity
    world.add(e2, Position { x: 3.0, y: 0.0 });
    world.add(e2, Velocity { dx: 3.0, dy: 0.0 });

    let results = world.query_with::<Position, Velocity>();
    assert_eq!(results.len(), 2);
    let entities: Vec<Entity> = results.iter().map(|(e, _)| *e).collect();
    assert!(entities.contains(&e0));
    assert!(entities.contains(&e2));
    assert!(!entities.contains(&e1));
}

#[test]
fn test_query_without() {
    let mut world = World::new();
    let e0 = world.spawn();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e1, Position { x: 2.0, y: 0.0 });
    // e1 has Position but no Velocity — should be included
    world.add(e2, Position { x: 3.0, y: 0.0 });
    world.add(e2, Velocity { dx: 3.0, dy: 0.0 });

    let results = world.query_without::<Position, Velocity>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e1);
}

#[test]
fn test_query2_with() {
    #[derive(Debug, PartialEq)]
    struct Health(i32);

    let mut world = World::new();
    let e0 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e0, Health(100));

    let e1 = world.spawn();
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    // e1 has no Health

    let results = world.query2_with::<Position, Velocity, Health>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e0);
}

#[test]
fn test_query2_without() {
    #[derive(Debug, PartialEq)]
    struct Health(i32);

    let mut world = World::new();
    let e0 = world.spawn();
    world.add(e0, Position { x: 1.0, y: 0.0 });
    world.add(e0, Velocity { dx: 1.0, dy: 0.0 });
    world.add(e0, Health(100));

    let e1 = world.spawn();
    world.add(e1, Position { x: 2.0, y: 0.0 });
    world.add(e1, Velocity { dx: 2.0, dy: 0.0 });
    // e1 has no Health

    let results = world.query2_without::<Position, Velocity, Health>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e1);
}

#[test]
fn test_query_changed() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    world.add(e1, 10u32);
    world.add(e2, 20u32);
    world.advance_change_tick();
    if let Some(v) = world.get_mut::<u32>(e1) {
        *v = 100;
    }
    let changed = world.query_changed::<u32>();
    assert_eq!(changed.len(), 1);
    assert_eq!(*changed[0].1, 100);
}

#[test]
fn test_advance_change_tick_all_storages() {
    let mut world = World::new();
    let e = world.spawn();
    world.add(e, 42u32);
    world.add(e, 2.5f32);
    let changed_u32 = world.query_changed::<u32>();
    assert_eq!(changed_u32.len(), 1);
    world.advance_change_tick();
    let changed_u32 = world.query_changed::<u32>();
    assert_eq!(changed_u32.len(), 0);
    let changed_f32 = world.query_changed::<f32>();
    assert_eq!(changed_f32.len(), 0);
}

#[test]
fn test_entity_count() {
    let mut world = World::new();
    assert_eq!(world.entity_count(), 0);
    let e0 = world.spawn();
    let e1 = world.spawn();
    assert_eq!(world.entity_count(), 2);
    world.despawn(e0);
    assert_eq!(world.entity_count(), 1);
    world.despawn(e1);
    assert_eq!(world.entity_count(), 0);
}

// ── Resources (type-erased world-global singletons) ─────────────────────

#[derive(Debug, PartialEq)]
struct Score(u32);

#[derive(Debug, PartialEq)]
struct Difficulty(f32);

#[test]
fn insert_and_read_resource() {
    let mut world = World::new();
    assert!(!world.has_resource::<Score>());
    world.insert_resource(Score(10));
    assert!(world.has_resource::<Score>());
    assert_eq!(world.resource::<Score>(), Some(&Score(10)));
}

#[test]
fn absent_resource_is_none() {
    let world = World::new();
    assert_eq!(world.resource::<Score>(), None);
}

#[test]
fn resource_mut_mutates_in_place() {
    let mut world = World::new();
    world.insert_resource(Score(1));
    world.resource_mut::<Score>().unwrap().0 += 41;
    assert_eq!(world.resource::<Score>(), Some(&Score(42)));
}

#[test]
fn insert_resource_overwrites() {
    let mut world = World::new();
    world.insert_resource(Score(1));
    world.insert_resource(Score(2));
    assert_eq!(world.resource::<Score>(), Some(&Score(2)));
}

#[test]
fn remove_resource_returns_and_clears() {
    let mut world = World::new();
    world.insert_resource(Score(7));
    assert_eq!(world.remove_resource::<Score>(), Some(Score(7)));
    assert_eq!(world.resource::<Score>(), None);
    assert_eq!(world.remove_resource::<Score>(), None);
}

#[test]
fn resources_of_different_types_are_independent() {
    let mut world = World::new();
    world.insert_resource(Score(3));
    world.insert_resource(Difficulty(1.5));
    assert_eq!(world.resource::<Score>(), Some(&Score(3)));
    assert_eq!(world.resource::<Difficulty>(), Some(&Difficulty(1.5)));
}

#[test]
fn resource_or_insert_with_inserts_then_reuses() {
    let mut world = World::new();
    assert_eq!(world.resource_or_insert_with(|| Score(100)).0, 100);
    // Second call must NOT re-run the default; it returns the existing value.
    world.resource_or_insert_with(|| Score(999)).0 += 1;
    assert_eq!(world.resource::<Score>(), Some(&Score(101)));
}

#[test]
fn clear_drops_resources() {
    let mut world = World::new();
    world.insert_resource(Score(5));
    world.clear();
    assert!(!world.has_resource::<Score>());
}
