# Phase 8-2: Networking Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** UDP 기반 클라이언트-서버 네트워킹 인프라 — 패킷 직렬화, 연결 핸드셰이크, 데이터 송수신

**Architecture:** `voltex_net` crate 신규. 바이너리 패킷 프로토콜 + non-blocking UdpSocket 래퍼 + NetServer(클라이언트 관리) + NetClient(서버 연결). std::net만 사용.

**Tech Stack:** Rust, std::net::UdpSocket

**Spec:** `docs/superpowers/specs/2026-03-25-phase8-2-networking.md`

---

## Task 1: Crate + Packet 직렬화

Create crate, packet.rs with Packet enum and binary serialization.

- Cargo.toml: no dependencies
- Workspace: add members + workspace.dependencies
- Packet enum: Connect, Accept, Disconnect, Ping, Pong, UserData
- Header: [type u8, payload_len u16 LE, reserved u8]
- to_bytes(), from_bytes()
- 6 tests: roundtrip per variant + invalid bytes error

Commit: `feat(net): add voltex_net crate with packet serialization`

## Task 2: NetSocket + NetServer + NetClient

Create socket.rs, server.rs, client.rs with full networking.

- NetSocket: bind(non-blocking), send_to, recv_from
- ClientInfo, ServerEvent, NetServer: poll, broadcast, send_to_client
- ClientEvent, NetClient: connect, poll, send, is_connected, disconnect
- Tests: localhost handshake, UserData roundtrip, disconnect

Commit: `feat(net): add UDP server/client with connection management`

## Task 3: Documentation

Update STATUS.md and DEFERRED.md.

Commit: `docs: add Phase 8-2 networking status and deferred items`
