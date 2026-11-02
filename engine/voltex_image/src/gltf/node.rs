use super::accessor::read_accessor_mat4;
use super::component::read_accessor_floats;
use super::types::{
    parse_animation_path, parse_interpolation, GltfAnimation, GltfChannel, GltfNode, GltfSkin,
    Interpolation,
};
use crate::json_parser::JsonValue;

pub(super) fn parse_nodes(json: &JsonValue) -> Vec<GltfNode> {
    let empty_arr: Vec<JsonValue> = Vec::new();
    let nodes_arr = json
        .get("nodes")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_arr);
    let mut nodes = Vec::with_capacity(nodes_arr.len());
    for node_val in nodes_arr {
        let name = node_val
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let children = node_val
            .get("children")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u32().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        let translation = node_val
            .get("translation")
            .and_then(|v| v.as_array())
            .map(|arr| {
                [
                    arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                ]
            })
            .unwrap_or([0.0, 0.0, 0.0]);

        let rotation = node_val
            .get("rotation")
            .and_then(|v| v.as_array())
            .map(|arr| {
                [
                    arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                ]
            })
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);

        let scale = node_val
            .get("scale")
            .and_then(|v| v.as_array())
            .map(|arr| {
                [
                    arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                ]
            })
            .unwrap_or([1.0, 1.0, 1.0]);

        // A node may give its local transform as a column-major `matrix`
        // instead of TRS (glTF 2.0 §5.25); when present it wins.
        let (translation, rotation, scale) = match node_matrix(node_val) {
            Some(m) => decompose_matrix(&m),
            None => (translation, rotation, scale),
        };

        let mesh = node_val
            .get("mesh")
            .and_then(|v| v.as_u32())
            .map(|n| n as usize);
        let skin = node_val
            .get("skin")
            .and_then(|v| v.as_u32())
            .map(|n| n as usize);

        nodes.push(GltfNode {
            name,
            children,
            translation,
            rotation,
            scale,
            mesh,
            skin,
        });
    }
    nodes
}

pub(super) fn parse_skins(
    json: &JsonValue,
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
) -> Vec<GltfSkin> {
    let empty_arr: Vec<JsonValue> = Vec::new();
    let skins_arr = json
        .get("skins")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_arr);
    let mut skins = Vec::with_capacity(skins_arr.len());
    for skin_val in skins_arr {
        let name = skin_val
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let joints = skin_val
            .get("joints")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u32().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();

        let skeleton = skin_val
            .get("skeleton")
            .and_then(|v| v.as_u32())
            .map(|n| n as usize);

        let inverse_bind_matrices = skin_val
            .get("inverseBindMatrices")
            .and_then(|v| v.as_u32())
            .and_then(|idx| read_accessor_mat4(accessors, buffer_views, buffers, idx as usize).ok())
            .unwrap_or_default();

        skins.push(GltfSkin {
            name,
            joints,
            inverse_bind_matrices,
            skeleton,
        });
    }
    skins
}

pub(super) fn parse_animations(
    json: &JsonValue,
    accessors: &[JsonValue],
    buffer_views: &[JsonValue],
    buffers: &[Vec<u8>],
) -> Vec<GltfAnimation> {
    let empty_arr: Vec<JsonValue> = Vec::new();
    let anims_arr = json
        .get("animations")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_arr);
    let mut animations = Vec::with_capacity(anims_arr.len());
    for anim_val in anims_arr {
        let name = anim_val
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let samplers_arr = anim_val
            .get("samplers")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_arr);
        let channels_arr = anim_val
            .get("channels")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_arr);

        // Parse samplers: each has input, output, interpolation
        struct Sampler {
            times: Vec<f32>,
            values: Vec<f32>,
            interpolation: Interpolation,
        }
        let mut samplers = Vec::with_capacity(samplers_arr.len());
        for s in samplers_arr {
            let interp_str = s
                .get("interpolation")
                .and_then(|v| v.as_str())
                .unwrap_or("LINEAR");
            let interpolation = parse_interpolation(interp_str);

            let times = s
                .get("input")
                .and_then(|v| v.as_u32())
                .and_then(|idx| {
                    read_accessor_floats(accessors, buffer_views, buffers, idx as usize).ok()
                })
                .unwrap_or_default();

            let values = s
                .get("output")
                .and_then(|v| v.as_u32())
                .and_then(|idx| {
                    read_accessor_floats(accessors, buffer_views, buffers, idx as usize).ok()
                })
                .unwrap_or_default();

            samplers.push(Sampler {
                times,
                values,
                interpolation,
            });
        }

        // Parse channels
        let mut channels = Vec::with_capacity(channels_arr.len());
        for ch in channels_arr {
            let sampler_idx = ch.get("sampler").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
            let target = match ch.get("target") {
                Some(t) => t,
                None => continue,
            };
            let target_node = target.get("node").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
            let path_str = target
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("translation");
            let target_path = parse_animation_path(path_str);

            if let Some(sampler) = samplers.get(sampler_idx) {
                channels.push(GltfChannel {
                    target_node,
                    target_path,
                    interpolation: sampler.interpolation,
                    times: sampler.times.clone(),
                    values: sampler.values.clone(),
                });
            }
        }

        animations.push(GltfAnimation { name, channels });
    }
    animations
}


/// The node's `matrix` (16 numbers, column-major), if present and complete.
fn node_matrix(node_val: &JsonValue) -> Option<[f32; 16]> {
    let arr = node_val.get("matrix")?.as_array()?;
    if arr.len() != 16 {
        return None;
    }
    let mut m = [0.0f32; 16];
    for (slot, v) in m.iter_mut().zip(arr) {
        *slot = v.as_f64()? as f32;
    }
    Some(m)
}

/// Split a column-major affine matrix into translation, rotation quaternion
/// `[x, y, z, w]` and scale. A negative determinant (mirroring) is carried by
/// a negative X scale so the remaining basis is a proper rotation.
fn decompose_matrix(m: &[f32; 16]) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let col = |c: usize| [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]];
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (c0, c1, c2) = (col(0), col(1), col(2));
    let det = c0[0] * (c1[1] * c2[2] - c1[2] * c2[1]) - c1[0] * (c0[1] * c2[2] - c0[2] * c2[1])
        + c2[0] * (c0[1] * c1[2] - c0[2] * c1[1]);
    let mut scale = [len(c0), len(c1), len(c2)];
    if det < 0.0 {
        scale[0] = -scale[0];
    }
    let unit = |v: [f32; 3], s: f32| if s.abs() > 1e-12 { v.map(|x| x / s) } else { v };
    let (r0, r1, r2) = (unit(c0, scale[0]), unit(c1, scale[1]), unit(c2, scale[2]));
    // Rotation matrix element R[row][col] with columns r0, r1, r2.
    let r = |row: usize, c: usize| [r0, r1, r2][c][row];
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(r(2, 1) - r(1, 2)) / s, (r(0, 2) - r(2, 0)) / s, (r(1, 0) - r(0, 1)) / s, 0.25 * s]
    } else if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
        let s = (1.0 + r(0, 0) - r(1, 1) - r(2, 2)).sqrt() * 2.0;
        [0.25 * s, (r(0, 1) + r(1, 0)) / s, (r(0, 2) + r(2, 0)) / s, (r(2, 1) - r(1, 2)) / s]
    } else if r(1, 1) > r(2, 2) {
        let s = (1.0 + r(1, 1) - r(0, 0) - r(2, 2)).sqrt() * 2.0;
        [(r(0, 1) + r(1, 0)) / s, 0.25 * s, (r(1, 2) + r(2, 1)) / s, (r(0, 2) - r(2, 0)) / s]
    } else {
        let s = (1.0 + r(2, 2) - r(0, 0) - r(1, 1)).sqrt() * 2.0;
        [(r(0, 2) + r(2, 0)) / s, (r(1, 2) + r(2, 1)) / s, 0.25 * s, (r(1, 0) - r(0, 1)) / s]
    };
    ([m[12], m[13], m[14]], q, scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_parser::parse_json;

    fn first_node(json: &str) -> GltfNode {
        parse_nodes(&parse_json(json).unwrap()).remove(0)
    }

    fn approx(a: &[f32], b: &[f32]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn node_matrix_is_decomposed_into_trs() {
        // T(1,2,3) * Rz(90°) * S(2,2,2), column-major as glTF stores it.
        let node = first_node(
            r#"{"nodes":[{"matrix":[0,2,0,0, -2,0,0,0, 0,0,2,0, 1,2,3,1]}]}"#,
        );
        assert!(approx(&node.translation, &[1.0, 2.0, 3.0]), "{:?}", node.translation);
        assert!(approx(&node.scale, &[2.0, 2.0, 2.0]), "{:?}", node.scale);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let q = node.rotation;
        let sign = if q[3] < 0.0 { -1.0 } else { 1.0 }; // q and -q are the same rotation
        assert!(approx(&q.map(|c| c * sign), &[0.0, 0.0, h, h]), "{q:?}");
    }

    #[test]
    fn node_matrix_with_mirroring_keeps_a_negative_scale() {
        // S(-1, 1, 1): determinant < 0 must surface as a negative scale, not
        // be silently folded into an impossible rotation.
        let node = first_node(r#"{"nodes":[{"matrix":[-1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]}]}"#);
        assert!(approx(&node.scale, &[-1.0, 1.0, 1.0]), "{:?}", node.scale);
        assert!(approx(&node.rotation, &[0.0, 0.0, 0.0, 1.0]), "{:?}", node.rotation);
    }

    #[test]
    fn half_turns_about_each_axis_hit_every_quaternion_branch() {
        // trace = -1 for a 180° rotation, so each axis takes a different
        // non-trace branch of the matrix->quaternion conversion.
        for (matrix, axis) in [
            ("[1,0,0,0, 0,-1,0,0, 0,0,-1,0, 0,0,0,1]", 0), // about X
            ("[-1,0,0,0, 0,1,0,0, 0,0,-1,0, 0,0,0,1]", 1), // about Y
            ("[-1,0,0,0, 0,-1,0,0, 0,0,1,0, 0,0,0,1]", 2), // about Z
        ] {
            let node = first_node(&format!(r#"{{"nodes":[{{"matrix":{matrix}}}]}}"#));
            let mut want = [0.0f32; 4];
            want[axis] = 1.0;
            let q = node.rotation;
            let sign = if q[axis] < 0.0 { -1.0 } else { 1.0 };
            assert!(approx(&q.map(|c| c * sign), &want), "axis {axis}: {q:?}");
            assert!(approx(&node.scale, &[1.0, 1.0, 1.0]), "axis {axis}: {:?}", node.scale);
        }
    }

    #[test]
    fn node_without_matrix_keeps_trs_defaults() {
        let node = first_node(r#"{"nodes":[{"translation":[4,5,6]}]}"#);
        assert_eq!(node.translation, [4.0, 5.0, 6.0]);
        assert_eq!(node.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(node.scale, [1.0, 1.0, 1.0]);
    }
}
