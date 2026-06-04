//! # voltex_editor
//!
//! Immediate-mode editor stack on top of `voltex_renderer`:
//!
//! - UI: [`UiContext`] + [`DrawList`] + [`UiRenderer`] (host-side commands,
//!   wgpu pipeline). [`widgets`] provides text/button/slider/checkbox/panel
//!   primitives.
//! - Layout: [`DockTree`] binary-split layout + [`FloatingPanel`].
//! - 3D scene: [`OrbitCamera`], [`ViewportRenderer`] + [`ViewportTexture`],
//!   [`pick_entity`] sphere picking, [`gizmo`] translate/rotate/scale handles.
//! - Fonts: [`FontAtlas`] (bitmap), [`TtfFont`] + [`ttf_parser`] + [`rasterizer`]
//!   + [`glyph_cache`] for TrueType.
//! - Editor sessions: [`EditorSession`] + [`EditorCommand`] + Undo/Redo +
//!   jsonl autosave.

#![warn(missing_docs)]

pub mod error;
pub mod font;
pub mod draw_list;
pub mod layout;
pub mod renderer;
pub mod ui_context;
pub mod widgets;
pub mod dock;
pub mod orbit_camera;
pub mod picking;
pub mod command;
pub mod session;
pub mod panels;

pub use error::{EditorError, Result};
pub use font::FontAtlas;
pub use draw_list::{DrawVertex, DrawCommand, DrawList};
pub use layout::LayoutState;
pub use renderer::UiRenderer;
pub use ui_context::UiContext;
pub use dock::{DockTree, DockNode, Axis, Rect, LeafLayout};
pub use panels::hierarchy::{hierarchy_panel, count_entity_nodes};
pub use panels::inspector::{
    attached_component_names, component_value_summary, inspector_panel, InspectorState,
};
pub use orbit_camera::OrbitCamera;
pub use picking::pick_entity;

pub mod gizmo;
pub use gizmo::{DragResult, GizmoAxis, GizmoMode, GizmoRenderer, GizmoState};

pub mod lifecycle;
pub use lifecycle::{LifecycleCallback, LifecycleEvent};

pub mod viewport_texture;
pub use viewport_texture::{ViewportTexture, VIEWPORT_COLOR_FORMAT, VIEWPORT_DEPTH_FORMAT};

pub mod viewport_renderer;
pub use viewport_renderer::ViewportRenderer;

pub mod asset_browser;
pub use asset_browser::{AssetBrowser, AssetKind, BrowserView, asset_browser_panel, grid_columns};

pub mod ttf_parser;
pub mod rasterizer;
pub mod glyph_cache;
pub mod ttf_font;
pub use ttf_font::TtfFont;

pub mod floating_panel;
pub use floating_panel::{FloatingPanel, FloatingPanelManager};

pub use command::EditorCommand;
pub use session::{EditorMode, EditorSession};
