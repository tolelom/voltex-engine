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

    fn compute_regions(&mut self) {} // Task 3에서 구현
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
}
