//! # voltex_ai
//!
//! AI primitives for the engine:
//!
//! - [`NavMesh`] — triangle-based navigation mesh.
//! - [`find_path`] — A\* pathfinding over a [`NavMesh`].
//! - [`NavAgent`] — ECS component tracking target + computed waypoint path.
//! - [`SteeringAgent`] + [`seek`] / [`flee`] / [`arrive`] / [`wander`] /
//!   [`follow_path`] — Reynolds-style steering behaviours.
//! - [`nav_agent_system`] — ECS system driving [`NavAgent`] entities along
//!   NavMesh paths each frame.

#![warn(missing_docs)]

pub mod error;
pub mod flocking;
pub mod nav_agent;
pub mod nav_agent_system;
pub mod navmesh;
pub mod navmesh_builder;
pub mod obstacle;
pub mod pathfinding;
#[cfg(test)]
mod pathfinding_property_tests;
pub mod steering;

pub use error::{AiError, Result};
pub use obstacle::{avoid_obstacles, DynamicObstacle};
pub use flocking::{alignment, cohesion, flocking_system, separation, Boid, BoidParams};
pub use nav_agent::NavAgent;
pub use nav_agent_system::nav_agent_system;
pub use navmesh::{deserialize_navmesh, serialize_navmesh, NavMesh, NavTriangle};
pub use navmesh_builder::NavMeshBuilder;
pub use pathfinding::find_path;
pub use steering::{SteeringAgent, seek, flee, arrive, wander, follow_path};
