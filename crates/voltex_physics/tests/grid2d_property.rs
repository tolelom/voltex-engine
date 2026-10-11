//! Randomized property for `move_and_collide_2d`: on random tile grids, with
//! random actor sizes (smaller and larger than a tile) and speeds (including
//! many tiles per frame), the resolved box must never end inside a solid
//! tile, never move further or backwards on an axis, report hits consistently,
//! and never tunnel through a wall on purely horizontal / vertical moves.

use voltex_math::Vec3;
use voltex_physics::move_and_collide_2d;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() % 100_000) as f32 / 100_000.0 * (hi - lo)
    }
}

const W: i32 = 24;
const H: i32 = 16;

struct Grid(Vec<bool>);
impl Grid {
    fn random(rng: &mut XorShift) -> Self {
        let density = rng.range(0.05, 0.35);
        Grid((0..W * H).map(|_| rng.range(0.0, 1.0) < density).collect())
    }
    /// Outside the map counts as solid (the usual caller policy).
    fn solid(&self, gx: i32, gy: i32) -> bool {
        !(0..W).contains(&gx) || !(0..H).contains(&gy) || self.0[(gy * W + gx) as usize]
    }
    /// Does the box overlap any solid cell by more than `slack`?
    fn overlaps(&self, tile: f32, p: Vec3, half: [f32; 2], slack: f32) -> bool {
        let x0 = ((p.x - half[0] + slack) / tile).floor() as i32;
        let x1 = ((p.x + half[0] - slack) / tile).floor() as i32;
        let y0 = ((p.y - half[1] + slack) / tile).floor() as i32;
        let y1 = ((p.y + half[1] - slack) / tile).floor() as i32;
        (y0..=y1).any(|gy| (x0..=x1).any(|gx| self.solid(gx, gy)))
    }
}

#[test]
fn grid_moves_never_end_in_walls_or_overshoot() {
    let mut checked = (0, 0); // (blocked moves, free moves)
    for seed in [11u32, 0xD00D, 0x2468_ACE0, 777] {
        let mut rng = XorShift(seed);
        for case in 0..600 {
            let grid = Grid::random(&mut rng);
            let tile = rng.range(0.5, 2.0);
            let half = [rng.range(0.1, 1.5) * tile, rng.range(0.1, 1.5) * tile];
            let Some(start) = (0..50).find_map(|_| {
                let p = Vec3::new(rng.range(0.0, W as f32) * tile, rng.range(0.0, H as f32) * tile, 0.0);
                (!grid.overlaps(tile, p, half, 0.0)).then_some(p)
            }) else {
                continue;
            };
            let speed = if rng.next().is_multiple_of(4) { 60.0 } else { 6.0 } * tile;
            let mut vel = Vec3::new(rng.range(-speed, speed), rng.range(-speed, speed), 0.0);
            match rng.next() % 3 {
                0 => vel.y = 0.0,
                1 => vel.x = 0.0,
                _ => {}
            }
            let dt = rng.range(0.005, 0.05);
            let ctx = format!("seed {seed:x} case {case}: tile {tile} half {half:?} start {start:?} vel {vel:?} dt {dt}");

            let r = move_and_collide_2d(|gx, gy| grid.solid(gx, gy), tile, start, vel, half, dt);

            let slack = tile * 1e-3;
            assert!(!grid.overlaps(tile, r.pos, half, slack), "{ctx}: ended inside a solid at {:?}", r.pos);
            for (axis, (moved, want, v_out, v_in)) in [
                (r.pos.x - start.x, vel.x * dt, r.vel.x, vel.x),
                (r.pos.y - start.y, vel.y * dt, r.vel.y, vel.y),
            ]
            .into_iter()
            .enumerate()
            {
                assert!(moved * want >= -slack * slack, "{ctx}: axis {axis} moved backwards ({moved} vs {want})");
                assert!(moved.abs() <= want.abs() + slack, "{ctx}: axis {axis} overshot ({moved} vs {want})");
                assert!(v_out == v_in || v_out == 0.0, "{ctx}: axis {axis} velocity changed to {v_out}");
                if v_out == v_in {
                    assert!((moved - want).abs() <= slack, "{ctx}: axis {axis} stopped short ({moved} vs {want}) without a hit");
                }
            }
            assert_eq!(r.hit_wall, r.vel.x == 0.0 && vel.x != 0.0, "{ctx}: hit_wall flag");
            assert_eq!(r.pos.z, start.z, "{ctx}: z changed");
            let below = Vec3::new(r.pos.x, r.pos.y - tile * 0.03, r.pos.z);
            if r.grounded {
                assert!(grid.overlaps(tile, below, half, slack), "{ctx}: grounded with nothing underfoot");
            }
            if vel.y < 0.0 && r.vel.y == 0.0 && !r.grounded {
                // Landing mid-frame and then sliding off the ledge is legitimate.
                let probe = Vec3::new(r.pos.x, r.pos.y - tile * 0.01, r.pos.z);
                assert!(!grid.overlaps(tile, probe, half, slack), "{ctx}: standing on ground but not grounded");
            }

            // Straight moves: every intermediate box along the path must be clear.
            if vel.x == 0.0 || vel.y == 0.0 {
                for i in 0..=200 {
                    let p = start + (r.pos - start) * (i as f32 / 200.0);
                    assert!(!grid.overlaps(tile, p, half, slack), "{ctx}: tunnelled through a wall at {p:?} (end {:?})", r.pos);
                }
            }
            if r.vel == vel {
                checked.1 += 1;
            } else {
                checked.0 += 1;
            }
        }
    }
    assert!(checked.0 > 300 && checked.1 > 300, "generator too one-sided: {checked:?}");
}
