//! ASCII 그리드 → 레벨 데이터. 전부 순수 (GPU/World 무관).
#![allow(dead_code)]
use voltex::prelude::*;
use crate::components::EnemyKind;

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
    pub regions: Vec<Option<usize>>, // Task 3
    pub region_count: usize,         // Task 3
    pub door_gates: Vec<usize>,      // Task 3
}

impl LevelMap {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
