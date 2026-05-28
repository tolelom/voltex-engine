//! # voltex_ai
//!
//! AI primitives for the engine:
//!
//! - [`NavMesh`] — triangle-based navigation mesh.
//! - [`find_path`] — A\* pathfinding over a [`NavMesh`].
//! - [`NavAgent`] — ECS component tracking target + computed waypoint path.
//! - [`SteeringAgent`] + [`seek`] / [`flee`] / [`arrive`] / [`wander`] /
//!   [`follow_path`] — Reynolds-style steering behaviours.

#![warn(missing_docs)]

pub mod error;
pub mod nav_agent;
pub mod navmesh;
pub mod pathfinding;
pub mod steering;

pub use error::{AiError, Result};
pub use nav_agent::NavAgent;
pub use navmesh::{deserialize_navmesh, serialize_navmesh, NavMesh, NavTriangle};
pub use pathfinding::find_path;
pub use steering::{SteeringAgent, seek, flee, arrive, wander, follow_path};
