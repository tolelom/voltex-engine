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

pub mod error;
pub use error::{ImageError, Result};
