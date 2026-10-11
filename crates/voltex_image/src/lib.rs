//! # voltex_image
//!
//! Dependency-free (no GPU) asset codecs for the Voltex engine: PNG/JPG
//! decoders, a minimal PNG encoder, raw DEFLATE inflation, OBJ + glTF mesh
//! parsing, and a small JSON parser. Decoders return CPU pixel/geometry
//! buffers; GPU upload stays in `voltex_renderer`.
//!
//! This is an L0 crate (no `voltex_*` dependencies) so the codecs can be
//! reused without pulling in wgpu.

#![warn(missing_docs)]

mod checksum;
pub mod deflate;
pub mod error;
pub mod gltf;
pub mod jpg;
pub mod json_parser;
pub mod obj;
pub mod png;
mod png_pixels;
pub mod png_encode;
pub mod progressive_jpeg;
pub mod vertex;

pub use error::{ImageError, Result};
pub use gltf::{parse_gltf, GltfData, GltfMaterial, GltfMesh};
pub use jpg::parse_jpg;
pub use png::parse_png;
pub use png_encode::encode_rgba8;
pub use vertex::MeshVertex;
