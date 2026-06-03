//! End-to-end composition test: drive a small headless gameplay slice using the
//! v1.0.62–74 authoring/runtime primitives together — `Prefabs`, `World`
//! resources, `Events`, `spawn_child_bundle`, `find_by_tag`/`iter_by_tag`, and
//! `despawn_recursive`. Unit tests cover each primitive in isolation; this
//! proves they actually *compose* into a frame loop the way a game wires them.

use voltex_ecs::{Prefabs, Tag, World};

#[derive(Debug, PartialEq)]
struct Health(i32);

#[derive(Debug, Default, PartialEq)]
struct Score(u32);

/// Frame budget for spawning new enemies.
struct SpawnBudget {
    remaining: u32,
}

/// Event: an enemy died this frame and is worth `points`.
#[derive(Debug, PartialEq)]
struct EnemyKilled {
    points: u32,
}

/// System: spend one unit of the spawn budget to instantiate an "enemy" prefab,
/// then attach a child "shield" so we can prove hierarchy teardown later.
fn spawn_system(world: &mut World, prefabs: &Prefabs) {
    let budget_left = world.resource::<SpawnBudget>().map(|b| b.remaining).unwrap_or(0);
    if budget_left == 0 {
        return;
    }
    let enemy = prefabs.spawn(world, "enemy").expect("enemy prefab registered");
    world.spawn_child_bundle(enemy, (Tag("shield".to_string()),));
    world.resource_mut::<SpawnBudget>().unwrap().remaining -= 1;
}

/// System: every enemy takes 1 damage; any that reach 0 HP are despawned
/// (recursively, taking their shield child) and emit an `EnemyKilled` event.
fn damage_system(world: &mut World) {
    let enemies: Vec<_> = world.iter_by_tag("enemy").collect();
    let mut dead = Vec::new();
    for e in enemies {
        if let Some(h) = world.get_mut::<Health>(e) {
            h.0 -= 1;
            if h.0 <= 0 {
                dead.push(e);
            }
        }
    }
    for e in dead {
        voltex_ecs::despawn_recursive(world, e);
        world.send_event(EnemyKilled { points: 10 });
    }
}

/// System: bank the points from every `EnemyKilled` event this frame.
fn score_system(world: &mut World) {
    let kills = world.drain_events::<EnemyKilled>();
    if kills.is_empty() {
        return;
    }
    let gained: u32 = kills.iter().map(|k| k.points).sum();
    world.resource_mut::<Score>().expect("Score resource exists").0 += gained;
}

fn enemy_prefabs() -> Prefabs {
    let mut prefabs = Prefabs::new();
    // Enemy: 2 HP, tagged. Two frames of damage_system kill it.
    prefabs.register("enemy", |world| {
        world.spawn_bundle((Health(2), Tag("enemy".to_string())))
    });
    prefabs
}

#[test]
fn full_stack_gameplay_loop_spawns_kills_and_scores() {
    let prefabs = enemy_prefabs();
    let mut world = World::new();
    world.insert_resource(Score::default());
    world.insert_resource(SpawnBudget { remaining: 3 });

    // Run frames until the spawn budget is spent AND every enemy is dead.
    // Producer (damage) runs before consumer (score) each frame — the documented
    // single-buffer event ordering.
    let mut frames = 0;
    loop {
        spawn_system(&mut world, &prefabs);
        damage_system(&mut world);
        score_system(&mut world);
        frames += 1;

        let budget = world.resource::<SpawnBudget>().unwrap().remaining;
        let alive = world.iter_by_tag("enemy").count();
        if budget == 0 && alive == 0 {
            break;
        }
        assert!(frames < 50, "gameplay loop failed to converge");
    }

    // 3 enemies, each worth 10, all eventually killed.
    assert_eq!(world.resource::<Score>(), Some(&Score(30)));
    // No enemies and no shields remain — despawn_recursive cleaned the children.
    assert_eq!(world.iter_by_tag("enemy").count(), 0);
    assert_eq!(world.iter_by_tag("shield").count(), 0);
    // The spawn budget was fully spent.
    assert_eq!(world.resource::<SpawnBudget>().unwrap().remaining, 0);
}

#[test]
fn events_left_undrained_persist_until_a_consumer_runs() {
    // If the score system doesn't run on a frame, the kill events are not lost —
    // they sit in the Events<T> resource until something drains them.
    let prefabs = enemy_prefabs();
    let mut world = World::new();
    world.insert_resource(Score::default());
    world.insert_resource(SpawnBudget { remaining: 1 });

    spawn_system(&mut world, &prefabs); // 1 enemy (2 HP) + shield
    damage_system(&mut world); // 2 -> 1, no kill, no event
    damage_system(&mut world); // 1 -> 0, kill -> 1 event queued, NOT consumed yet

    // The event is still queued (no score_system ran).
    assert_eq!(world.resource::<Score>(), Some(&Score(0)));
    // A consumer finally runs and banks it.
    score_system(&mut world);
    assert_eq!(world.resource::<Score>(), Some(&Score(10)));
}

#[test]
fn child_entities_are_torn_down_with_their_parent_mid_loop() {
    // Focused check that spawn_child_bundle + despawn_recursive interplay holds
    // inside the gameplay flow: spawning an enemy creates exactly one shield,
    // and killing the enemy removes it.
    let prefabs = enemy_prefabs();
    let mut world = World::new();
    world.insert_resource(SpawnBudget { remaining: 1 });

    spawn_system(&mut world, &prefabs);
    assert_eq!(world.iter_by_tag("enemy").count(), 1);
    assert_eq!(world.iter_by_tag("shield").count(), 1);

    let enemy = world.find_by_tag("enemy").unwrap();
    voltex_ecs::despawn_recursive(&mut world, enemy);
    assert_eq!(world.iter_by_tag("enemy").count(), 0);
    assert_eq!(world.iter_by_tag("shield").count(), 0, "shield child must die with the enemy");
}
