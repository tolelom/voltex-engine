# Phase 8-2: Networking Foundation — Design Spec

## Overview

`voltex_net` crate를 신규 생성한다. UDP 소켓 래핑, 패킷 직렬화, 클라이언트-서버 연결 관리를 구현한다.

## Scope

- 패킷 프로토콜 (Connect, Accept, Disconnect, Ping, Pong, UserData)
- 바이너리 직렬화/역직렬화
- Non-blocking UDP 소켓 래퍼
- NetServer (클라이언트 관리, broadcast)
- NetClient (서버 연결, 핸드셰이크)

## Out of Scope

- 상태 동기화 (스냅샷)
- 보간 / 예측
- 지연 보상
- 신뢰성 계층 (재전송, 순서 보장)
- 암호화 / 인증

## Module Structure

```
crates/voltex_net/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── packet.rs    — Packet enum, to_bytes, from_bytes
    ├── socket.rs    — NetSocket (UdpSocket wrapper)
    ├── server.rs    — NetServer
    └── client.rs    — NetClient
```

## Dependencies

- 없음 (std::net::UdpSocket만 사용)

## Types

### Packet

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Packet {
    Connect { client_name: String },
    Accept { client_id: u32 },
    Disconnect { client_id: u32 },
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    UserData { client_id: u32, data: Vec<u8> },
}
```

### 직렬화 포맷

```
[type: u8] [payload_len: u16 LE] [reserved: u8] [payload...]
```

Type IDs: Connect=1, Accept=2, Disconnect=3, Ping=4, Pong=5, UserData=6

Payload encoding:
- Connect: name_len(u16) + name_bytes
- Accept: client_id(u32)
- Disconnect: client_id(u32)
- Ping/Pong: timestamp(u64)
- UserData: client_id(u32) + data_len(u16) + data_bytes

```rust
impl Packet {
    pub fn to_bytes(&self) -> Vec<u8>
    pub fn from_bytes(data: &[u8]) -> Result<Packet, String>
}
```

### NetSocket

```rust
pub struct NetSocket {
    socket: UdpSocket,
}
```

- `bind(addr: &str) -> Result<Self, String>` — bind + set_nonblocking(true)
- `send_to(&self, packet: &Packet, addr: SocketAddr) -> Result<(), String>`
- `recv_from(&self) -> Option<(Packet, SocketAddr)>` — None if WouldBlock

### ClientInfo

```rust
pub struct ClientInfo {
    pub id: u32,
    pub addr: SocketAddr,
    pub name: String,
}
```

### NetServer

```rust
pub struct NetServer {
    socket: NetSocket,
    clients: HashMap<u32, ClientInfo>,
    next_id: u32,
}
```

- `new(addr: &str) -> Result<Self, String>`
- `poll(&mut self) -> Vec<ServerEvent>` — process incoming packets
- `broadcast(&self, packet: &Packet)` — send to all clients
- `send_to_client(&self, client_id: u32, packet: &Packet)`
- `clients(&self) -> &HashMap<u32, ClientInfo>`
- `client_count(&self) -> usize`

### ServerEvent

```rust
pub enum ServerEvent {
    ClientConnected { client_id: u32, name: String },
    ClientDisconnected { client_id: u32 },
    PacketReceived { client_id: u32, packet: Packet },
}
```

### NetClient

```rust
pub struct NetClient {
    socket: NetSocket,
    server_addr: SocketAddr,
    client_id: Option<u32>,
    name: String,
}
```

- `new(local_addr: &str, server_addr: &str, name: &str) -> Result<Self, String>`
- `connect(&self)` — send Connect packet
- `poll(&mut self) -> Vec<ClientEvent>` — process incoming
- `send(&self, packet: &Packet) -> Result<(), String>`
- `is_connected(&self) -> bool`
- `client_id(&self) -> Option<u32>`
- `disconnect(&self)`

### ClientEvent

```rust
pub enum ClientEvent {
    Connected { client_id: u32 },
    Disconnected,
    PacketReceived { packet: Packet },
}
```

## Test Plan

### packet.rs
- Connect 라운드트립
- Accept 라운드트립
- Disconnect 라운드트립
- Ping/Pong 라운드트립
- UserData 라운드트립
- 잘못된 바이트 → 에러

### socket.rs
- localhost 바인드
- send + recv 라운드트립

### server.rs + client.rs (통합)
- 서버 시작 + 클라이언트 연결 → Accept 수신
- 다수 클라이언트 연결
- 클라이언트 Disconnect → 서버에서 제거
- UserData 송수신
