/// Packet type IDs
const TYPE_CONNECT: u8 = 1;
const TYPE_ACCEPT: u8 = 2;
const TYPE_DISCONNECT: u8 = 3;
const TYPE_PING: u8 = 4;
const TYPE_PONG: u8 = 5;
const TYPE_USER_DATA: u8 = 6;

/// Header size: type_id(1) + payload_len(2 LE) + reserved(1) = 4 bytes
const HEADER_SIZE: usize = 4;

/// All packet variants for the Voltex network protocol.
#[derive(Debug, Clone, PartialEq)]
pub enum Packet {
    Connect { client_name: String },
    Accept { client_id: u32 },
    Disconnect { client_id: u32 },
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    UserData { client_id: u32, data: Vec<u8> },
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
    pub fn from_bytes(data: &[u8]) -> Result<Packet, String> {
        if data.len() < HEADER_SIZE {
            return Err(format!(
                "Buffer too short for header: {} bytes",
                data.len()
            ));
        }

        let type_id = data[0];
        let payload_len = u16::from_le_bytes([data[1], data[2]]) as usize;
        // data[3] is reserved, ignored

        if data.len() < HEADER_SIZE + payload_len {
            return Err(format!(
                "Buffer too short: expected {} bytes, got {}",
                HEADER_SIZE + payload_len,
                data.len()
            ));
        }

        let payload = &data[HEADER_SIZE..HEADER_SIZE + payload_len];

        match type_id {
            TYPE_CONNECT => {
                if payload.len() < 2 {
                    return Err("Connect payload too short".to_string());
                }
                let name_len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
                if payload.len() < 2 + name_len {
                    return Err("Connect name bytes too short".to_string());
                }
                let client_name = String::from_utf8(payload[2..2 + name_len].to_vec())
                    .map_err(|e| format!("Invalid UTF-8 in client_name: {}", e))?;
                Ok(Packet::Connect { client_name })
            }
            TYPE_ACCEPT => {
                if payload.len() < 4 {
                    return Err("Accept payload too short".to_string());
                }
                let client_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                Ok(Packet::Accept { client_id })
            }
            TYPE_DISCONNECT => {
                if payload.len() < 4 {
                    return Err("Disconnect payload too short".to_string());
                }
                let client_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                Ok(Packet::Disconnect { client_id })
            }
            TYPE_PING => {
                if payload.len() < 8 {
                    return Err("Ping payload too short".to_string());
                }
                let timestamp = u64::from_le_bytes([
                    payload[0], payload[1], payload[2], payload[3],
                    payload[4], payload[5], payload[6], payload[7],
                ]);
                Ok(Packet::Ping { timestamp })
            }
            TYPE_PONG => {
                if payload.len() < 8 {
                    return Err("Pong payload too short".to_string());
                }
                let timestamp = u64::from_le_bytes([
                    payload[0], payload[1], payload[2], payload[3],
                    payload[4], payload[5], payload[6], payload[7],
                ]);
                Ok(Packet::Pong { timestamp })
            }
            TYPE_USER_DATA => {
                if payload.len() < 4 {
                    return Err("UserData payload too short".to_string());
                }
                let client_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let data = payload[4..].to_vec();
                Ok(Packet::UserData { client_id, data })
            }
            _ => Err(format!("Unknown packet type_id: {}", type_id)),
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
    fn test_invalid_type_returns_error() {
        // Build a packet with type_id = 99 (unknown)
        let bytes = vec![99u8, 0, 0, 0]; // type=99, payload_len=0, reserved=0
        let result = Packet::from_bytes(&bytes);
        assert!(result.is_err(), "Expected error for unknown type_id");
    }
}
