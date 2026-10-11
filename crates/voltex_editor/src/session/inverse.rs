//! Undo/redo inverses and handle remapping for [`super::EditorSession`].
//! Split from `mod.rs` (600-line rule).

use crate::command::EditorCommand;
use voltex_ecs::Entity;
use voltex_math::{Quat, Vec3};

/// Spawn redo가 배정받은 새 엔티티로, redo 스택에 남은 항목들의 옛 핸들
/// 참조를 치환한다 (forward + inverse 양쪽). 없으면 남은 체인이
/// `EntityNotAlive`로 조용히 끊긴다.
pub(super) fn remap_entity_refs(
    stack: &mut [(EditorCommand, Inverse)],
    old: Entity,
    new: Entity,
) {
    fn fix(e: &mut Entity, old: Entity, new: Entity) {
        if *e == old {
            *e = new;
        }
    }
    for (forward, inverse) in stack.iter_mut() {
        match forward {
            EditorCommand::Despawn { entity }
            | EditorCommand::SetTransform { entity, .. }
            | EditorCommand::SetTag { entity, .. }
            | EditorCommand::AddComponent { entity, .. }
            | EditorCommand::RemoveComponent { entity, .. } => fix(entity, old, new),
            EditorCommand::AddChild { parent, child }
            | EditorCommand::RemoveChild { parent, child } => {
                fix(parent, old, new);
                fix(child, old, new);
            }
            EditorCommand::Spawn
            | EditorCommand::SaveScene { .. }
            | EditorCommand::LoadScene { .. } => {}
        }
        match inverse {
            Inverse::DespawnSpawned(entity)
            | Inverse::SetTransform { entity, .. }
            | Inverse::SetTag { entity, .. }
            | Inverse::RemoveComponentByName { entity, .. }
            | Inverse::AddComponentByBytes { entity, .. } => fix(entity, old, new),
            Inverse::RestoreEntity { entity, parent, children, .. } => {
                fix(entity, old, new);
                if let Some(p) = parent {
                    fix(p, old, new);
                }
                for c in children.iter_mut() {
                    fix(c, old, new);
                }
            }
            Inverse::AddChild { parent, child } => {
                fix(parent, old, new);
                fix(child, old, new);
            }
            Inverse::RemoveChild { parent, child, prev_parent } => {
                fix(parent, old, new);
                fix(child, old, new);
                if let Some(p) = prev_parent {
                    fix(p, old, new);
                }
            }
        }
    }
}

/// The inverse of a dispatched command. The undo stack stores one of these per
/// undoable command. Only SaveScene/LoadScene clear the history (no tractable
/// inverse). Despawn became undoable in E3 via the `RestoreEntity` variant.
#[derive(Debug, Clone)]
pub(super) enum Inverse {
    /// Undo Spawn: despawn the entity that was just created.
    DespawnSpawned(Entity),
    /// Undo SetTransform: write back the previous Transform values.
    SetTransform {
        entity: Entity,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
    },
    /// Undo SetTag: restore the previous Tag (or remove if there was none).
    SetTag {
        entity: Entity,
        prev: Option<String>,
    },
    /// Undo of AddChild: detach from `parent` and, if the child had a
    /// different parent before (re-parenting), put it back there.
    RemoveChild { parent: Entity, child: Entity, prev_parent: Option<Entity> },
    /// Undo RemoveChild: re-attach the child to the parent.
    AddChild { parent: Entity, child: Entity },
    /// Undo AddComponent: remove the just-added component by name (E3).
    RemoveComponentByName { entity: Entity, name: String },
    /// Undo RemoveComponent: re-add with prior bytes (E3). `data = None`
    /// when the component was absent before — undo leaves it absent.
    AddComponentByBytes { entity: Entity, name: String, data: Option<Vec<u8>> },
    /// Undo Despawn: restore the entity slot at its original generation and
    /// re-attach every registered component captured pre-despawn (E3).
    RestoreEntity {
        entity: Entity,
        components: Vec<(String, Vec<u8>)>,
        /// Hierarchy links at despawn time — Parent/Children are not
        /// registered components, so they are restored separately.
        parent: Option<Entity>,
        children: Vec<Entity>,
    },
}
