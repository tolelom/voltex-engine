use voltex_ecs::Entity;
use voltex_math::{AABB, Ray};
use crate::ray::ray_vs_aabb;

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

    /// Query all overlapping pairs using recursive tree traversal (replaces N² brute force).
    pub fn query_pairs(&self) -> Vec<(Entity, Entity)> {
        let mut pairs = Vec::new();
        if self.nodes.is_empty() {
            return pairs;
        }
        let root = self.nodes.len() - 1;
        self.query_pairs_recursive(root, root, &mut pairs);
        pairs
    }

    fn query_pairs_recursive(&self, a: usize, b: usize, pairs: &mut Vec<(Entity, Entity)>) {
        let aabb_a = self.node_aabb(a);
        let aabb_b = self.node_aabb(b);

        if !aabb_a.intersects(aabb_b) {
            return;
        }

        match (&self.nodes[a], &self.nodes[b]) {
            (BvhNode::Leaf { entity: ea, aabb: aabb_a }, BvhNode::Leaf { entity: eb, aabb: aabb_b }) => {
                if a != b && ea.id <= eb.id && aabb_a.intersects(aabb_b) {
                    pairs.push((*ea, *eb));
                }
            }
            (BvhNode::Leaf { .. }, BvhNode::Internal { left, right, .. }) => {
                self.query_pairs_recursive(a, *left, pairs);
                self.query_pairs_recursive(a, *right, pairs);
            }
            (BvhNode::Internal { left, right, .. }, BvhNode::Leaf { .. }) => {
                self.query_pairs_recursive(*left, b, pairs);
                self.query_pairs_recursive(*right, b, pairs);
            }
            (BvhNode::Internal { left: la, right: ra, .. }, BvhNode::Internal { left: lb, right: rb, .. }) => {
                if a == b {
                    // Same node: check children against each other and themselves
                    let la = *la;
                    let ra = *ra;
                    self.query_pairs_recursive(la, la, pairs);
                    self.query_pairs_recursive(ra, ra, pairs);
                    self.query_pairs_recursive(la, ra, pairs);
                } else {
                    let la = *la;
                    let ra = *ra;
                    let lb = *lb;
                    let rb = *rb;
                    self.query_pairs_recursive(la, lb, pairs);
                    self.query_pairs_recursive(la, rb, pairs);
                    self.query_pairs_recursive(ra, lb, pairs);
                    self.query_pairs_recursive(ra, rb, pairs);
                }
            }
        }
    }

    fn node_aabb(&self, idx: usize) -> &AABB {
        match &self.nodes[idx] {
            BvhNode::Leaf { aabb, .. } => aabb,
            BvhNode::Internal { aabb, .. } => aabb,
        }
    }

    /// Query ray against BVH, returning all (Entity, t) hits sorted by t.
    pub fn query_ray(&self, ray: &Ray, max_t: f32) -> Vec<(Entity, f32)> {
        let mut hits = Vec::new();
        if self.nodes.is_empty() {
            return hits;
        }
        let root = self.nodes.len() - 1;
        self.query_ray_recursive(root, ray, max_t, &mut hits);
        hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        hits
    }

    fn query_ray_recursive(&self, idx: usize, ray: &Ray, max_t: f32, hits: &mut Vec<(Entity, f32)>) {
        let aabb = self.node_aabb(idx);
        match ray_vs_aabb(ray, aabb) {
            Some(t) if t <= max_t => {}
            _ => return,
        }

        match &self.nodes[idx] {
            BvhNode::Leaf { entity, aabb } => {
                if let Some(t) = ray_vs_aabb(ray, aabb) {
                    if t <= max_t {
                        hits.push((*entity, t));
                    }
                }
            }
            BvhNode::Internal { left, right, .. } => {
                self.query_ray_recursive(*left, ray, max_t, hits);
                self.query_ray_recursive(*right, ray, max_t, hits);
            }
        }
    }

    /// Refit the BVH: update leaf AABBs and propagate changes to parents.
    /// `updated` maps entity → new AABB. Leaves not in map are unchanged.
    pub fn refit(&mut self, updated: &[(Entity, AABB)]) {
        if self.nodes.is_empty() {
            return;
        }
        let root = self.nodes.len() - 1;
        self.refit_recursive(root, updated);
    }

    fn refit_recursive(&mut self, idx: usize, updated: &[(Entity, AABB)]) -> AABB {
        match self.nodes[idx] {
            BvhNode::Leaf { entity, ref mut aabb } => {
                if let Some((_, new_aabb)) = updated.iter().find(|(e, _)| *e == entity) {
                    *aabb = *new_aabb;
                }
                *aabb
            }
            BvhNode::Internal { left, right, aabb: _ } => {
                let left_aabb = self.refit_recursive(left, updated);
                let right_aabb = self.refit_recursive(right, updated);
                let new_aabb = left_aabb.merged(&right_aabb);
                // Update the internal node's AABB
                if let BvhNode::Internal { ref mut aabb, .. } = self.nodes[idx] {
                    *aabb = new_aabb;
                }
                new_aabb
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

    // --- query_pairs: verify recursive gives same results as brute force ---

    #[test]
    fn test_query_pairs_matches_brute_force() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))),
            (make_entity(1), AABB::new(Vec3::ONE, Vec3::new(3.0, 3.0, 3.0))),
            (make_entity(2), AABB::new(Vec3::new(2.5, 2.5, 2.5), Vec3::new(4.0, 4.0, 4.0))),
            (make_entity(3), AABB::new(Vec3::new(10.0, 10.0, 10.0), Vec3::new(11.0, 11.0, 11.0))),
            (make_entity(4), AABB::new(Vec3::new(10.5, 10.5, 10.5), Vec3::new(12.0, 12.0, 12.0))),
        ];

        let tree = BvhTree::build(&entries);
        let mut pairs = tree.query_pairs();
        pairs.sort_by_key(|(a, b)| (a.id.min(b.id), a.id.max(b.id)));

        // Brute force
        let mut brute: Vec<(Entity, Entity)> = Vec::new();
        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                if entries[i].1.intersects(&entries[j].1) {
                    let a = entries[i].0;
                    let b = entries[j].0;
                    brute.push(if a.id <= b.id { (a, b) } else { (b, a) });
                }
            }
        }
        brute.sort_by_key(|(a, b)| (a.id, b.id));

        assert_eq!(pairs.len(), brute.len(), "pair count mismatch: tree={}, brute={}", pairs.len(), brute.len());
        for (tree_pair, brute_pair) in pairs.iter().zip(brute.iter()) {
            let t = (tree_pair.0.id.min(tree_pair.1.id), tree_pair.0.id.max(tree_pair.1.id));
            let b = (brute_pair.0.id, brute_pair.1.id);
            assert_eq!(t, b);
        }
    }

    // --- query_ray tests ---

    #[test]
    fn test_query_ray_basic() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::new(4.0, -1.0, -1.0), Vec3::new(6.0, 1.0, 1.0))),
            (make_entity(1), AABB::new(Vec3::new(10.0, 10.0, 10.0), Vec3::new(11.0, 11.0, 11.0))),
        ];
        let tree = BvhTree::build(&entries);
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = tree.query_ray(&ray, 100.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.id, 0);
    }

    #[test]
    fn test_query_ray_multiple() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::new(2.0, -1.0, -1.0), Vec3::new(4.0, 1.0, 1.0))),
            (make_entity(1), AABB::new(Vec3::new(6.0, -1.0, -1.0), Vec3::new(8.0, 1.0, 1.0))),
        ];
        let tree = BvhTree::build(&entries);
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = tree.query_ray(&ray, 100.0);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].1 < hits[1].1, "should be sorted by distance");
    }

    #[test]
    fn test_query_ray_miss() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::new(4.0, 4.0, 4.0), Vec3::new(6.0, 6.0, 6.0))),
        ];
        let tree = BvhTree::build(&entries);
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = tree.query_ray(&ray, 100.0);
        assert!(hits.is_empty());
    }

    // --- refit tests ---

    #[test]
    fn test_refit_updates_leaf() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::ONE)),
            (make_entity(1), AABB::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0))),
        ];
        let mut tree = BvhTree::build(&entries);

        // Initially separated
        assert!(tree.query_pairs().is_empty());

        // Move entity 1 to overlap entity 0
        tree.refit(&[(make_entity(1), AABB::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5)))]);

        let pairs = tree.query_pairs();
        assert_eq!(pairs.len(), 1, "after refit, overlapping entities should be found");
    }

    #[test]
    fn test_refit_separates_entities() {
        let entries = vec![
            (make_entity(0), AABB::new(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))),
            (make_entity(1), AABB::new(Vec3::ONE, Vec3::new(3.0, 3.0, 3.0))),
        ];
        let mut tree = BvhTree::build(&entries);

        // Initially overlapping
        assert_eq!(tree.query_pairs().len(), 1);

        // Move entity 1 far away
        tree.refit(&[(make_entity(1), AABB::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0)))]);

        assert!(tree.query_pairs().is_empty(), "after refit, separated entities should not overlap");
    }
}
