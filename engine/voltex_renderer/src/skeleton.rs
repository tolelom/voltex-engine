//! `Skeleton` — joint hierarchy + current pose → GPU [`BonePalette`].
//!
//! glTF parsing already produces [`GltfSkin`] (joint node list + inverse-bind
//! matrices) and [`GltfAnimation`] (per-node TRS keyframes), and
//! [`crate::animation`] can sample channels. The missing brain was turning that
//! into the per-joint *skinning matrices* a vertex shader multiplies. This
//! module fills it, standalone from the ECS (joints are not ECS entities — that
//! avoids polluting picking / scene serialization with hundreds of bone
//! entities).
//!
//! Pipeline: [`Skeleton::from_skin`] builds the joint tree → [`apply_animation`]
//! samples a pose at time `t` into each joint's local TRS →
//! [`compute_palette`] walks the hierarchy (`world = parent · local`) and
//! multiplies by the inverse-bind matrix to produce a [`BonePalette`] indexed by
//! joint (matching `SkinnedMeshVertex::joints`).
//!
//! [`apply_animation`]: Skeleton::apply_animation
//! [`compute_palette`]: Skeleton::compute_palette

use std::collections::HashMap;

use crate::animation::{build_trs, mat4_mul, sample_channel};
use crate::gltf::{AnimationPath, GltfAnimation, GltfNode, GltfSkin};
use crate::skinned_vertex::{BonePalette, MAX_BONES};

/// Local transform of one joint (TRS), the unit the animation system writes.
#[derive(Debug, Clone, Copy)]
pub struct JointPose {
    /// Local translation `[x, y, z]`.
    pub translation: [f32; 3],
    /// Local rotation quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Local scale `[x, y, z]`.
    pub scale: [f32; 3],
}

impl Default for JointPose {
    fn default() -> Self {
        JointPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// A skeleton: ordered joints (matching `SkinnedMeshVertex::joints` indices),
/// their parent links, inverse-bind matrices, and current local pose.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// glTF node index for each joint (joint-space index → node index).
    pub joint_nodes: Vec<usize>,
    /// Parent joint index per joint (`None` for roots), in joint space.
    pub parents: Vec<Option<usize>>,
    /// Inverse-bind matrix per joint (column-major `[[f32; 4]; 4]`).
    pub inverse_bind: Vec<[[f32; 4]; 4]>,
    /// Current local pose per joint (mutated by [`Self::apply_animation`]).
    pub local: Vec<JointPose>,
    /// node index → joint index, for resolving animation channel targets.
    node_to_joint: HashMap<usize, usize>,
}

/// 단위 행렬 (column-major 배열 — `build_trs`/`mat4_mul` 과 같은 표현).
const IDENTITY4: [[f32; 4]; 4] = voltex_math::Mat4::IDENTITY.cols;

impl Skeleton {
    /// Build a skeleton from a glTF skin + the scene node array.
    ///
    /// - Joint order follows `skin.joints` (so it matches the JOINTS_0 vertex
    ///   attribute indices).
    /// - Parent links are derived from `nodes[].children`: a joint's parent is
    ///   the nearest ancestor node that is itself a joint.
    /// - Missing inverse-bind matrices default to identity.
    /// - Initial local pose is taken from each joint node's TRS.
    pub fn from_skin(skin: &GltfSkin, nodes: &[GltfNode]) -> Self {
        let joint_nodes = skin.joints.clone();
        let n = joint_nodes.len();

        let mut node_to_joint = HashMap::with_capacity(n);
        for (j, &node) in joint_nodes.iter().enumerate() {
            node_to_joint.insert(node, j);
        }

        // child node → parent node, from the scene hierarchy.
        let mut child_to_parent_node: HashMap<usize, usize> = HashMap::new();
        for (parent_node, node) in nodes.iter().enumerate() {
            for &child in &node.children {
                child_to_parent_node.insert(child, parent_node);
            }
        }

        // Joint parent = nearest ancestor node that is also a joint.
        let mut parents = Vec::with_capacity(n);
        for &node in &joint_nodes {
            let mut parent_joint = None;
            let mut cur = child_to_parent_node.get(&node).copied();
            while let Some(p) = cur {
                if let Some(&pj) = node_to_joint.get(&p) {
                    parent_joint = Some(pj);
                    break;
                }
                cur = child_to_parent_node.get(&p).copied();
            }
            parents.push(parent_joint);
        }

        // Inverse-bind matrices (identity-pad if the skin omitted them).
        let mut inverse_bind = Vec::with_capacity(n);
        for j in 0..n {
            inverse_bind.push(
                skin.inverse_bind_matrices
                    .get(j)
                    .copied()
                    .unwrap_or(IDENTITY4),
            );
        }

        // Initial local pose from each joint node's TRS.
        let mut local = Vec::with_capacity(n);
        for &node in &joint_nodes {
            let node_data = &nodes[node];
            local.push(JointPose {
                translation: node_data.translation,
                rotation: node_data.rotation,
                scale: node_data.scale,
            });
        }

        Skeleton {
            joint_nodes,
            parents,
            inverse_bind,
            local,
            node_to_joint,
        }
    }

    /// Number of joints.
    pub fn joint_count(&self) -> usize {
        self.joint_nodes.len()
    }

    /// Overwrite each joint's local pose from `anim` sampled at `time` (seconds).
    /// Channels targeting non-joint nodes are ignored. Joints not animated keep
    /// their current pose.
    pub fn apply_animation(&mut self, anim: &GltfAnimation, time: f32) {
        for channel in &anim.channels {
            let Some(&j) = self.node_to_joint.get(&channel.target_node) else {
                continue;
            };
            let v = sample_channel(channel, time);
            match channel.target_path {
                AnimationPath::Translation if v.len() >= 3 => {
                    self.local[j].translation = [v[0], v[1], v[2]];
                }
                AnimationPath::Rotation if v.len() >= 4 => {
                    self.local[j].rotation = [v[0], v[1], v[2], v[3]];
                }
                AnimationPath::Scale if v.len() >= 3 => {
                    self.local[j].scale = [v[0], v[1], v[2]];
                }
                _ => {}
            }
        }
    }

    /// Compute the world matrix of joint `j` (column-major), memoizing into
    /// `world` (`None` = not yet computed). Iterative-safe via recursion over
    /// parent links, which form a tree (no cycles in valid glTF).
    fn world_matrix(&self, j: usize, world: &mut [Option<[[f32; 4]; 4]>]) -> [[f32; 4]; 4] {
        if let Some(m) = world[j] {
            return m;
        }
        let local = build_trs(
            &self.local[j].translation,
            &self.local[j].rotation,
            &self.local[j].scale,
        );
        let m = match self.parents[j] {
            Some(p) => {
                let parent_world = self.world_matrix(p, world);
                mat4_mul(&parent_world, &local)
            }
            None => local,
        };
        world[j] = Some(m);
        m
    }

    /// Produce the GPU bone palette for the current pose. Bone `j`'s matrix is
    /// `world_j · inverse_bind_j`; at the bind pose this is identity (no
    /// deformation). Joints beyond [`MAX_BONES`] are dropped.
    pub fn compute_palette(&self) -> BonePalette {
        let n = self.joint_count();
        let mut world: Vec<Option<[[f32; 4]; 4]>> = vec![None; n];
        let mut palette = BonePalette::new();
        for j in 0..n.min(MAX_BONES) {
            let world_j = self.world_matrix(j, &mut world);
            palette.set_bone(j, mat4_mul(&world_j, &self.inverse_bind[j]));
        }
        palette
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf::{GltfChannel, Interpolation};

    fn node(translation: [f32; 3], children: Vec<usize>) -> GltfNode {
        GltfNode {
            name: None,
            children,
            translation,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            mesh: None,
            skin: None,
        }
    }

    /// Transform a point by a column-major matrix (translation in col 3).
    fn xform(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
        let mut out = [0.0f32; 3];
        for (row, slot) in out.iter_mut().enumerate() {
            *slot = p[0] * m[0][row] + p[1] * m[1][row] + p[2] * m[2][row] + m[3][row];
        }
        out
    }

    fn translate(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [x, y, z, 1.0],
        ]
    }

    #[test]
    fn single_identity_joint_yields_identity_palette() {
        let nodes = vec![node([0.0, 0.0, 0.0], vec![])];
        let skin = GltfSkin {
            name: None,
            joints: vec![0],
            inverse_bind_matrices: vec![IDENTITY4],
            skeleton: None,
        };
        let skel = Skeleton::from_skin(&skin, &nodes);
        let palette = skel.compute_palette();
        for r in 0..4 {
            for c in 0..4 {
                let expect = if r == c { 1.0 } else { 0.0 };
                assert!((palette.matrices[0][c][r] - expect).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn parent_link_derived_from_node_children() {
        // node 0 (root) → child node 1.
        let nodes = vec![node([0.0, 0.0, 0.0], vec![1]), node([1.0, 0.0, 0.0], vec![])];
        let skin = GltfSkin {
            name: None,
            joints: vec![0, 1],
            inverse_bind_matrices: vec![IDENTITY4, translate(-1.0, 0.0, 0.0)],
            skeleton: None,
        };
        let skel = Skeleton::from_skin(&skin, &nodes);
        assert_eq!(skel.parents[0], None, "joint 0 is the root");
        assert_eq!(skel.parents[1], Some(0), "joint 1's parent is joint 0");
    }

    #[test]
    fn bind_pose_produces_identity_palettes() {
        // 2-joint chain: root at origin, child translated +1 X. Inverse-bind is
        // the inverse of each joint's bind world, so the rest pose deforms
        // nothing → every palette entry is identity.
        let nodes = vec![node([0.0, 0.0, 0.0], vec![1]), node([1.0, 0.0, 0.0], vec![])];
        let skin = GltfSkin {
            name: None,
            joints: vec![0, 1],
            inverse_bind_matrices: vec![IDENTITY4, translate(-1.0, 0.0, 0.0)],
            skeleton: None,
        };
        let skel = Skeleton::from_skin(&skin, &nodes);
        let palette = skel.compute_palette();
        for bone in 0..2 {
            for r in 0..4 {
                for c in 0..4 {
                    let expect = if r == c { 1.0 } else { 0.0 };
                    assert!(
                        (palette.matrices[bone][c][r] - expect).abs() < 1e-5,
                        "bone {bone} [{c}][{r}] = {} expected {expect}",
                        palette.matrices[bone][c][r]
                    );
                }
            }
        }
    }

    #[test]
    fn animating_root_rotation_moves_child_bound_vertex() {
        // Same chain. Rotate the root 90° about +Y. A vertex bound 100% to the
        // child at its bind position (1,0,0) should rotate to (0,0,-1).
        let nodes = vec![node([0.0, 0.0, 0.0], vec![1]), node([1.0, 0.0, 0.0], vec![])];
        let skin = GltfSkin {
            name: None,
            joints: vec![0, 1],
            inverse_bind_matrices: vec![IDENTITY4, translate(-1.0, 0.0, 0.0)],
            skeleton: None,
        };
        let mut skel = Skeleton::from_skin(&skin, &nodes);

        // Animation: rotate node 0 to 90° about Y at t=1 (single keyframe held).
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let anim = GltfAnimation {
            name: None,
            channels: vec![GltfChannel {
                target_node: 0,
                target_path: AnimationPath::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: vec![0.0, s, 0.0, s], // quat (0, sin45, 0, cos45)
            }],
        };
        skel.apply_animation(&anim, 0.0);
        let palette = skel.compute_palette();

        // Skin the child's bind vertex (1,0,0) with bone 1.
        let skinned = xform(&palette.matrices[1], [1.0, 0.0, 0.0]);
        assert!((skinned[0] - 0.0).abs() < 1e-4, "x={}", skinned[0]);
        assert!(skinned[1].abs() < 1e-4, "y={}", skinned[1]);
        assert!((skinned[2] + 1.0).abs() < 1e-4, "z={}", skinned[2]);
    }

    #[test]
    fn apply_animation_ignores_non_joint_channels() {
        let nodes = vec![node([0.0, 0.0, 0.0], vec![])];
        let skin = GltfSkin {
            name: None,
            joints: vec![0],
            inverse_bind_matrices: vec![IDENTITY4],
            skeleton: None,
        };
        let mut skel = Skeleton::from_skin(&skin, &nodes);
        let anim = GltfAnimation {
            name: None,
            channels: vec![GltfChannel {
                target_node: 99, // not a joint
                target_path: AnimationPath::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: vec![5.0, 5.0, 5.0],
            }],
        };
        // Must not panic / must leave the single joint at its rest pose.
        skel.apply_animation(&anim, 0.0);
        assert_eq!(skel.local[0].translation, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn missing_inverse_bind_defaults_to_identity() {
        let nodes = vec![node([0.0, 0.0, 0.0], vec![])];
        let skin = GltfSkin {
            name: None,
            joints: vec![0],
            inverse_bind_matrices: vec![], // omitted
            skeleton: None,
        };
        let skel = Skeleton::from_skin(&skin, &nodes);
        assert_eq!(skel.inverse_bind[0], IDENTITY4);
    }
}
