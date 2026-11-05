//! On-wire protocol for the Voltex network layer. Defines the [`Packet`] enum
//! and its `to_bytes` / `from_bytes` serialisers. Each packet begins with a
//! 4-byte header: `type_id(1) | payload_len(2 LE) | reserved(1)`.

use crate::wire::{read_u16, read_u32, read_u64};

/// Packet type IDs
const TYPE_CONNECT: u8 = 1;
const TYPE_ACCEPT: u8 = 2;
const TYPE_DISCONNECT: u8 = 3;
const TYPE_PING: u8 = 4;
const TYPE_PONG: u8 = 5;
const TYPE_USER_DATA: u8 = 6;
const TYPE_RELIABLE: u8 = 7;
const TYPE_ACK: u8 = 8;
const TYPE_SNAPSHOT: u8 = 9;
const TYPE_SNAPSHOT_DELTA: u8 = 10;

/// Header size: type_id(1) + payload_len(2 LE) + reserved(1) = 4 bytes
const HEADER_SIZE: usize = 4;

/// All packet variants for the Voltex network protocol.
///
/// Wire format: `[type_id(1) | payload_len(2 LE) | reserved(1) | payload..]`.
#[derive(Debug, Clone, PartialEq)]
pub enum Packet {
    /// Client → server: initial connect request carrying a display name.
    Connect {
        /// Self-reported display name.
        client_name: String,
    },
    /// Server → client: accept the connection and assign `client_id`.
    Accept {
        /// Server-assigned client id (used to identify the peer afterwards).
        client_id: u32,
    },
    /// Either side: explicit disconnect notice.
    Disconnect {
        /// Id of the client being disconnected.
        client_id: u32,
    },
    /// Client → server: latency probe. Echoed back as a [`Packet::Pong`].
    Ping {
        /// Sender's local timestamp (caller-defined units, typically ms).
        timestamp: u64,
    },
    /// Server → client: response to a [`Packet::Ping`].
    Pong {
        /// `timestamp` from the originating Ping.
        timestamp: u64,
    },
    /// Either side: opaque application-level bytes.
    UserData {
        /// Source / target client id.
        client_id: u32,
        /// Application bytes (interpretation is up to the user).
        data: Vec<u8>,
    },
    /// Either side: reliable-channel payload (sequence-tagged, retransmitted on loss).
    Reliable {
        /// Channel sequence number assigned by the sender.
        sequence: u16,
        /// Reliable payload bytes.
        data: Vec<u8>,
    },
    /// Either side: acknowledgement of a previously sent [`Packet::Reliable`].
    Ack {
        /// Sequence number being acknowledged.
        sequence: u16,
    },
    /// Server → client: full world snapshot at a given tick.
    Snapshot {
        /// Server tick at which this snapshot was captured.
        tick: u32,
        /// Serialized snapshot bytes (see [`serialize_snapshot`](crate::serialize_snapshot)).
        data: Vec<u8>,
    },
    /// Server → client: delta from `base_tick` to `tick` (bandwidth save).
    SnapshotDelta {
        /// Reference tick the receiver must already hold.
        base_tick: u32,
        /// Tick the diff produces when applied to `base_tick`.
        tick: u32,
        /// Serialized delta bytes (see [`diff_snapshots`](crate::diff_snapshots)).
        data: Vec<u8>,
    },
}

impl Packet {
    /// Serialize the packet into bytes: [type_id(1), payload_len(2 LE), reserved(1), payload...]
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload = self.encode_payload();
        let payload_len = payload.len() as u16;

        let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
        buf.push(self.type_id());
        buf.extend_from_slice(&payload_len.to_le_bytes());
        buf.push(0u8); // reserved
        buf.extend_from_slice(&payload);
        buf
    }

    /// Deserialize a packet from bytes.
    pub fn from_bytes(data: &[u8]) -> crate::Result<Packet> {
        if data.len() < HEADER_SIZE {
            return Err(format!(
                "Buffer too short for header: {} bytes",
                data.len()
            ).into());
        }

        let type_id = data[0];
        let payload_len = read_u16(data, 1) as usize;
        // data[3] is reserved, ignored

        if data.len() < HEADER_SIZE + payload_len {
            return Err(format!(
                "Buffer too short: expected {} bytes, got {}",
                HEADER_SIZE + payload_len,
                data.len()
            ).into());
        }

        let payload = &data[HEADER_SIZE..HEADER_SIZE + payload_len];
        Self::decode_payload(type_id, payload)
    }

    /// Decodes the payload of a packet whose header said `type_id`.
    fn decode_payload(type_id: u8, payload: &[u8]) -> crate::Result<Packet> {
        // Every variant checks its fixed-size prefix before reading it.
        let require = |min_len: usize, what: &str| -> crate::Result<()> {
            if payload.len() < min_len {
                Err(format!("{what} payload too short").into())
            } else {
                Ok(())
            }
        };
        match type_id {
            TYPE_CONNECT => {
                require(2, "Connect")?;
                let name_len = read_u16(payload, 0) as usize;
                if payload.len() < 2 + name_len {
                    return Err("Connect name bytes too short".into());
                }
                let client_name = String::from_utf8(payload[2..2 + name_len].to_vec())
                    .map_err(|e| format!("Invalid UTF-8 in client_name: {}", e))?;
                Ok(Packet::Connect { client_name })
            }
            TYPE_ACCEPT => {
                require(4, "Accept")?;
                Ok(Packet::Accept { client_id: read_u32(payload, 0) })
            }
            TYPE_DISCONNECT => {
                require(4, "Disconnect")?;
                Ok(Packet::Disconnect { client_id: read_u32(payload, 0) })
            }
            TYPE_PING => {
                require(8, "Ping")?;
                Ok(Packet::Ping { timestamp: read_u64(payload, 0) })
            }
            TYPE_PONG => {
                require(8, "Pong")?;
                Ok(Packet::Pong { timestamp: read_u64(payload, 0) })
            }
            TYPE_USER_DATA => {
                require(4, "UserData")?;
                Ok(Packet::UserData { client_id: read_u32(payload, 0), data: payload[4..].to_vec() })
            }
            TYPE_RELIABLE => {
                require(2, "Reliable")?;
                Ok(Packet::Reliable { sequence: read_u16(payload, 0), data: payload[2..].to_vec() })
            }
            TYPE_ACK => {
                require(2, "Ack")?;
                Ok(Packet::Ack { sequence: read_u16(payload, 0) })
            }
            TYPE_SNAPSHOT => {
                require(4, "Snapshot")?;
                Ok(Packet::Snapshot { tick: read_u32(payload, 0), data: payload[4..].to_vec() })
            }
            TYPE_SNAPSHOT_DELTA => {
                require(8, "SnapshotDelta")?;
                Ok(Packet::SnapshotDelta {
                    base_tick: read_u32(payload, 0),
                    tick: read_u32(payload, 4),
                    data: payload[8..].to_vec(),
                })
            }
            _ => Err(format!("Unknown packet type_id: {}", type_id).into()),
        }
    }

    fn type_id(&self) -> u8 {
        match self {
            Packet::Connect { .. } => TYPE_CONNECT,
            Packet::Accept { .. } => TYPE_ACCEPT,
            Packet::Disconnect { .. } => TYPE_DISCONNECT,
            Packet::Ping { .. } => TYPE_PING,
            Packet::Pong { .. } => TYPE_PONG,
            Packet::UserData { .. } => TYPE_USER_DATA,
            Packet::Reliable { .. } => TYPE_RELIABLE,
            Packet::Ack { .. } => TYPE_ACK,
            Packet::Snapshot { .. } => TYPE_SNAPSHOT,
            Packet::SnapshotDelta { .. } => TYPE_SNAPSHOT_DELTA,
        }
    }

    fn encode_payload(&self) -> Vec<u8> {
        match self {
            Packet::Connect { client_name } => {
                let name_bytes = client_name.as_bytes();
                let name_len = name_bytes.len() as u16;
                let mut buf = Vec::with_capacity(2 + name_bytes.len());
                buf.extend_from_slice(&name_len.to_le_bytes());
                buf.extend_from_slice(name_bytes);
                buf
            }
            Packet::Accept { client_id } => client_id.to_le_bytes().to_vec(),
            Packet::Disconnect { client_id } => client_id.to_le_bytes().to_vec(),
            Packet::Ping { timestamp } => timestamp.to_le_bytes().to_vec(),
            Packet::Pong { timestamp } => timestamp.to_le_bytes().to_vec(),
            Packet::UserData { client_id, data } => {
                let mut buf = Vec::with_capacity(4 + data.len());
                buf.extend_from_slice(&client_id.to_le_bytes());
                buf.extend_from_slice(data);
                buf
            }
            Packet::Reliable { sequence, data } => {
                let mut buf = Vec::with_capacity(2 + data.len());
                buf.extend_from_slice(&sequence.to_le_bytes());
                buf.extend_from_slice(data);
                buf
            }
            Packet::Ack { sequence } => sequence.to_le_bytes().to_vec(),
            Packet::Snapshot { tick, data } => {
                let mut buf = Vec::with_capacity(4 + data.len());
                buf.extend_from_slice(&tick.to_le_bytes());
                buf.extend_from_slice(data);
                buf
            }
            Packet::SnapshotDelta { base_tick, tick, data } => {
                let mut buf = Vec::with_capacity(8 + data.len());
                buf.extend_from_slice(&base_tick.to_le_bytes());
                buf.extend_from_slice(&tick.to_le_bytes());
                buf.extend_from_slice(data);
                buf
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(packet: Packet) {
        let bytes = packet.to_bytes();
        let decoded = Packet::from_bytes(&bytes).expect("roundtrip failed");
        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_connect_roundtrip() {
        roundtrip(Packet::Connect {
            client_name: "Alice".to_string(),
        });
    }

    #[test]
    fn test_accept_roundtrip() {
        roundtrip(Packet::Accept { client_id: 42 });
    }

    #[test]
    fn test_disconnect_roundtrip() {
        roundtrip(Packet::Disconnect { client_id: 7 });
    }

    #[test]
    fn test_ping_roundtrip() {
        roundtrip(Packet::Ping {
            timestamp: 1_234_567_890_u64,
        });
    }

    #[test]
    fn test_pong_roundtrip() {
        roundtrip(Packet::Pong {
            timestamp: 9_876_543_210_u64,
        });
    }

    #[test]
    fn test_user_data_roundtrip() {
        roundtrip(Packet::UserData {
            client_id: 3,
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        });
    }

    #[test]
    fn test_reliable_roundtrip() {
        roundtrip(Packet::Reliable {
            sequence: 42,
            data: vec![0xCA, 0xFE],
        });
    }

    #[test]
    fn test_ack_roundtrip() {
        roundtrip(Packet::Ack { sequence: 100 });
    }

    #[test]
    fn test_snapshot_roundtrip() {
        roundtrip(Packet::Snapshot {
            tick: 999,
            data: vec![1, 2, 3],
        });
    }

    #[test]
    fn test_snapshot_delta_roundtrip() {
        roundtrip(Packet::SnapshotDelta {
            base_tick: 10,
            tick: 15,
            data: vec![4, 5, 6],
        });
    }

    #[test]
    fn test_invalid_type_returns_error() {
        // Build a packet with type_id = 99 (unknown)
        let bytes = vec![99u8, 0, 0, 0]; // type=99, payload_len=0, reserved=0
        let result = Packet::from_bytes(&bytes);
        assert!(result.is_err(), "Expected error for unknown type_id");
    }
}
