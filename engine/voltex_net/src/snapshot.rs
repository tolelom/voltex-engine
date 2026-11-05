//! Snapshot + delta encoding for replicated world state. Defines
//! [`EntityState`] / [`Snapshot`] plus binary `serialize_snapshot` /
//! `deserialize_snapshot` and `diff_snapshots` / `apply_snapshot_diff` helpers used by
//! the `Snapshot` and `SnapshotDelta` packet variants.

/// State of a single entity at a point in time.
///
/// Wire format changed in E3b: `rotation` is now a unit quaternion
/// `[x, y, z, w]` instead of the prior 3-float Euler triple. Old peers
/// cannot decode new packets and vice versa — this is an intentional
/// hard cutover for an ephemeral wire format (snapshots are not persisted).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityState {
    /// Entity id (typically `Entity::id`).
    pub id: u32,
    /// World position `[x, y, z]`.
    pub position: [f32; 3],
    /// Orientation quaternion `[x, y, z, w]` (Hamilton, right-handed).
    pub rotation: [f32; 4],
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

use crate::wire::{read_f32, read_u32};

/// Snapshot and delta header: tick(4 LE) + entry count(4 LE).
const HEADER_LEN: usize = 8;
/// Byte sizes of the encoded entity fields.
const ID_LEN: usize = 4;
const POSITION_LEN: usize = 12;
const ROTATION_LEN: usize = 16;
const VELOCITY_LEN: usize = 12;
/// Full entity state without the id: pos(12) + rot quat(16) + vel(12) = 40 bytes.
const FULL_STATE_LEN: usize = POSITION_LEN + ROTATION_LEN + VELOCITY_LEN;
/// Binary size of one entity: id(4) + full state(40) = 44 bytes
const ENTITY_SIZE: usize = ID_LEN + FULL_STATE_LEN;
/// Delta entry header: id(4) + flags(1).
const DELTA_ENTRY_HEADER_LEN: usize = ID_LEN + 1;

/// Delta entry flags (bitmask).
const FLAG_POSITION: u8 = 0x01;
const FLAG_ROTATION: u8 = 0x02;
const FLAG_VELOCITY: u8 = 0x04;
/// Entity removed — no field data follows.
const FLAG_REMOVED: u8 = 0x40;
/// Entity added — full state follows.
const FLAG_NEW: u8 = 0x80;

fn write_f32_le(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_f32x3(buf: &mut Vec<u8>, v: &[f32; 3]) {
    write_f32_le(buf, v[0]);
    write_f32_le(buf, v[1]);
    write_f32_le(buf, v[2]);
}

fn read_f32x3(data: &[u8], offset: usize) -> [f32; 3] {
    [
        read_f32(data, offset),
        read_f32(data, offset + 4),
        read_f32(data, offset + 8),
    ]
}

fn write_f32x4(buf: &mut Vec<u8>, v: &[f32; 4]) {
    write_f32_le(buf, v[0]);
    write_f32_le(buf, v[1]);
    write_f32_le(buf, v[2]);
    write_f32_le(buf, v[3]);
}

fn read_f32x4(data: &[u8], offset: usize) -> [f32; 4] {
    [
        read_f32(data, offset),
        read_f32(data, offset + 4),
        read_f32(data, offset + 8),
        read_f32(data, offset + 12),
    ]
}

fn serialize_entity(buf: &mut Vec<u8>, e: &EntityState) {
    buf.extend_from_slice(&e.id.to_le_bytes());
    write_f32x3(buf, &e.position);
    write_f32x4(buf, &e.rotation);
    write_f32x3(buf, &e.velocity);
}

/// Reads position + rotation + velocity ([`FULL_STATE_LEN`] bytes) at `offset`.
fn read_full_state(data: &[u8], offset: usize) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let position = read_f32x3(data, offset);
    let rotation = read_f32x4(data, offset + POSITION_LEN);
    let velocity = read_f32x3(data, offset + POSITION_LEN + ROTATION_LEN);
    (position, rotation, velocity)
}

fn deserialize_entity(data: &[u8], offset: usize) -> EntityState {
    let id = read_u32(data, offset);
    let (position, rotation, velocity) = read_full_state(data, offset + ID_LEN);
    EntityState { id, position, rotation, velocity }
}

/// Serialize a snapshot into compact binary format.
/// Layout: tick(4 LE) + entity_count(4 LE) + entities...
pub fn serialize_snapshot(snapshot: &Snapshot) -> Vec<u8> {
    let count = snapshot.entities.len() as u32;
    let mut buf = Vec::with_capacity(HEADER_LEN + ENTITY_SIZE * snapshot.entities.len());
    buf.extend_from_slice(&snapshot.tick.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    for e in &snapshot.entities {
        serialize_entity(&mut buf, e);
    }
    buf
}

/// Maximum entity count we will accept in a single snapshot. Snapshots
/// arrive over the network from potentially untrusted peers; a header
/// declaring billions of entities used to overflow `count * ENTITY_SIZE`
/// on 32-bit `usize` and could trigger huge `Vec::with_capacity`
/// allocations on 64-bit. UDP datagrams are capped well below this in
/// practice, so a 1 M ceiling is generous.
const MAX_SNAPSHOT_ENTITIES: usize = 1_000_000;

/// Deserialize a snapshot from binary data.
pub fn deserialize_snapshot(data: &[u8]) -> crate::Result<Snapshot> {
    if data.len() < HEADER_LEN {
        return Err("Snapshot data too short for header".into());
    }
    let tick = read_u32(data, 0);
    let count = read_u32(data, 4) as usize;

    if count > MAX_SNAPSHOT_ENTITIES {
        return Err(format!(
            "Snapshot entity count {} exceeds limit {}",
            count, MAX_SNAPSHOT_ENTITIES
        ).into());
    }

    let body_size = count
        .checked_mul(ENTITY_SIZE)
        .ok_or("Snapshot body size overflow")?;
    let expected = body_size
        .checked_add(HEADER_LEN)
        .ok_or("Snapshot expected size overflow")?;
    if data.len() < expected {
        return Err(format!(
            "Snapshot data too short: expected {} bytes, got {}",
            expected,
            data.len()
        ).into());
    }

    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        entities.push(deserialize_entity(data, HEADER_LEN + i * ENTITY_SIZE));
    }

    Ok(Snapshot { tick, entities })
}

/// Compute a delta between two snapshots.
/// Format: new_tick(4) + count(4) + [id(4) + flags(1) + changed_fields...]
/// Flags bitmask: [`FLAG_POSITION`] 0x01, [`FLAG_ROTATION`] 0x02,
/// [`FLAG_VELOCITY`] 0x04, [`FLAG_REMOVED`] 0x40 (no field data),
/// [`FLAG_NEW`] 0x80 (full state).
pub fn diff_snapshots(old: &Snapshot, new: &Snapshot) -> Vec<u8> {
    use std::collections::{HashMap, HashSet};

    let old_map: HashMap<u32, &EntityState> = old.entities.iter().map(|e| (e.id, e)).collect();

    let mut entries: Vec<u8> = Vec::new();
    let mut count: u32 = 0;

    for new_ent in &new.entities {
        if let Some(old_ent) = old_map.get(&new_ent.id) {
            let mut flags: u8 = 0;
            let mut fields = Vec::new();

            if new_ent.position != old_ent.position {
                flags |= FLAG_POSITION;
                write_f32x3(&mut fields, &new_ent.position);
            }
            if new_ent.rotation != old_ent.rotation {
                flags |= FLAG_ROTATION;
                write_f32x4(&mut fields, &new_ent.rotation);
            }
            if new_ent.velocity != old_ent.velocity {
                flags |= FLAG_VELOCITY;
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
            entries.push(FLAG_NEW);
            write_f32x3(&mut entries, &new_ent.position);
            write_f32x4(&mut entries, &new_ent.rotation);
            write_f32x3(&mut entries, &new_ent.velocity);
            count += 1;
        }
    }

    // Removed entities: present in `old` but absent in `new`. Without these
    // the receiver (which starts from a clone of the base) would keep
    // despawned entities forever.
    let new_ids: HashSet<u32> = new.entities.iter().map(|e| e.id).collect();
    for old_ent in &old.entities {
        if !new_ids.contains(&old_ent.id) {
            entries.extend_from_slice(&old_ent.id.to_le_bytes());
            entries.push(FLAG_REMOVED);
            count += 1;
        }
    }

    let mut buf = Vec::with_capacity(HEADER_LEN + entries.len());
    buf.extend_from_slice(&new.tick.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&entries);
    buf
}

/// Apply a delta from [`diff_snapshots`] to a base snapshot to produce the
/// updated snapshot.
pub fn apply_snapshot_diff(base: &Snapshot, diff: &[u8]) -> crate::Result<Snapshot> {
    if diff.len() < HEADER_LEN {
        return Err("Diff data too short for header".into());
    }

    let tick = read_u32(diff, 0);
    let count = read_u32(diff, 4) as usize;

    // Start from a clone of the base
    let mut entities: Vec<EntityState> = base.entities.clone();
    let mut offset = HEADER_LEN;

    for _ in 0..count {
        if offset + DELTA_ENTRY_HEADER_LEN > diff.len() {
            return Err("Diff truncated at entry header".into());
        }
        let id = read_u32(diff, offset);
        let flags = diff[offset + ID_LEN];
        offset += DELTA_ENTRY_HEADER_LEN;

        if flags & FLAG_REMOVED != 0 {
            entities.retain(|e| e.id != id);
            continue;
        }

        if flags & FLAG_NEW != 0 {
            if offset + FULL_STATE_LEN > diff.len() {
                return Err("Diff truncated at new entity data".into());
            }
            let (position, rotation, velocity) = read_full_state(diff, offset);
            offset += FULL_STATE_LEN;

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

            if flags & FLAG_POSITION != 0 {
                if offset + POSITION_LEN > diff.len() {
                    return Err("Diff truncated at position".into());
                }
                ent.position = read_f32x3(diff, offset);
                offset += POSITION_LEN;
            }
            if flags & FLAG_ROTATION != 0 {
                if offset + ROTATION_LEN > diff.len() {
                    return Err("Diff truncated at rotation".into());
                }
                ent.rotation = read_f32x4(diff, offset);
                offset += ROTATION_LEN;
            }
            if flags & FLAG_VELOCITY != 0 {
                if offset + VELOCITY_LEN > diff.len() {
                    return Err("Diff truncated at velocity".into());
                }
                ent.velocity = read_f32x3(diff, offset);
                offset += VELOCITY_LEN;
            }
        }
    }

    Ok(Snapshot { tick, entities })
}

/// Old name of [`apply_snapshot_diff`].
#[deprecated(note = "use `apply_snapshot_diff`")]
pub fn apply_diff(base: &Snapshot, diff: &[u8]) -> crate::Result<Snapshot> {
    apply_snapshot_diff(base, diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(id: u32, px: f32, py: f32, pz: f32) -> EntityState {
        EntityState {
            id,
            position: [px, py, pz],
            rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
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
                rotation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        };

        let diff = diff_snapshots(&old, &new);
        let result = apply_snapshot_diff(&old, &diff).expect("apply_snapshot_diff failed");

        assert_eq!(result.tick, 11);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(result.entities[0].rotation, [0.0, 0.0, 0.0, 1.0]); // unchanged
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
        let result = apply_snapshot_diff(&old, &diff).expect("apply_snapshot_diff failed");

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
                rotation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0, 0.0, 0.0],
            }],
        };
        let new = Snapshot {
            tick: 11,
            entities: vec![EntityState {
                id: 1,
                position: [1.0, 1.0, 1.0],
                rotation: [0.1, 0.2, 0.3, 0.9],
                velocity: [3.0, 3.0, 3.0],
            }],
        };

        let diff = diff_snapshots(&old, &new);
        let result = apply_snapshot_diff(&old, &diff).unwrap();

        assert_eq!(result.entities[0].position, [1.0, 1.0, 1.0]);
        assert_eq!(result.entities[0].rotation, [0.1, 0.2, 0.3, 0.9]);
        assert_eq!(result.entities[0].velocity, [3.0, 3.0, 3.0]);
    }

    #[test]
    fn test_diff_removed_entity_propagates() {
        // Entity 2 exists in `old` but is despawned in `new`. Applying the
        // delta to `old` must reproduce `new` exactly — i.e. entity 2 must be
        // removed on the receiver, not linger forever.
        let old = Snapshot {
            tick: 10,
            entities: vec![make_entity(1, 0.0, 0.0, 0.0), make_entity(2, 5.0, 6.0, 7.0)],
        };
        let new = Snapshot {
            tick: 11,
            entities: vec![make_entity(1, 0.0, 0.0, 0.0)],
        };

        let diff = diff_snapshots(&old, &new);
        let result = apply_snapshot_diff(&old, &diff).expect("apply_snapshot_diff failed");

        assert_eq!(result.tick, 11);
        assert_eq!(result.entities.len(), 1, "removed entity must not linger");
        assert_eq!(result.entities[0].id, 1);
        assert!(
            !result.entities.iter().any(|e| e.id == 2),
            "despawned entity 2 should be gone after the delta"
        );
    }

    #[test]
    fn test_diff_removal_and_change_together() {
        // One entity moves, another is removed, in the same delta.
        let old = Snapshot {
            tick: 1,
            entities: vec![
                make_entity(1, 0.0, 0.0, 0.0),
                make_entity(2, 1.0, 1.0, 1.0),
                make_entity(3, 2.0, 2.0, 2.0),
            ],
        };
        let new = Snapshot {
            tick: 2,
            entities: vec![
                make_entity(1, 9.0, 9.0, 9.0), // moved
                make_entity(3, 2.0, 2.0, 2.0), // unchanged; entity 2 removed
            ],
        };

        let result = apply_snapshot_diff(&old, &diff_snapshots(&old, &new)).unwrap();
        // Compare as id->position maps (order is not guaranteed).
        use std::collections::HashMap;
        let got: HashMap<u32, [f32; 3]> =
            result.entities.iter().map(|e| (e.id, e.position)).collect();
        let want: HashMap<u32, [f32; 3]> =
            new.entities.iter().map(|e| (e.id, e.position)).collect();
        assert_eq!(got, want);
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
                rotation: [0.0, 0.0, 0.0, 1.0],
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

    #[test]
    fn deserialize_snapshot_rejects_huge_entity_count() {
        // Adversarial header claiming u32::MAX entities. Before the cap,
        // `count * ENTITY_SIZE` overflowed on 32-bit and `Vec::with_capacity`
        // could attempt a multi-GB allocation on 64-bit.
        let mut bytes = vec![0u8; 8];
        bytes[0..4].copy_from_slice(&0u32.to_le_bytes());        // tick
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());    // count = u32::MAX
        let err = deserialize_snapshot(&bytes).unwrap_err().to_string();
        assert!(err.contains("exceeds limit"), "unexpected error: {err}");
    }

    #[test]
    fn deserialize_snapshot_rejects_truncated_body() {
        // Header claims 2 entities but body is short.
        let mut bytes = vec![0u8; 8];
        bytes[0..4].copy_from_slice(&0u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes()); // claims 2 entities
        let err = deserialize_snapshot(&bytes).unwrap_err().to_string();
        assert!(err.contains("too short"), "unexpected error: {err}");
    }
}
