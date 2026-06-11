//! ASCII 그리드 → 레벨 데이터. 전부 순수 (GPU/World 무관).
use crate::components::{Door, Enemy, EnemyKind, Health, RegionCounts, Score, Velocity};
use voltex::ai::{NavAgent, NavMesh, NavMeshBuilder};
use voltex::ecs::Tag;
use voltex::physics::Collider;
use voltex::prelude::*;

// NOTE: 아래 개별 #[allow(dead_code)]는 main이 아직 level 모듈을 안 써서 임시.
// 후속 태스크(엔티티 스폰/게임 루프 연결)에서 호출자가 생기는 대로 제거.
pub const CELL: f32 = 2.0;
pub const WALL_H: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Wall,
    Floor,
    Door,
}

#[derive(Clone)]
pub struct LevelMap {
    pub w: usize,
    pub h: usize,
    cells: Vec<Cell>,
    pub player_spawn: (usize, usize),
    pub enemies: Vec<(usize, usize, EnemyKind)>,
    pub doors: Vec<(usize, usize)>,
    pub regions: Vec<Option<usize>>,
    pub region_count: usize,
    /// 문 i의 gate = 플레이어 출발 지점에 가까운 쪽 region id (BFS depth 기준) — 그 region 클리어 시 개방.
    pub door_gates: Vec<usize>,
}

impl LevelMap {
    #[allow(dead_code)]
    pub fn parse(rows: &[&str]) -> Result<Self, String> {
        let h = rows.len();
        let w = rows.first().map(|r| r.len()).ok_or("empty map")?;
        let mut cells = vec![Cell::Wall; w * h];
        let mut player = None;
        let mut enemies = Vec::new();
        let mut doors = Vec::new();
        for (z, row) in rows.iter().enumerate() {
            if row.len() != w {
                return Err(format!("row {z} width {} != {w}", row.len()));
            }
            for (x, ch) in row.chars().enumerate() {
                cells[z * w + x] = match ch {
                    '#' => Cell::Wall,
                    'D' => {
                        doors.push((x, z));
                        Cell::Door
                    }
                    '.' => Cell::Floor,
                    'P' => {
                        player = Some((x, z));
                        Cell::Floor
                    }
                    'c' => {
                        enemies.push((x, z, EnemyKind::Chaser));
                        Cell::Floor
                    }
                    's' => {
                        enemies.push((x, z, EnemyKind::Shooter));
                        Cell::Floor
                    }
                    'B' => {
                        enemies.push((x, z, EnemyKind::Boss));
                        Cell::Floor
                    }
                    other => return Err(format!("unknown cell '{other}' at ({x},{z})")),
                };
            }
        }
        let mut map = Self {
            w,
            h,
            cells,
            player_spawn: player.ok_or("map has no 'P' player spawn")?,
            enemies,
            doors,
            regions: Vec::new(),
            region_count: 0,
            door_gates: Vec::new(),
        };
        map.compute_regions();
        Ok(map)
    }

    pub fn cell(&self, x: usize, z: usize) -> Cell {
        self.cells[z * self.w + x]
    }

    /// 맵 중심이 월드 원점. +x = 그리드 x 증가, +z = 그리드 z 증가.
    pub fn world_pos(&self, x: usize, z: usize, y: f32) -> Vec3 {
        Vec3::new(
            (x as f32 - self.w as f32 / 2.0 + 0.5) * CELL,
            y,
            (z as f32 - self.h as f32 / 2.0 + 0.5) * CELL,
        )
    }

    pub fn cell_of(&self, p: Vec3) -> (usize, usize) {
        let x = (p.x / CELL + self.w as f32 / 2.0).floor();
        let z = (p.z / CELL + self.h as f32 / 2.0).floor();
        (
            x.clamp(0.0, self.w as f32 - 1.0) as usize,
            z.clamp(0.0, self.h as f32 - 1.0) as usize,
        )
    }

    pub fn region_at(&self, (x, z): (usize, usize)) -> Option<usize> {
        self.regions[z * self.w + x]
    }

    #[allow(dead_code)]
    pub fn region_of(&self, p: Vec3) -> Option<usize> {
        self.region_at(self.cell_of(p))
    }

    /// 문 셀 (x,z)에 인접한 Floor region id 목록 (중복 없음).
    fn door_adjacent_regions(&self, door: (usize, usize)) -> Vec<usize> {
        let (x, z) = door;
        let mut rs = Vec::new();
        for (nx, nz) in [
            (x.wrapping_sub(1), z),
            (x + 1, z),
            (x, z.wrapping_sub(1)),
            (x, z + 1),
        ] {
            if nx < self.w && nz < self.h {
                if let Some(r) = self.region_at((nx, nz)) {
                    if !rs.contains(&r) {
                        rs.push(r);
                    }
                }
            }
        }
        rs
    }

    fn compute_regions(&mut self) {
        let (w, h) = (self.w, self.h);
        self.regions = vec![None; w * h];
        self.region_count = 0;

        // 1) Floor 셀 플러드필 (Door는 경계 — Floor로 취급하지 않음)
        for start in 0..w * h {
            if self.cells[start] != Cell::Floor || self.regions[start].is_some() {
                continue;
            }
            let id = self.region_count;
            self.region_count += 1;
            let mut stack = vec![start];
            while let Some(i) = stack.pop() {
                if self.regions[i].is_some() || self.cells[i] != Cell::Floor {
                    continue;
                }
                self.regions[i] = Some(id);
                let (x, z) = (i % w, i / w);
                if x > 0 {
                    stack.push(i - 1);
                }
                if x + 1 < w {
                    stack.push(i + 1);
                }
                if z > 0 {
                    stack.push(i - w);
                }
                if z + 1 < h {
                    stack.push(i + w);
                }
            }
        }

        // 2) region 그래프 BFS depth (시작 = 플레이어 region, 간선 = 문)
        let start = self
            .region_at(self.player_spawn)
            .expect("player on floor");
        let mut depth = vec![usize::MAX; self.region_count];
        depth[start] = 0;
        let mut frontier = vec![start];
        while !frontier.is_empty() {
            let mut next = Vec::new();
            // 문 목록을 미리 복사해 &self 이중 차용 회피
            let doors: Vec<(usize, usize)> = self.doors.clone();
            for d in &doors {
                let rs = self.door_adjacent_regions(*d);
                if rs.len() == 2 {
                    for (a, b) in [(rs[0], rs[1]), (rs[1], rs[0])] {
                        if frontier.contains(&a) && depth[b] == usize::MAX {
                            depth[b] = depth[a] + 1;
                            next.push(b);
                        }
                    }
                }
            }
            frontier = next;
        }

        // 3) 문별 gate = 인접 region 중 depth 작은 쪽
        let doors: Vec<(usize, usize)> = self.doors.clone();
        self.door_gates = doors
            .iter()
            .map(|d| {
                let rs = self.door_adjacent_regions(*d);
                match rs.as_slice() {
                    [a, b] => {
                        if depth[*a] <= depth[*b] {
                            *a
                        } else {
                            *b
                        }
                    }
                    [a] => *a,
                    _ => start,
                }
            })
            .collect();
    }

    pub fn is_blocked(&self, x: usize, z: usize) -> bool {
        !matches!(self.cell(x, z), Cell::Floor)
    }

    pub fn blocked_at(&self, p: Vec3) -> bool {
        let (x, z) = self.cell_of(p);
        self.is_blocked(x, z)
    }

    /// 문 개방: Door → Floor. region 데이터는 재계산하지 않는다.
    #[allow(dead_code)]
    pub fn open_door(&mut self, (x, z): (usize, usize)) {
        self.cells[z * self.w + x] = Cell::Floor;
    }

    /// 0.25셀 간격 샘플링 시야 체크.
    #[allow(dead_code)]
    pub fn los_clear(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let steps = ((d.length() / (CELL * 0.25)).ceil() as usize).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            if self.blocked_at(a + d * t) {
                return false;
            }
        }
        true
    }
}

/// NavMesh 입력용 바닥 지오메트리: Floor + Door 셀당 y=0 쿼드(삼각형 2개).
/// 벽은 지오메트리에 없으므로 워크블 경계가 곧 벽이다.
#[allow(dead_code)]
pub fn level_geometry(map: &LevelMap) -> (Vec<Vec3>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let half = CELL / 2.0;
    for z in 0..map.h {
        for x in 0..map.w {
            if map.is_blocked(x, z) && map.cell(x, z) != Cell::Door {
                continue;
            }
            let c = map.world_pos(x, z, 0.0);
            let base = verts.len() as u32;
            verts.push(Vec3::new(c.x - half, 0.0, c.z - half));
            verts.push(Vec3::new(c.x + half, 0.0, c.z - half));
            verts.push(Vec3::new(c.x + half, 0.0, c.z + half));
            verts.push(Vec3::new(c.x - half, 0.0, c.z + half));
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (verts, idx)
}

/// 레벨 전체 NavMesh 빌드. 문은 항상 워크블 — 닫힌 문 통과는 적 활성화 로직이 막는다.
#[allow(dead_code)]
pub fn build_navmesh(map: &LevelMap) -> NavMesh {
    let (verts, idx) = level_geometry(map);
    NavMeshBuilder {
        cell_size: 0.5,
        cell_height: 0.2,
        agent_height: 2.0,
        agent_radius: 0.4,
        max_slope: 45.0,
    }
    .build(&verts, &idx)
}

pub const PLAYER_Y: f32 = 0.5;

/// World에 레벨 메시(바닥/벽/문) + 플레이어 + 적 + 리소스를 시딩한다.
#[allow(dead_code)]
pub fn build_level(world: &mut World, map: &LevelMap) {
    // 바닥 — 맵 전체 크기의 납작 큐브
    let mut t = Transform::from_position(Vec3::new(0.0, -0.05, 0.0));
    t.scale = Vec3::new(map.w as f32 * CELL, 0.1, map.h as f32 * CELL);
    world.spawn_bundle((t, PrimitiveMesh::cube(1.0), MaterialDesc::color([0.22, 0.22, 0.26, 1.0])));

    // 벽 + 문
    for z in 0..map.h {
        for x in 0..map.w {
            match map.cell(x, z) {
                Cell::Wall => {
                    let mut t = Transform::from_position(map.world_pos(x, z, WALL_H / 2.0));
                    t.scale = Vec3::new(CELL, WALL_H, CELL);
                    world.spawn_bundle((t, PrimitiveMesh::cube(1.0), MaterialDesc::color([0.45, 0.42, 0.50, 1.0])));
                }
                Cell::Door => {
                    let mut t = Transform::from_position(map.world_pos(x, z, WALL_H / 2.0));
                    t.scale = Vec3::new(CELL, WALL_H, CELL);
                    let gate = map.doors.iter().position(|&d| d == (x, z))
                        .map(|i| map.door_gates[i]).unwrap_or(0);
                    world.spawn_bundle((
                        t,
                        PrimitiveMesh::cube(1.0),
                        MaterialDesc::color([0.85, 0.65, 0.20, 1.0]),
                        Door { gate_region: gate, cell: (x, z) },
                        Tag("door".into()),
                    ));
                }
                Cell::Floor => {}
            }
        }
    }

    // 플레이어
    world.spawn_bundle((
        Transform::from_position(map.world_pos(map.player_spawn.0, map.player_spawn.1, PLAYER_Y)),
        Tag("player".into()),
        PrimitiveMesh::cube(0.8),
        MaterialDesc::color([0.20, 0.85, 0.35, 1.0]),
        Health { hp: 5 },
        Collider::Sphere { radius: 0.5 },
    ));

    // 적 + region 카운트
    let mut counts = vec![0u32; map.region_count.max(1)];
    for &(x, z, kind) in &map.enemies {
        let region = map.region_at((x, z)).unwrap_or(0);
        counts[region] += 1;
        let (size, color, speed, hp) = match kind {
            EnemyKind::Chaser  => (0.45, [0.90_f32, 0.25, 0.20, 1.0], 3.2_f32, 2_i32),
            EnemyKind::Shooter => (0.45, [0.95_f32, 0.55, 0.15, 1.0], 2.4_f32, 2_i32),
            EnemyKind::Boss    => (1.1,  [0.60_f32, 0.20, 0.80, 1.0], 2.0_f32, 30_i32),
        };
        world.spawn_bundle((
            Transform::from_position(map.world_pos(x, z, PLAYER_Y)),
            PrimitiveMesh::sphere_default(size),
            MaterialDesc::color(color),
            Enemy::new(kind, region),
            NavAgent::new(speed),
            Health { hp },
            Collider::Sphere { radius: size },
            Velocity(Vec3::new(0.0, 0.0, 0.0)),
        ));
    }
    world.insert_resource(RegionCounts(counts));
    world.insert_resource(Score(0));
}

/// 축 분리 이동: x/z 각각 독립으로 시도 → 벽에 비비면 미끄러진다.
#[allow(dead_code)]
pub fn try_move(map: &LevelMap, from: Vec3, delta: Vec3, radius: f32) -> Vec3 {
    let mut pos = from;
    let probe_x = Vec3::new(
        pos.x + delta.x + delta.x.signum() * radius,
        pos.y,
        pos.z,
    );
    if !map.blocked_at(probe_x) {
        pos.x += delta.x;
    }
    let probe_z = Vec3::new(
        pos.x,
        pos.y,
        pos.z + delta.z + delta.z.signum() * radius,
    );
    if !map.blocked_at(probe_z) {
        pos.z += delta.z;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex::prelude::World;

    pub(crate) const T_MAP: &[&str] = &[
        "#########",
        "#...#...#",
        "#.P.D.c.#",
        "#...#.s.#",
        "#########",
    ];

    #[test]
    fn parse_extracts_cells_and_spawns() {
        let m = LevelMap::parse(T_MAP).unwrap();
        assert_eq!((m.w, m.h), (9, 5));
        assert_eq!(m.cell(0, 0), Cell::Wall);
        assert_eq!(m.cell(1, 1), Cell::Floor);
        assert_eq!(m.cell(4, 2), Cell::Door);
        assert_eq!(m.player_spawn, (2, 2));
        assert_eq!(
            m.enemies,
            vec![
                (6, 2, EnemyKind::Chaser),
                (6, 3, EnemyKind::Shooter)
            ]
        );
        assert_eq!(m.doors, vec![(4, 2)]);
    }

    #[test]
    fn world_pos_roundtrips_with_cell_of() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let p = m.world_pos(6, 2, 0.5);
        assert_eq!(m.cell_of(p), (6, 2));
        let c = m.world_pos(4, 2, 0.0);
        assert!(c.x.abs() < 1e-5 && c.z.abs() < 1e-5);
    }

    #[test]
    fn parse_rejects_map_without_player() {
        assert!(LevelMap::parse(&["###", "#.#", "###"]).is_err());
    }

    #[test]
    fn regions_split_by_doors_and_gates_point_backward() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let rp = m.region_at(m.player_spawn).unwrap();
        let rc = m.region_at((6, 2)).unwrap();
        assert_ne!(rp, rc);
        assert_eq!(m.region_count, 2);
        assert_eq!(m.door_gates[0], rp);
        assert_eq!(m.region_at((4, 2)), None);
    }

    #[test]
    fn three_room_chain_gates_are_sequential() {
        let m = LevelMap::parse(&[
            "###########",
            "#P.#.c#.B.#",
            "#..D..D...#",
            "###########",
        ])
        .unwrap();
        assert_eq!(m.region_count, 3);
        let r0 = m.region_at(m.player_spawn).unwrap();
        let r1 = m.region_at((5, 1)).unwrap();
        assert_eq!(m.door_gates[0], r0);
        assert_eq!(m.door_gates[1], r1);
    }

    #[test]
    fn try_move_blocks_walls_and_slides() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let p = m.world_pos(1, 1, 0.5);
        let out = try_move(&m, p, Vec3::new(-1.0, 0.0, 0.6), 0.4);
        assert!(out.x >= p.x - 1e-4, "wall must block -x");
        assert!(out.z > p.z, "z component must slide");
    }

    #[test]
    fn closed_door_blocks_open_door_passes() {
        let mut m = LevelMap::parse(T_MAP).unwrap();
        let before = m.world_pos(3, 2, 0.5);
        let blocked = try_move(&m, before, Vec3::new(CELL, 0.0, 0.0), 0.4);
        assert!(
            blocked.x < m.world_pos(4, 2, 0.0).x - 0.5,
            "closed door blocks"
        );
        m.open_door((4, 2));
        let passed = try_move(&m, before, Vec3::new(CELL, 0.0, 0.0), 0.4);
        assert!(passed.x > blocked.x, "open door passes");
    }

    #[test]
    fn geometry_emits_quads_for_walkable_cells_only() {
        let m = LevelMap::parse(&["####", "#P.#", "####"]).unwrap();
        let (verts, idx) = level_geometry(&m);
        assert_eq!(verts.len(), 2 * 4); // 2 walkable cells × 4 verts
        assert_eq!(idx.len(), 2 * 6); // × 2 tri × 3
        assert!(verts.iter().all(|v| v.y == 0.0));
    }

    #[test]
    fn navmesh_path_detours_around_wall() {
        // 가운데 벽 기둥을 우회해야 하는 ㄷ자 맵
        let m = LevelMap::parse(&[
            "#######",
            "#P....#",
            "#.###.#",
            "#.....#",
            "#######",
        ])
        .unwrap();
        let nav = build_navmesh(&m);
        let start = m.world_pos(1, 1, 0.0);
        let goal = m.world_pos(5, 1, 0.0);
        let path = voltex::ai::find_path(&nav, start, goal)
            .expect("path must exist around the pillar");
        // 직선이면 2 waypoint — 우회는 더 많다
        assert!(path.len() > 2, "expected detour, got straight line: {path:?}");
        // 경로 위 샘플이 벽을 통과하지 않는다
        for w in path.windows(2) {
            assert!(
                m.los_clear(
                    Vec3::new(w[0].x, 0.5, w[0].z),
                    Vec3::new(w[1].x, 0.5, w[1].z),
                ),
                "path segment crosses a wall: {w:?}"
            );
        }
    }

    #[test]
    fn build_level_seeds_entities_and_region_counts() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let mut w = World::new();
        build_level(&mut w, &m);
        assert!(w.find_by_tag("player").is_some());
        let enemies: Vec<_> = w.query::<crate::components::Enemy>().collect();
        assert_eq!(enemies.len(), 2);
        let doors: Vec<_> = w.query::<crate::components::Door>().collect();
        assert_eq!(doors.len(), 1);
        let rc = w.resource::<crate::components::RegionCounts>().unwrap();
        assert_eq!(rc.0.iter().sum::<u32>(), 2);
        let p = w.find_by_tag("player").unwrap();
        assert_eq!(w.get::<crate::components::Health>(p).unwrap().hp, 5);
        assert!(w.get::<voltex::physics::Collider>(p).is_some());
    }

    #[test]
    fn los_blocked_by_wall_clear_in_open() {
        let m = LevelMap::parse(T_MAP).unwrap();
        let a = m.world_pos(1, 1, 0.5);
        let b = m.world_pos(1, 3, 0.5);
        assert!(m.los_clear(a, b));
        let c = m.world_pos(6, 2, 0.5);
        assert!(!m.los_clear(a, c));
    }
}
