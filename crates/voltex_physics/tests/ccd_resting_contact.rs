//! CCD must only react to *new* contacts along the sweep. A fast body that is
//! already touching something (a ball skimming the floor, a bullet fired from
//! inside its muzzle collider) used to get t = 0 from that collider, be held at
//! its start position and have its velocity halved — every step until it fell
//! below the CCD speed threshold.

use voltex_ecs::{Entity, Transform, World};
use voltex_math::Vec3;
use voltex_physics::{physics_step, Collider, PhysicsConfig, RigidBody};

fn ground_and_ball(velocity: Vec3) -> (World, Entity) {
    let mut world = World::new();
    let ground = world.spawn();
    world.add(ground, Transform::from_position(Vec3::new(0.0, -0.5, 0.0)));
    world.add(ground, Collider::Box { half_extents: Vec3::new(100.0, 0.5, 100.0) });
    world.add(ground, RigidBody::statik());
    let ball = world.spawn();
    world.add(ball, Transform::from_position(Vec3::new(0.0, 0.5, 0.0)));
    world.add(ball, Collider::Sphere { radius: 0.5 });
    let mut body = RigidBody::dynamic(1.0);
    body.velocity = velocity;
    body.friction = 0.0;
    world.add(ball, body);
    (world, ball)
}

#[test]
fn fast_ball_sliding_on_the_ground_keeps_its_speed() {
    let (mut world, ball) = ground_and_ball(Vec3::new(80.0, 0.0, 0.0));
    let config = PhysicsConfig::default();
    for _ in 0..5 {
        physics_step(&mut world, &config);
    }
    let speed = world.get::<RigidBody>(ball).unwrap().velocity.x;
    let x = world.get::<Transform>(ball).unwrap().position.x;
    assert!(speed > 70.0, "CCD braked a body that was only resting on the ground: vx = {speed}");
    assert!(x > 5.0 * 70.0 * config.fixed_dt, "ball barely moved: x = {x}");
}

#[test]
fn fast_ball_still_stops_at_a_wall_ahead() {
    // The fix must not disable CCD for genuinely new contacts.
    let (mut world, ball) = ground_and_ball(Vec3::new(300.0, 0.0, 0.0));
    let wall = world.spawn();
    world.add(wall, Transform::from_position(Vec3::new(3.0, 1.0, 0.0)));
    world.add(wall, Collider::Box { half_extents: Vec3::new(0.05, 2.0, 5.0) });
    world.add(wall, RigidBody::statik());
    let config = PhysicsConfig::default();
    for step in 0..30 {
        physics_step(&mut world, &config);
        let x = world.get::<Transform>(ball).unwrap().position.x;
        assert!(x < 3.0, "step {step}: ball tunnelled through the wall: x = {x}");
    }
    // The contact was handed to the solver, which must have stopped the approach.
    let vx = world.get::<RigidBody>(ball).unwrap().velocity.x;
    assert!(vx <= 1e-3, "ball still pushing into the wall: vx = {vx}");
}
