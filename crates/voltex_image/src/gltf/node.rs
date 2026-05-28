use super::accessor::{read_accessor_floats, read_accessor_mat4};
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
