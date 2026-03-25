use voltex_ecs::Entity;
use voltex_math::AABB;

#[derive(Debug)]
enum BvhNode {
    Leaf { entity: Entity, aabb: AABB },
    Internal { aabb: AABB, left: usize, right: usize },
}

#[derive(Debug)]
pub struct BvhTree {
    nodes: Vec<BvhNode>,
}

impl BvhTree {
    pub fn build(entries: &[(Entity, AABB)]) -> Self {
        let mut tree = BvhTree { nodes: Vec::new() };
        if !entries.is_empty() {
            let mut sorted: Vec<(Entity, AABB)> = entries.to_vec();
            tree.build_recursive(&mut sorted);
        }
        tree
    }

    fn build_recursive(&mut self, entries: &mut [(Entity, AABB)]) -> usize {
        if entries.len() == 1 {
            let idx = self.nodes.len();
            self.nodes.push(BvhNode::Leaf {
                entity: entries[0].0,
                aabb: entries[0].1,
            });
            return idx;
        }

        // Compute bounding AABB
        let mut combined = entries[0].1;
        for e in entries.iter().skip(1) {
            combined = combined.merged(&e.1);
        }

        // Find longest axis
        let extent = combined.max - combined.min;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };

        // Sort by axis center
        entries.sort_by(|a, b| {
            let ca = a.1.center();
            let cb = b.1.center();
            let va = match axis { 0 => ca.x, 1 => ca.y, _ => ca.z };
            let vb = match axis { 0 => cb.x, 1 => cb.y, _ => cb.z };
            va.partial_cmp(&vb).unwrap()
        });

        let mid = entries.len() / 2;
        let (left_entries, right_entries) = entries.split_at_mut(mid);

        let left = self.build_recursive(left_entries);
        let right = self.build_recursive(right_entries);

        let idx = self.nodes.len();
        self.nodes.push(BvhNode::Internal {
            aabb: combined,
            left,
            right,
        });
        idx
    }

    pub fn query_pairs(&self) -> Vec<(Entity, Entity)> {
        let mut pairs = Vec::new();
        if self.nodes.is_empty() {
            return pairs;
        }
        let root = self.nodes.len() - 1;
        let mut leaves = Vec::new();
        self.collect_leaves(root, &mut leaves);
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                let (ea, aabb_a) = leaves[i];
                let (eb, aabb_b) = leaves[j];
                if aabb_a.intersects(&aabb_b) {
                    pairs.push((ea, eb));
                }
            }
        }
        pairs
    }

    fn collect_leaves(&self, node_idx: usize, out: &mut Vec<(Entity, AABB)>) {
        match &self.nodes[node_idx] {
            BvhNode::Leaf { entity, aabb } => {
                out.push((*entity, *aabb));
            }
            BvhNode::Internal { left, right, .. } => {
                self.collect_leaves(*left, out);
                self.collect_leaves(*right, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltex_math::Vec3;

    fn make_entity(id: u32) -> Entity {
        Entity { id, generation: 0 }
    }

    #[test]
    fn test_build_empty() {
        let tree = BvhTree::build(&[]);
        assert!(tree.query_pairs().is_empty());
    }

    #[test]
    fn test_build_single() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::ONE)),
        ];
        let tree = BvhTree::build(&entries);
        assert!(tree.query_pairs().is_empty());
    }

    #[test]
    fn test_overlapping_pair() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))),
            (make_entity(1), AABB::new(Vec3::ONE, Vec3::new(3.0, 3.0, 3.0))),
        ];
        let tree = BvhTree::build(&entries);
        let pairs = tree.query_pairs();
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_separated_pair() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::ONE)),
            (make_entity(1), AABB::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0))),
        ];
        let tree = BvhTree::build(&entries);
        assert!(tree.query_pairs().is_empty());
    }

    #[test]
    fn test_multiple_entities() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))),
            (make_entity(1), AABB::new(Vec3::ONE, Vec3::new(3.0, 3.0, 3.0))),
            (make_entity(2), AABB::new(Vec3::new(10.0, 10.0, 10.0), Vec3::new(11.0, 11.0, 11.0))),
        ];
        let tree = BvhTree::build(&entries);
        let pairs = tree.query_pairs();
        assert_eq!(pairs.len(), 1);
        let (a, b) = pairs[0];
        assert!((a.id == 0 && b.id == 1) || (a.id == 1 && b.id == 0));
    }
}
