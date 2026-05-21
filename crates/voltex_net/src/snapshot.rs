//! Snapshot + delta encoding for replicated world state. Defines
//! [`EntityState`] / [`Snapshot`] plus binary `serialize_snapshot` /
//! `deserialize_snapshot` and `diff_snapshots` / `apply_diff` helpers used by
//! the `Snapshot` and `SnapshotDelta` packet variants.

/// State of a single entity at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityState {
    /// Entity id (typically `Entity::id`).
    pub id: u32,
    /// World position `[x, y, z]`.
    pub position: [f32; 3],
    /// Euler rotation `[pitch, yaw, roll]` in radians.
    pub rotation: [f32; 3],
    /// Linear velocity `[vx, vy, vz]`.
    pub velocity: [f32; 3],
}

/// A snapshot of the world at a given tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Server tick this snapshot represents.
    pub tick: u32,
    /// All entities tracked at this tick.
    pub entities: Vec<EntityState>,
}

/// Binary size of one entity: id(4) + pos(12) + rot(12) + vel(12) = 40 bytes
const ENTITY_SIZE: usize = 4 + 12 + 12 + 12;

fn write_f32_le(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn read_f32_le(data: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn write_f32x3(buf: &mut Vec<u8>, v: &[f32; 3]) {
    write_f32_le(buf, v[0]);
    write_f32_le(buf, v[1]);
    write_f32_le(buf, v[2]);
}

fn read_f32x3(data: &[u8], offset: usize) -> [f32; 3] {
    [
        read_f32_le(data, offset),
        read_f32_le(data, offset + 4),
        read_f32_le(data, offset + 8),
    ]
}

fn serialize_entity(buf: &mut Vec<u8>, e: &EntityState) {
    buf.extend_from_slice(&e.id.to_le_bytes());
    write_f32x3(buf, &e.position);
    write_f32x3(buf, &e.rotation);
    write_f32x3(buf, &e.velocity);
}

fn deserialize_entity(data: &[u8], offset: usize) -> EntityState {
    let id = u32::from_le_bytes([
        data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
    ]);
    let position = read_f32x3(data, offset + 4);
    let rotation = read_f32x3(data, offset + 16);
    let velocity = read_f32x3(data, offset + 28);
    EntityState { id, position, rotation, velocity }
}

/// Serialize a snapshot into compact binary format.
/// Layout: tick(4 LE) + entity_count(4 LE) + entities...
pub fn serialize_snapshot(snapshot: &Snapshot) -> Vec<u8> {
    let count = snapshot.entities.len() as u32;
    let mut buf = Vec::with_capacity(8 + ENTITY_SIZE * snapshot.entities.len());
    buf.extend_from_slice(&snapshot.tick.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    for e in &snapshot.entities {
        serialize_entity(&mut buf, e);
    }
    buf
}

/// Deserialize a snapshot from binary data.
pub fn deserialize_snapshot(data: &[u8]) -> crate::Result<Snapshot> {
    if data.len() < 8 {
        return Err("Snapshot data too short for header".into());
    }
    let tick = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    let expected = 8 + count * ENTITY_SIZE;
    if data.len() < expected {
        return Err(format!(
            "Snapshot data too short: expected {} bytes, got {}",
            expected,
            data.len()
        ).into());
    }

    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        entities.push(deserialize_entity(data, 8 + i * ENTITY_SIZE));
    }

    Ok(Snapshot { tick, entities })
}

/// Compute a delta between two snapshots.
/// Format: new_tick(4) + count(4) + [id(4) + flags(1) + changed_fields...]
/// Flags bitmask: 0x01 = position, 0x02 = rotation, 0x04 = velocity, 0x80 = new entity (full)
pub fn diff_snapshots(old: &Snapshot, new: &Snapshot) -> Vec<u8> {
    use std::collections::HashMap;

    let old_map: HashMap<u32, &EntityState> = old.entities.iter().map(|e| (e.id, e)).collect();

    let mut entries: Vec<u8> = Vec::new();
    let mut count: u32 = 0;

    for new_ent in &new.entities {
        if let Some(old_ent) = old_map.get(&new_ent.id) {
            let mut flags: u8 = 0;
            let mut fields = Vec::new();

            if new_ent.position != old_ent.position {
                flags |= 0x01;
                write_f32x3(&mut fields, &new_ent.position);
            }
            if new_ent.rotation != old_ent.rotation {
                flags |= 0x02;
                write_f32x3(&mut fields, &new_ent.rotation);
            }
            if new_ent.velocity != old_ent.velocity {
                flags |= 0x04;
                write_f32x3(&mut fields, &new_ent.velocity);
            }

            if flags != 0 {
                entries.extend_from_slice(&new_ent.id.to_le_bytes());
                entries.push(flags);
                entries.extend_from_slice(&fields);
                count += 1;
            }
        } else {
            // New entity — send full state
            entries.extend_from_slice(&new_ent.id.to_le_bytes());
            entries.push(0x80); // "new entity" flag
            write_f32x3(&mut entries, &new_ent.position);
            write_f32x3(&mut entries, &new_ent.rotation);
            write_f32x3(&mut entries, &new_ent.velocity);
            count += 1;
        }
    }

    let mut buf = Vec::with_capacity(8 + entries.len());
    buf.extend_from_slice(&new.tick.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&entries);
    buf
}

/// Apply a delta to a base snapshot to produce an updated snapshot.
pub fn apply_diff(base: &Snapshot, diff: &[u8]) -> crate::Result<Snapshot> {
    if diff.len() < 8 {
        return Err("Diff data too short for header".into());
    }

    let tick = u32::from_le_bytes([diff[0], diff[1], diff[2], diff[3]]);
    let count = u32::from_le_bytes([diff[4], diff[5], diff[6], diff[7]]) as usize;

    // Start from a clone of the base
    let mut entities: Vec<EntityState> = base.entities.clone();
    let mut offset = 8;

    for _ in 0..count {
        if offset + 5 > diff.len() {
            return Err("Diff truncated at entry header".into());
        }
        let id = u32::from_le_bytes([diff[offset], diff[offset + 1], diff[offset + 2], diff[offset + 3]]);
        let flags = diff[offset + 4];
        offset += 5;

        if flags & 0x80 != 0 {
            // New entity — full state
            if offset + 36 > diff.len() {
                return Err("Diff truncated at new entity data".into());
            }
            let position = read_f32x3(diff, offset);
            let rotation = read_f32x3(diff, offset + 12);
            let velocity = read_f32x3(diff, offset + 24);
            offset += 36;

            // Add or replace
            if let Some(ent) = entities.iter_mut().find(|e| e.id == id) {
                ent.position = position;
                ent.rotation = rotation;
                ent.velocity = velocity;
            } else {
                entities.push(EntityState { id, position, rotation, velocity });
            }
        } else {
            // Delta update — find existing entity
            let ent = entities.iter_mut().find(|e| e.id == id)
                .ok_or_else(|| crate::NetError::from(format!("Diff references unknown entity {}", id)))?;

            if flags & 0x01 != 0 {
                if offset + 12 > diff.len() {
                    return Err("Diff truncated at position".into());
                }
                ent.position = read_f32x3(diff, offset);
                offset += 12;
            }
            if flags & 0x02 != 0 {
                if offset + 12 > diff.len() {
                    return Err("Diff truncated at rotation".into());
                }
                ent.rotation = read_f32x3(diff, offset);
                offset += 12;
            }
            if flags & 0x04 != 0 {
                if offset + 12 > diff.len() {
                    return Err("Diff truncated at velocity".into());
                }
                ent.velocity = read_f32x3(diff, offset);
                offset += 12;
            }
        }
    }

    Ok(Snapshot { tick, entities })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(id: u32, px: f32, py: f32, pz: f32) -> EntityState {
        EntityState {
            id,
            position: [px, py, pz],
            rotation: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let snap = Snapshot {
            tick: 42,
            entities: vec![
                make_entity(1, 1.0, 2.0, 3.0),
                make_entity(2, 4.0, 5.0, 6.0),
            ],
        };

        let bytes = serialize_snapshot(&snap);
        let decoded = deserialize_snapshot(&bytes).expect("deserialize failed");
        assert_eq!(snap, decoded);
    }

    #[test]
    fn test_snapshot_empty() {
        let snap = Snapshot { tick: 0, entities: vec![] };
        let bytes = serialize_snapshot(&snap);
        assert_eq!(bytes.len(), 8); // just header
        let decoded = deserialize_snapshot(&bytes).unwrap();
        assert_eq!(snap, decoded);
    }

    #[test]
    fn test_diff_no_changes() {
        let snap = Snapshot {
            tick: 10,
            entities: vec![make_entity(1, 1.0, 2.0, 3.0)],
        };
        let snap2 = Snapshot {
            tick: 11,
            entities: vec![make_entity(1, 1.0, 2.0, 3.0)],
        };

        let diff = diff_snapshots(&snap, &snap2);
        // Header only: tick(4) + count(4) = 8, count = 0
        assert_eq!(diff.len(), 8);
        let count = u32::from_le_bytes([diff[4], diff[5], diff[6], diff[7]]);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_diff_position_changed() {
        let old = Snapshot {
            tick: 10,
            entities: vec![make_entity(1, 0.0, 0.0, 0.0)],
        };
        let new = Snapshot {
            tick: 11,
            entities: vec![EntityState {
                id: 1,
                position: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        };

        let diff = diff_snapshots(&old, &new);
        let result = apply_diff(&old, &diff).expect("apply_diff failed");

        assert_eq!(result.tick, 11);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(result.entities[0].rotation, [0.0, 0.0, 0.0]); // unchanged
    }

    #[test]
    fn test_diff_new_entity() {
        let old = Snapshot {
            tick: 10,
            entities: vec![make_entity(1, 0.0, 0.0, 0.0)],
        };
        let new = Snapshot {
            tick: 11,
            entities: vec![
                make_entity(1, 0.0, 0.0, 0.0),
                make_entity(2, 5.0, 6.0, 7.0),
            ],
        };

        let diff = diff_snapshots(&old, &new);
        let result = apply_diff(&old, &diff).expect("apply_diff failed");

        assert_eq!(result.entities.len(), 2);
        assert_eq!(result.entities[1].id, 2);
        assert_eq!(result.entities[1].position, [5.0, 6.0, 7.0]);
    }

    #[test]
    fn test_diff_multiple_fields_changed() {
        let old = Snapshot {
            tick: 10,
            entities: vec![EntityState {
                id: 1,
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        };
        let new = Snapshot {
            tick: 11,
            entities: vec![EntityState {
                id: 1,
                position: [1.0, 1.0, 1.0],
                rotation: [2.0, 2.0, 2.0],
                velocity: [3.0, 3.0, 3.0],
            }],
        };

        let diff = diff_snapshots(&old, &new);
        let result = apply_diff(&old, &diff).unwrap();

        assert_eq!(result.entities[0].position, [1.0, 1.0, 1.0]);
        assert_eq!(result.entities[0].rotation, [2.0, 2.0, 2.0]);
        assert_eq!(result.entities[0].velocity, [3.0, 3.0, 3.0]);
    }

    #[test]
    fn test_diff_is_compact() {
        // Only position changes — diff should be smaller than full snapshot
        let old = Snapshot {
            tick: 10,
            entities: vec![make_entity(1, 0.0, 0.0, 0.0)],
        };
        let new = Snapshot {
            tick: 11,
            entities: vec![EntityState {
                id: 1,
                position: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        };

        let full_bytes = serialize_snapshot(&new);
        let diff_bytes = diff_snapshots(&old, &new);
        assert!(
            diff_bytes.len() < full_bytes.len(),
            "Diff ({} bytes) should be smaller than full snapshot ({} bytes)",
            diff_bytes.len(),
            full_bytes.len()
        );
    }
}
