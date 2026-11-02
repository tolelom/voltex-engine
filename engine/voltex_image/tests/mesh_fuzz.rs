//! Mutation fuzz for the mesh loaders: a hand-built glTF (embedded base64 and
//! GLB container, with indices, normals, UVs, a node hierarchy, a skin and an
//! animation) and the repo's `cube.obj` are corrupted thousands of times.
//! Model files are untrusted assets: loaders must return `Err` (glTF) or a
//! consistent mesh, never panic.

use std::panic::{catch_unwind, AssertUnwindSafe};
use voltex_image::obj::parse_obj;
use voltex_image::parse_gltf;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// One triangle: positions(36) normals(36) uvs(24) indices u16(6, pad 2)
/// joints u8x4(12) weights(48) ibm(64) anim times(8) anim rotations(32).
fn buffer() -> Vec<u8> {
    let mut b = f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    b.extend(f32s(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]));
    b.extend(f32s(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]));
    b.extend([0u8, 0, 1, 0, 2, 0, 0, 0]);
    b.extend([0u8; 12]);
    b.extend(f32s(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]));
    b.extend(f32s(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]));
    b.extend(f32s(&[0.0, 1.0]));
    let h = std::f32::consts::FRAC_1_SQRT_2;
    b.extend(f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, h, 0.0, h]));
    b
}

fn gltf_json(buffer_uri: Option<String>, len: usize) -> String {
    let uri = buffer_uri.map(|u| format!(r#","uri":"{u}""#)).unwrap_or_default();
    format!(
        r#"{{"asset":{{"version":"2.0"}},
"buffers":[{{"byteLength":{len}{uri}}}],
"bufferViews":[
 {{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":36}},
 {{"buffer":0,"byteOffset":72,"byteLength":24}},{{"buffer":0,"byteOffset":96,"byteLength":6}},
 {{"buffer":0,"byteOffset":104,"byteLength":12}},{{"buffer":0,"byteOffset":116,"byteLength":48}},
 {{"buffer":0,"byteOffset":164,"byteLength":64}},{{"buffer":0,"byteOffset":228,"byteLength":8}},
 {{"buffer":0,"byteOffset":236,"byteLength":32}}],
"accessors":[
 {{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},
 {{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"}},
 {{"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}},
 {{"bufferView":3,"componentType":5123,"count":3,"type":"SCALAR"}},
 {{"bufferView":4,"componentType":5121,"count":3,"type":"VEC4"}},
 {{"bufferView":5,"componentType":5126,"count":3,"type":"VEC4"}},
 {{"bufferView":6,"componentType":5126,"count":1,"type":"MAT4"}},
 {{"bufferView":7,"componentType":5126,"count":2,"type":"SCALAR"}},
 {{"bufferView":8,"componentType":5126,"count":2,"type":"VEC4"}}],
"materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[1,0.5,0.2,1],"metallicFactor":0.3,"roughnessFactor":0.7}}}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2,"JOINTS_0":4,"WEIGHTS_0":5}},"indices":3,"material":0}}]}}],
"nodes":[{{"name":"root","children":[1],"translation":[1,2,3]}},{{"name":"mesh","mesh":0,"skin":0,"rotation":[0,0,0,1],"scale":[1,1,1]}}],
"skins":[{{"joints":[0],"inverseBindMatrices":6}}],
"animations":[{{"samplers":[{{"input":7,"output":8,"interpolation":"LINEAR"}}],"channels":[{{"sampler":0,"target":{{"node":0,"path":"rotation"}}}}]}}],
"scenes":[{{"nodes":[0]}}],"scene":0}}"#
    )
}

fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut j = json.as_bytes().to_vec();
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    let mut b = bin.to_vec();
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + b.len();
    let mut out = Vec::new();
    out.extend(0x4654_6C67u32.to_le_bytes());
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((j.len() as u32).to_le_bytes());
    out.extend(0x4E4F_534Au32.to_le_bytes());
    out.extend(j);
    out.extend((b.len() as u32).to_le_bytes());
    out.extend(0x004E_4942u32.to_le_bytes());
    out.extend(b);
    out
}

fn mutate(rng: &mut XorShift, src: &[u8], digits_only: bool) -> Vec<u8> {
    let mut b = src.to_vec();
    for _ in 0..1 + rng.next() % 4 {
        let i = rng.next() as usize % b.len();
        b[i] = match rng.next() % 3 {
            // Swap one digit for another: keeps JSON/OBJ syntax valid, so
            // the mutation reaches indices, counts and offsets.
            0 if b[i].is_ascii_digit() => b"0123456789"[rng.next() as usize % 10],
            _ if digits_only => b[i],
            1 => b"0123456789-.e,:[]{}\" /\n"[rng.next() as usize % 23],
            _ => rng.next() as u8,
        };
    }
    if rng.next().is_multiple_of(5) {
        b.truncate(rng.next() as usize % b.len());
    }
    b
}

#[test]
fn gltf_and_glb_loaders_never_panic_on_corrupted_files() {
    let bin = buffer();
    let embedded = gltf_json(Some(format!("data:application/octet-stream;base64,{}", base64(&bin))), bin.len());
    let container = glb(&gltf_json(None, bin.len()), &bin);
    parse_gltf(embedded.as_bytes()).expect("clean embedded glTF parses");
    parse_gltf(&container).expect("clean GLB parses");

    let mut rng = XorShift(0x61F7_0001);
    for i in 0..6000u32 {
        let (name, src) = if i.is_multiple_of(2) { ("gltf", embedded.as_bytes()) } else { ("glb", container.as_slice()) };
        let input = mutate(&mut rng, src, i.is_multiple_of(3));
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(data) = parse_gltf(&input) {
                for mesh in &data.meshes {
                    let n = mesh.vertices.len() as u32;
                    assert!(mesh.indices.iter().all(|&ix| ix < n), "index past vertex count");
                }
            }
        }));
        assert!(outcome.is_ok(), "{name} mutation {i} panicked parse_gltf");
    }
}

#[test]
fn obj_loader_never_panics_on_corrupted_files() {
    let cube = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/cube.obj")).unwrap();
    let mut rng = XorShift(0x0B1E_C7ED);
    for i in 0..6000u32 {
        let input = mutate(&mut rng, &cube, i.is_multiple_of(2));
        let text = String::from_utf8_lossy(&input).into_owned();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let data = parse_obj(&text);
            let n = data.vertices.len() as u32;
            assert!(data.indices.iter().all(|&ix| ix < n), "index past vertex count");
        }));
        assert!(outcome.is_ok(), "obj mutation {i} panicked parse_obj");
    }
}
