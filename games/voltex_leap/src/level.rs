//! ASCII 맵 파싱 + 그리드↔월드 좌표. 그리드 좌표는 월드 방향(gy 0 = 최하단).

use voltex::prelude::Vec3;

/// 타일 한 변의 월드 길이.
pub const TILE: f32 = 1.0;

/// 파싱된 레벨. 엔티티 좌표는 전부 그리드 `(gx, gy)`, gy는 월드 방향.
#[derive(Clone, Debug)]
pub struct LevelMap {
    pub cols: usize,
    pub rows: usize,
    solids: Vec<bool>,
    pub spikes: Vec<(usize, usize)>,
    pub coins: Vec<(usize, usize)>,
    pub walkers: Vec<(usize, usize)>,
    pub player_spawn: (usize, usize),
    pub door: (usize, usize),
}

impl LevelMap {
    /// 문자: `#` 솔리드, `^` 가시, `o` 코인, `E` 워커, `P` 스폰(정확히 1),
    /// `D` 문(정확히 1), `.`/공백 빈칸. 행 길이 불일치·P/D 개수 오류는 Err.
    pub fn parse(rows: &[&str]) -> Result<LevelMap, String> {
        Self::parse_impl(rows, true)
    }

    /// 테스트 전용 — P/D 부재를 허용 (부재 시 (0,0)).
    #[cfg(test)]
    pub fn parse_no_door(rows: &[&str]) -> Result<LevelMap, String> {
        Self::parse_impl(rows, false)
    }

    fn parse_impl(rows: &[&str], require_pd: bool) -> Result<LevelMap, String> {
        if rows.is_empty() {
            return Err("empty map".into());
        }
        let cols = rows[0].chars().count();
        let nrows = rows.len();
        let mut solids = vec![false; cols * nrows];
        let mut spikes = Vec::new();
        let mut coins = Vec::new();
        let mut walkers = Vec::new();
        let mut spawn = None;
        let mut door = None;
        for (ri, line) in rows.iter().enumerate() {
            if line.chars().count() != cols {
                return Err(format!("row {ri}: length != {cols}"));
            }
            let gy = nrows - 1 - ri; // ASCII 최상단 행 → 최대 gy
            for (gx, ch) in line.chars().enumerate() {
                match ch {
                    '#' => solids[gy * cols + gx] = true,
                    '^' => spikes.push((gx, gy)),
                    'o' => coins.push((gx, gy)),
                    'E' => walkers.push((gx, gy)),
                    'P' => {
                        if spawn.replace((gx, gy)).is_some() {
                            return Err("multiple P".into());
                        }
                    }
                    'D' => {
                        if door.replace((gx, gy)).is_some() {
                            return Err("multiple D".into());
                        }
                    }
                    '.' | ' ' => {}
                    c => return Err(format!("unknown tile '{c}'")),
                }
            }
        }
        if require_pd && (spawn.is_none() || door.is_none()) {
            return Err("map needs exactly one P and one D".into());
        }
        Ok(LevelMap {
            cols,
            rows: nrows,
            solids,
            spikes,
            coins,
            walkers,
            player_spawn: spawn.unwrap_or((0, 0)),
            door: door.unwrap_or((0, 0)),
        })
    }

    /// 맵 밖 x는 솔리드(벽 취급), 맵 밖 y는 빈칸(점프/낙사 허용).
    pub fn is_solid(&self, gx: i32, gy: i32) -> bool {
        if gx < 0 || gx >= self.cols as i32 {
            return true;
        }
        if gy < 0 || gy >= self.rows as i32 {
            return false;
        }
        self.solids[gy as usize * self.cols + gx as usize]
    }
}

/// 셀 `(gx, gy)`의 월드 중심 (z=0).
pub fn cell_center(gx: usize, gy: usize) -> Vec3 {
    Vec3::new(
        gx as f32 * TILE + TILE * 0.5,
        gy as f32 * TILE + TILE * 0.5,
        0.0,
    )
}

/// 월드 좌표가 속한 셀 (floor — 음수 좌표도 올바름).
pub fn grid_at(pos_x: f32, pos_y: f32) -> (i32, i32) {
    ((pos_x / TILE).floor() as i32, (pos_y / TILE).floor() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &[&str] = &[
        "................",
        "..P...o....E..D.",
        "####..####..####",
    ];

    #[test]
    fn parses_entities_into_world_oriented_grid() {
        let m = LevelMap::parse(MAP).unwrap();
        assert_eq!((m.cols, m.rows), (16, 3));
        // ASCII 행 1(중단) → 월드 gy 1. P는 (2,1).
        assert_eq!(m.player_spawn, (2, 1));
        assert_eq!(m.door, (14, 1));
        assert_eq!(m.coins, vec![(6, 1)]);
        assert_eq!(m.walkers, vec![(11, 1)]);
        // ASCII 최하단 행 → 월드 gy 0.
        assert!(m.is_solid(0, 0));
        assert!(!m.is_solid(4, 0)); // 갭
    }

    #[test]
    fn out_of_bounds_x_is_solid_but_y_is_open() {
        let m = LevelMap::parse(MAP).unwrap();
        assert!(m.is_solid(-1, 0));
        assert!(m.is_solid(m.cols as i32, 0));
        assert!(!m.is_solid(2, -1));
        assert!(!m.is_solid(2, m.rows as i32));
    }

    #[test]
    fn rejects_bad_maps() {
        assert!(LevelMap::parse(&["##", "#"]).is_err()); // 행 길이
        assert!(LevelMap::parse(&["..", ".."]).is_err()); // P 없음
        assert!(LevelMap::parse(&["PP", "D."]).is_err()); // P 2개
    }

    #[test]
    fn coordinate_round_trip() {
        let c = cell_center(3, 2);
        assert_eq!(grid_at(c.x, c.y), (3, 2));
        assert_eq!(grid_at(-0.1, 0.0), (-1, 0)); // floor, truncation 아님
    }
}
