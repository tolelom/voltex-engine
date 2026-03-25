# glTF/GLB Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Self-contained glTF 2.0 / GLB parser that returns mesh data compatible with existing `MeshVertex` and `ObjData` patterns.

**Architecture:** GLB header parser → mini JSON parser → accessor/bufferView extraction → vertex assembly with existing `compute_tangents`. Single file `gltf.rs` plus `json_parser.rs` for the JSON subset parser.

**Tech Stack:** Pure Rust, no external dependencies. Reuses `MeshVertex` from `vertex.rs` and `compute_tangents` from `obj.rs`.

---

### Task 1: Mini JSON Parser

**Files:**
- Create: `crates/voltex_renderer/src/json_parser.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: Write tests for JSON parsing**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() {
        assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn test_parse_number() {
        match parse_json("42").unwrap() {
            JsonValue::Number(n) => assert!((n - 42.0).abs() < 1e-10),
            other => panic!("Expected Number, got {:?}", other),
        }
        match parse_json("-3.14").unwrap() {
            JsonValue::Number(n) => assert!((n - (-3.14)).abs() < 1e-10),
            other => panic!("Expected Number, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(parse_json("\"hello\"").unwrap(), JsonValue::String("hello".into()));
    }

    #[test]
    fn test_parse_string_escapes() {
        assert_eq!(
            parse_json(r#""hello\nworld""#).unwrap(),
            JsonValue::String("hello\nworld".into())
        );
    }

    #[test]
    fn test_parse_array() {
        let val = parse_json("[1, 2, 3]").unwrap();
        match val {
            JsonValue::Array(arr) => assert_eq!(arr.len(), 3),
            other => panic!("Expected Array, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_object() {
        let val = parse_json(r#"{"name": "test", "value": 42}"#).unwrap();
        match val {
            JsonValue::Object(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map[0].0, "name");
            }
            other => panic!("Expected Object, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_nested() {
        let json = r#"{"meshes": [{"name": "Cube", "primitives": [{"attributes": {"POSITION": 0}}]}]}"#;
        let val = parse_json(json).unwrap();
        assert!(matches!(val, JsonValue::Object(_)));
    }

    #[test]
    fn test_parse_empty_array() {
        assert_eq!(parse_json("[]").unwrap(), JsonValue::Array(vec![]));
    }

    #[test]
    fn test_parse_empty_object() {
        assert_eq!(parse_json("{}").unwrap(), JsonValue::Object(vec![]));
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --package voltex_renderer -- json_parser::tests -v`
Expected: FAIL — module not found

- [ ] **Step 3: Implement mini JSON parser**

```rust
// crates/voltex_renderer/src/json_parser.rs

/// Minimal JSON parser for glTF. No external dependencies.

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),  // preserve order
}

impl JsonValue {
    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self { JsonValue::Object(v) => Some(v), _ => None }
    }
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self { JsonValue::Array(v) => Some(v), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { JsonValue::String(s) => Some(s), _ => None }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self { JsonValue::Number(n) => Some(*n), _ => None }
    }
    pub fn as_u32(&self) -> Option<u32> {
        self.as_f64().map(|n| n as u32)
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self { JsonValue::Bool(b) => Some(*b), _ => None }
    }
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.as_object()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn index(&self, i: usize) -> Option<&JsonValue> {
        self.as_array()?.get(i)
    }
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(input);
    let val = parser.parse_value()?;
    Ok(val)
}

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input: input.as_bytes(), pos: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Result<u8, String> {
        if self.pos >= self.input.len() {
            return Err("Unexpected end of JSON".into());
        }
        let b = self.input[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn expect(&mut self, ch: u8) -> Result<(), String> {
        let b = self.advance()?;
        if b != ch {
            return Err(format!("Expected '{}', got '{}'", ch as char, b as char));
        }
        Ok(())
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') => self.parse_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal("null", JsonValue::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(ch) => Err(format!("Unexpected character: '{}'", ch as char)),
            None => Err("Unexpected end of JSON".into()),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            let b = self.advance()?;
            match b {
                b'"' => return Ok(s),
                b'\\' => {
                    let esc = self.advance()?;
                    match esc {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{08}'),
                        b'f' => s.push('\u{0C}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                hex.push(self.advance()? as char);
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| format!("Invalid unicode escape: {}", hex))?;
                            if let Some(ch) = char::from_u32(code) {
                                s.push(ch);
                            }
                        }
                        _ => return Err(format!("Invalid escape: \\{}", esc as char)),
                    }
                }
                _ => s.push(b as char),
            }
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') { self.pos += 1; }
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.input.len() && self.input[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < self.input.len() && (self.input[self.pos] == b'e' || self.input[self.pos] == b'E') {
            self.pos += 1;
            if self.pos < self.input.len() && (self.input[self.pos] == b'+' || self.input[self.pos] == b'-') {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| "Invalid UTF-8 in number")?;
        let n: f64 = s.parse().map_err(|_| format!("Invalid number: {}", s))?;
        Ok(JsonValue::Number(n))
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        self.skip_whitespace();
        let mut pairs = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            let val = self.parse_value()?;
            pairs.push((key, val));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => { self.pos += 1; }
                Some(b'}') => { self.pos += 1; return Ok(JsonValue::Object(pairs)); }
                _ => return Err("Expected ',' or '}' in object".into()),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let val = self.parse_value()?;
            items.push(val);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => { self.pos += 1; }
                Some(b']') => { self.pos += 1; return Ok(JsonValue::Array(items)); }
                _ => return Err("Expected ',' or ']' in array".into()),
            }
        }
    }

    fn parse_literal(&mut self, expected: &str, value: JsonValue) -> Result<JsonValue, String> {
        for &b in expected.as_bytes() {
            let actual = self.advance()?;
            if actual != b {
                return Err(format!("Expected '{}', got '{}'", b as char, actual as char));
            }
        }
        Ok(value)
    }
}
```

Register in lib.rs: `pub mod json_parser;`

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_renderer -- json_parser::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/json_parser.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add self-contained JSON parser for glTF support"
```

---

### Task 2: GLB Header + Base64 Decoder

**Files:**
- Create: `crates/voltex_renderer/src/gltf.rs`
- Modify: `crates/voltex_renderer/src/lib.rs`

- [ ] **Step 1: Write tests for GLB header parsing and base64**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glb_header_magic() {
        // Invalid magic
        let data = [0u8; 12];
        assert!(parse_gltf(&data).is_err());
    }

    #[test]
    fn test_glb_header_version() {
        // Valid magic but wrong version
        let mut data = Vec::new();
        data.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
        data.extend_from_slice(&1u32.to_le_bytes()); // version 1 (we need 2)
        data.extend_from_slice(&12u32.to_le_bytes()); // length
        assert!(parse_gltf(&data).is_err());
    }

    #[test]
    fn test_base64_decode() {
        let encoded = "SGVsbG8="; // "Hello"
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_base64_decode_no_padding() {
        let encoded = "SGVsbG8"; // "Hello" without padding
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, b"Hello");
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

- [ ] **Step 3: Implement GLB parser skeleton and base64 decoder**

```rust
// crates/voltex_renderer/src/gltf.rs

use crate::json_parser::{self, JsonValue};
use crate::vertex::MeshVertex;
use crate::obj::compute_tangents;

pub struct GltfData {
    pub meshes: Vec<GltfMesh>,
}

pub struct GltfMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub name: Option<String>,
    pub material: Option<GltfMaterial>,
}

pub struct GltfMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

const GLB_MAGIC: u32 = 0x46546C67;
const GLB_VERSION: u32 = 2;
const CHUNK_JSON: u32 = 0x4E4F534A;
const CHUNK_BIN: u32 = 0x004E4942;

pub fn parse_gltf(data: &[u8]) -> Result<GltfData, String> {
    if data.len() < 4 {
        return Err("Data too short".into());
    }

    // Detect format: GLB (binary) or JSON
    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic == GLB_MAGIC {
        parse_glb(data)
    } else if data[0] == b'{' {
        parse_gltf_json(data)
    } else {
        Err("Unknown glTF format: not GLB or JSON".into())
    }
}

fn parse_glb(data: &[u8]) -> Result<GltfData, String> {
    if data.len() < 12 {
        return Err("GLB header too short".into());
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != GLB_VERSION {
        return Err(format!("Unsupported GLB version: {} (expected 2)", version));
    }
    let _total_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

    // Parse chunks
    let mut pos = 12;
    let mut json_str = String::new();
    let mut bin_data: Vec<u8> = Vec::new();

    while pos + 8 <= data.len() {
        let chunk_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        let chunk_type = u32::from_le_bytes([data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
        pos += 8;

        if pos + chunk_len > data.len() {
            return Err("Chunk extends past data".into());
        }

        match chunk_type {
            CHUNK_JSON => {
                json_str = std::str::from_utf8(&data[pos..pos + chunk_len])
                    .map_err(|_| "Invalid UTF-8 in JSON chunk")?
                    .to_string();
            }
            CHUNK_BIN => {
                bin_data = data[pos..pos + chunk_len].to_vec();
            }
            _ => {} // skip unknown chunks
        }
        pos += chunk_len;
        // Chunks are 4-byte aligned
        pos = (pos + 3) & !3;
    }

    if json_str.is_empty() {
        return Err("No JSON chunk found in GLB".into());
    }

    let json = json_parser::parse_json(&json_str)?;
    let buffers = vec![bin_data]; // GLB has one implicit binary buffer
    extract_meshes(&json, &buffers)
}

fn parse_gltf_json(data: &[u8]) -> Result<GltfData, String> {
    let json_str = std::str::from_utf8(data).map_err(|_| "Invalid UTF-8")?;
    let json = json_parser::parse_json(json_str)?;

    // Resolve buffers (embedded base64 URIs)
    let mut buffers = Vec::new();
    if let Some(bufs) = json.get("buffers").and_then(|v| v.as_array()) {
        for buf in bufs {
            if let Some(uri) = buf.get("uri").and_then(|v| v.as_str()) {
                if let Some(b64) = uri.strip_prefix("data:application/octet-stream;base64,") {
                    buffers.push(decode_base64(b64)?);
                } else if let Some(b64) = uri.strip_prefix("data:application/gltf-buffer;base64,") {
                    buffers.push(decode_base64(b64)?);
                } else {
                    return Err(format!("External buffer URIs not supported: {}", uri));
                }
            } else {
                buffers.push(Vec::new());
            }
        }
    }

    extract_meshes(&json, &buffers)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let table = |c: u8| -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0), // padding
            _ => Err(format!("Invalid base64 character: {}", c as char)),
        }
    };

    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'\n' && b != b'\r' && b != b' ').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        let b0 = table(chunk[0])?;
        let b1 = if chunk.len() > 1 { table(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { table(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { table(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 && chunk[2] != b'=' {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 && chunk[3] != b'=' {
            out.push((b2 << 6) | b3);
        }
    }
    Ok(out)
}
```

Register in lib.rs:
```rust
pub mod gltf;
pub use gltf::{parse_gltf, GltfData, GltfMesh, GltfMaterial};
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_renderer -- gltf::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/gltf.rs crates/voltex_renderer/src/lib.rs
git commit -m "feat(renderer): add GLB header parser and base64 decoder for glTF"
```

---

### Task 3: Accessor/BufferView Data Extraction

**Files:**
- Modify: `crates/voltex_renderer/src/gltf.rs`

- [ ] **Step 1: Write tests for accessor reading**

```rust
#[test]
fn test_read_f32_accessor() {
    // Simulate a buffer with 3 float32 values
    let buffer: Vec<u8> = [1.0f32, 2.0, 3.0].iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let data = read_floats(&buffer, 0, 3);
    assert_eq!(data, vec![1.0, 2.0, 3.0]);
}

#[test]
fn test_read_u16_indices() {
    let buffer: Vec<u8> = [0u16, 1, 2].iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let indices = read_indices_u16(&buffer, 0, 3);
    assert_eq!(indices, vec![0u32, 1, 2]);
}
```

- [ ] **Step 2: Run tests to verify failure**

- [ ] **Step 3: Implement accessor reading and mesh extraction**

```rust
fn extract_meshes(json: &JsonValue, buffers: &[Vec<u8>]) -> Result<GltfData, String> {
    let accessors = json.get("accessors").and_then(|v| v.as_array()).unwrap_or(&[]);
    let buffer_views = json.get("bufferViews").and_then(|v| v.as_array()).unwrap_or(&[]);
    let materials_json = json.get("materials").and_then(|v| v.as_array());

    let mut meshes = Vec::new();

    let mesh_list = json.get("meshes").and_then(|v| v.as_array())
        .ok_or("No meshes in glTF")?;

    for mesh_val in mesh_list {
        let name = mesh_val.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let primitives = mesh_val.get("primitives").and_then(|v| v.as_array())
            .ok_or("Mesh has no primitives")?;

        for prim in primitives {
            let attrs = prim.get("attributes").and_then(|v| v.as_object())
                .ok_or("Primitive has no attributes")?;

            // Read position data (required)
            let pos_idx = attrs.iter().find(|(k, _)| k == "POSITION")
                .and_then(|(_, v)| v.as_u32())
                .ok_or("Missing POSITION attribute")? as usize;
            let positions = read_accessor_vec3(accessors, buffer_views, buffers, pos_idx)?;

            // Read normals (optional)
            let normals = if let Some(idx) = attrs.iter().find(|(k, _)| k == "NORMAL").and_then(|(_, v)| v.as_u32()) {
                read_accessor_vec3(accessors, buffer_views, buffers, idx as usize)?
            } else {
                vec![[0.0, 1.0, 0.0]; positions.len()]
            };

            // Read UVs (optional)
            let uvs = if let Some(idx) = attrs.iter().find(|(k, _)| k == "TEXCOORD_0").and_then(|(_, v)| v.as_u32()) {
                read_accessor_vec2(accessors, buffer_views, buffers, idx as usize)?
            } else {
                vec![[0.0, 0.0]; positions.len()]
            };

            // Read tangents (optional)
            let tangents = if let Some(idx) = attrs.iter().find(|(k, _)| k == "TANGENT").and_then(|(_, v)| v.as_u32()) {
                Some(read_accessor_vec4(accessors, buffer_views, buffers, idx as usize)?)
            } else {
                None
            };

            // Read indices
            let indices = if let Some(idx) = prim.get("indices").and_then(|v| v.as_u32()) {
                read_accessor_indices(accessors, buffer_views, buffers, idx as usize)?
            } else {
                // No indices — generate sequential
                (0..positions.len() as u32).collect()
            };

            // Assemble vertices
            let mut vertices: Vec<MeshVertex> = Vec::with_capacity(positions.len());
            for i in 0..positions.len() {
                vertices.push(MeshVertex {
                    position: positions[i],
                    normal: normals[i],
                    uv: uvs[i],
                    tangent: tangents.as_ref().map_or([0.0; 4], |t| t[i]),
                });
            }

            // Compute tangents if not provided
            if tangents.is_none() {
                compute_tangents(&mut vertices, &indices);
            }

            // Read material
            let material = prim.get("material")
                .and_then(|v| v.as_u32())
                .and_then(|idx| materials_json?.get(idx as usize))
                .and_then(|mat| extract_material(mat));

            meshes.push(GltfMesh { vertices, indices, name: name.clone(), material });
        }
    }

    Ok(GltfData { meshes })
}

fn get_buffer_data<'a>(
    accessor: &JsonValue,
    buffer_views: &[JsonValue],
    buffers: &'a [Vec<u8>],
) -> Result<(&'a [u8], usize), String> {
    let bv_idx = accessor.get("bufferView").and_then(|v| v.as_u32())
        .ok_or("Accessor missing bufferView")? as usize;
    let bv = buffer_views.get(bv_idx).ok_or("BufferView index out of range")?;
    let buf_idx = bv.get("buffer").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
    let bv_offset = bv.get("byteOffset").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
    let acc_offset = accessor.get("byteOffset").and_then(|v| v.as_u32()).unwrap_or(0) as usize;
    let buffer = buffers.get(buf_idx).ok_or("Buffer index out of range")?;
    let offset = bv_offset + acc_offset;
    Ok((buffer, offset))
}

fn read_accessor_vec3(
    accessors: &[JsonValue], buffer_views: &[JsonValue], buffers: &[Vec<u8>], idx: usize,
) -> Result<Vec<[f32; 3]>, String> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = acc.get("count").and_then(|v| v.as_u32()).ok_or("Missing count")? as usize;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 12;
        if o + 12 > buffer.len() { return Err("Buffer overflow reading vec3".into()); }
        let x = f32::from_le_bytes([buffer[o], buffer[o+1], buffer[o+2], buffer[o+3]]);
        let y = f32::from_le_bytes([buffer[o+4], buffer[o+5], buffer[o+6], buffer[o+7]]);
        let z = f32::from_le_bytes([buffer[o+8], buffer[o+9], buffer[o+10], buffer[o+11]]);
        result.push([x, y, z]);
    }
    Ok(result)
}

fn read_accessor_vec2(
    accessors: &[JsonValue], buffer_views: &[JsonValue], buffers: &[Vec<u8>], idx: usize,
) -> Result<Vec<[f32; 2]>, String> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = acc.get("count").and_then(|v| v.as_u32()).ok_or("Missing count")? as usize;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 8;
        if o + 8 > buffer.len() { return Err("Buffer overflow reading vec2".into()); }
        let x = f32::from_le_bytes([buffer[o], buffer[o+1], buffer[o+2], buffer[o+3]]);
        let y = f32::from_le_bytes([buffer[o+4], buffer[o+5], buffer[o+6], buffer[o+7]]);
        result.push([x, y]);
    }
    Ok(result)
}

fn read_accessor_vec4(
    accessors: &[JsonValue], buffer_views: &[JsonValue], buffers: &[Vec<u8>], idx: usize,
) -> Result<Vec<[f32; 4]>, String> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = acc.get("count").and_then(|v| v.as_u32()).ok_or("Missing count")? as usize;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * 16;
        if o + 16 > buffer.len() { return Err("Buffer overflow reading vec4".into()); }
        let x = f32::from_le_bytes([buffer[o], buffer[o+1], buffer[o+2], buffer[o+3]]);
        let y = f32::from_le_bytes([buffer[o+4], buffer[o+5], buffer[o+6], buffer[o+7]]);
        let z = f32::from_le_bytes([buffer[o+8], buffer[o+9], buffer[o+10], buffer[o+11]]);
        let w = f32::from_le_bytes([buffer[o+12], buffer[o+13], buffer[o+14], buffer[o+15]]);
        result.push([x, y, z, w]);
    }
    Ok(result)
}

fn read_accessor_indices(
    accessors: &[JsonValue], buffer_views: &[JsonValue], buffers: &[Vec<u8>], idx: usize,
) -> Result<Vec<u32>, String> {
    let acc = accessors.get(idx).ok_or("Accessor index out of range")?;
    let count = acc.get("count").and_then(|v| v.as_u32()).ok_or("Missing count")? as usize;
    let comp_type = acc.get("componentType").and_then(|v| v.as_u32()).ok_or("Missing componentType")?;
    let (buffer, offset) = get_buffer_data(acc, buffer_views, buffers)?;

    let mut result = Vec::with_capacity(count);
    match comp_type {
        5121 => { // UNSIGNED_BYTE
            for i in 0..count {
                result.push(buffer[offset + i] as u32);
            }
        }
        5123 => { // UNSIGNED_SHORT
            for i in 0..count {
                let o = offset + i * 2;
                result.push(u16::from_le_bytes([buffer[o], buffer[o+1]]) as u32);
            }
        }
        5125 => { // UNSIGNED_INT
            for i in 0..count {
                let o = offset + i * 4;
                result.push(u32::from_le_bytes([buffer[o], buffer[o+1], buffer[o+2], buffer[o+3]]));
            }
        }
        _ => return Err(format!("Unsupported index component type: {}", comp_type)),
    }
    Ok(result)
}

fn extract_material(mat: &JsonValue) -> Option<GltfMaterial> {
    let pbr = mat.get("pbrMetallicRoughness")?;
    let base_color = if let Some(arr) = pbr.get("baseColorFactor").and_then(|v| v.as_array()) {
        [
            arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        ]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    };
    let metallic = pbr.get("metallicFactor").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let roughness = pbr.get("roughnessFactor").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    Some(GltfMaterial { base_color, metallic, roughness })
}

// Helper functions for tests
fn read_floats(buffer: &[u8], offset: usize, count: usize) -> Vec<f32> {
    (0..count).map(|i| {
        let o = offset + i * 4;
        f32::from_le_bytes([buffer[o], buffer[o+1], buffer[o+2], buffer[o+3]])
    }).collect()
}

fn read_indices_u16(buffer: &[u8], offset: usize, count: usize) -> Vec<u32> {
    (0..count).map(|i| {
        let o = offset + i * 2;
        u16::from_le_bytes([buffer[o], buffer[o+1]]) as u32
    }).collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package voltex_renderer -- gltf::tests -v`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voltex_renderer/src/gltf.rs
git commit -m "feat(renderer): add glTF accessor/bufferView extraction and mesh assembly"
```

---

### Task 4: GLB Integration Test with Synthetic Triangle

**Files:**
- Modify: `crates/voltex_renderer/src/gltf.rs`

- [ ] **Step 1: Write integration test**

```rust
#[test]
fn test_parse_minimal_glb() {
    let glb = build_minimal_glb_triangle();
    let data = parse_gltf(&glb).unwrap();
    assert_eq!(data.meshes.len(), 1);
    let mesh = &data.meshes[0];
    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices.len(), 3);
    // Verify positions
    assert_eq!(mesh.vertices[0].position, [0.0, 0.0, 0.0]);
    assert_eq!(mesh.vertices[1].position, [1.0, 0.0, 0.0]);
    assert_eq!(mesh.vertices[2].position, [0.0, 1.0, 0.0]);
}

/// Build a minimal GLB with one triangle.
fn build_minimal_glb_triangle() -> Vec<u8> {
    // Binary buffer: 3 positions (vec3) + 3 indices (u16)
    let mut bin = Vec::new();
    // Positions: 3 * vec3 = 36 bytes
    for &v in &[0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        bin.extend_from_slice(&v.to_le_bytes());
    }
    // Indices: 3 * u16 = 6 bytes + 2 padding = 8 bytes
    for &i in &[0u16, 1, 2] {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    bin.extend_from_slice(&[0, 0]); // padding to 4-byte alignment

    let json_str = format!(r#"{{
        "asset": {{"version": "2.0"}},
        "buffers": [{{"byteLength": {}}}],
        "bufferViews": [
            {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
            {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
        ],
        "accessors": [
            {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
              "max": [1.0, 1.0, 0.0], "min": [0.0, 0.0, 0.0]}},
            {{"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}}
        ],
        "meshes": [{{
            "name": "Triangle",
            "primitives": [{{
                "attributes": {{"POSITION": 0}},
                "indices": 1
            }}]
        }}]
    }}"#, bin.len());

    let json_bytes = json_str.as_bytes();
    // Pad JSON to 4-byte alignment
    let json_padded_len = (json_bytes.len() + 3) & !3;
    let mut json_padded = json_bytes.to_vec();
    while json_padded.len() < json_padded_len {
        json_padded.push(b' ');
    }

    let total_len = 12 + 8 + json_padded.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total_len);

    // Header
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());

    // JSON chunk
    glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(&json_padded);

    // BIN chunk
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    glb.extend_from_slice(&bin);

    glb
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package voltex_renderer -- gltf::tests::test_parse_minimal_glb -v`
Expected: PASS

- [ ] **Step 3: Add material test**

```rust
#[test]
fn test_parse_glb_with_material() {
    // Same triangle but with a material
    let glb = build_glb_with_material();
    let data = parse_gltf(&glb).unwrap();
    let mesh = &data.meshes[0];
    let mat = mesh.material.as_ref().unwrap();
    assert!((mat.base_color[0] - 1.0).abs() < 0.01);
    assert!((mat.metallic - 0.5).abs() < 0.01);
    assert!((mat.roughness - 0.8).abs() < 0.01);
}
```

- [ ] **Step 4: Run all glTF tests**

Run: `cargo test --package voltex_renderer -- gltf::tests -v`
Expected: All PASS

- [ ] **Step 5: Run full workspace build**

Run: `cargo build --workspace`
Expected: BUILD SUCCESS

- [ ] **Step 6: Commit**

```bash
git add crates/voltex_renderer/src/gltf.rs
git commit -m "feat(renderer): complete glTF/GLB parser with mesh and material extraction"
```
