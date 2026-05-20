# voltex_net

## 책임 (한 줄)
UDP 기반 게임 네트워킹 — 신뢰성/순서/암호화/스냅샷/예측 보간.

## 공개 API 진입점
- `NetServer` — UDP 서버 (src/server.rs)
- `NetClient` — UDP 클라이언트 (src/client.rs)
- `Packet` — 패킷 프로토콜 (src/packet.rs)
- `ReliableChannel` — sequence + ACK + 재전송 + RTT (src/reliable.rs)
- `Snapshot` — 상태 직렬화 + delta 압축 (src/snapshot.rs)
- `InterpolationBuffer` — 클라이언트 보간 (src/interpolation.rs)
- `PacketCipher`, `AuthToken` — XOR 암호화 + anti-replay + 토큰 (src/encryption.rs)
- `LagCompensation` — 히스토리 rewind (src/lag_compensation.rs)

## 모듈 구조
- `packet` — 패킷 직렬화/역직렬화 (src/packet.rs)
- `socket` — UDP 소켓 래퍼 (src/socket.rs)
- `server`, `client` — 서버/클라이언트 (src/server.rs, src/client.rs)
- `reliable` — 신뢰성 채널 (src/reliable.rs)
- `snapshot` — 스냅샷 + delta (src/snapshot.rs)
- `interpolation` — 보간 버퍼 (src/interpolation.rs)
- `encryption` — 암호화/인증 (src/encryption.rs)
- `lag_compensation` — 지연 보상 (src/lag_compensation.rs)

## 의존성
- voltex crate: 없음
- 외부 crate: 없음 (std::net 사용)

## 변경 시 주의점
- Packet 와이어 포맷 변경 시 클라이언트/서버 모두 재컴파일 필요 (버전 호환 없음)
- ReliableChannel의 sequence 32-bit wraparound 고려

## 테스트
- 단위 테스트: src/**/*.rs `#[cfg(test)]`
- 실행: `cargo test -p voltex_net --lib`
- 현재 테스트 개수: 47개
