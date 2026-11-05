//! # voltex_net
//!
//! UDP-based networking primitives used by the engine:
//!
//! - [`Packet`] — the on-wire protocol (Connect/Accept/Disconnect/Ping/Pong/
//!   UserData/Reliable/Ack/Snapshot/SnapshotDelta).
//! - [`NetSocket`] — non-blocking UDP wrapper.
//! - [`NetServer`] / [`NetClient`] — listen/connect endpoints with
//!   client-id-bookkeeping and ping/timeout tracking.
//! - [`ReliableChannel`] / [`OrderedChannel`] — sequence + ACK + retransmission
//!   on top of UDP, with optional in-order delivery.
//! - [`Snapshot`] — entity-state container + delta compression (`diff_snapshots`
//!   / `apply_snapshot_diff`).
//! - [`InterpolationBuffer`] — client-side linear interpolation over
//!   `Snapshot`s.
//! - [`LagCompensation`] — server-side history rewind for hit registration.
//! - [`PacketCipher`] / [`AuthToken`] — minimal symmetric crypto used for
//!   authenticated packet framing.

#![warn(missing_docs)]

pub mod error;
pub mod packet;
pub mod socket;
pub mod server;
pub mod client;
pub mod reliable;
pub mod snapshot;
pub mod interpolation;
pub mod lag_compensation;
pub mod encryption;
mod wire;

pub use error::{NetError, Result};
pub use packet::{Packet, MAX_PAYLOAD_LEN};
pub use socket::NetSocket;
pub use server::{NetServer, ServerEvent, ClientInfo};
pub use client::{NetClient, ClientEvent};
pub use reliable::{ReliableChannel, OrderedChannel};
pub use snapshot::{Snapshot, EntityState, serialize_snapshot, deserialize_snapshot, diff_snapshots, apply_snapshot_diff};
#[allow(deprecated)]
pub use snapshot::apply_diff;
pub use interpolation::InterpolationBuffer;
pub use lag_compensation::LagCompensation;
pub use encryption::{PacketCipher, AuthToken};
