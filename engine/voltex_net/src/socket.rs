//! Non-blocking UDP socket wrapper. Encodes/decodes [`Packet`]s on send/recv.

use std::net::{SocketAddr, UdpSocket};

use crate::Packet;

/// Maximum UDP datagram size we'll allocate for receiving.
const MAX_PACKET_SIZE: usize = 65535;

/// A non-blocking UDP socket wrapper.
pub struct NetSocket {
    inner: UdpSocket,
}

impl NetSocket {
    /// Bind a new non-blocking UDP socket to the given address.
    pub fn bind(addr: &str) -> crate::Result<Self> {
        let socket = UdpSocket::bind(addr)
            .map_err(|e| format!("UdpSocket::bind({}) failed: {}", addr, e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("set_nonblocking failed: {}", e))?;
        Ok(NetSocket { inner: socket })
    }

    /// Returns the local address this socket is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr().expect("local_addr unavailable")
    }

    /// Serialize and send a packet to the given address. Fails without sending
    /// if the payload is too large for one packet.
    pub fn send_to(&self, packet: &Packet, addr: SocketAddr) -> crate::Result<()> {
        let bytes = packet.try_to_bytes()?;
        self.inner
            .send_to(&bytes, addr)
            .map_err(|e| format!("send_to failed: {}", e))?;
        Ok(())
    }

    /// Receive the next well-formed packet. Datagrams that fail to parse are
    /// logged and skipped, so `None` always means "nothing more to read right
    /// now" (WouldBlock) or a socket error — callers drain with
    /// `while let Some(..) = socket.recv_from()`.
    pub fn recv_from(&self) -> Option<(Packet, SocketAddr)> {
        let mut buf = vec![0u8; MAX_PACKET_SIZE];
        loop {
            match self.inner.recv_from(&mut buf) {
                Ok((len, addr)) => match Packet::from_bytes(&buf[..len]) {
                    Ok(packet) => return Some((packet, addr)),
                    Err(e) => {
                        eprintln!("[NetSocket] Failed to parse packet from {}: {}", addr, e);
                    }
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return None,
                Err(e) => {
                    eprintln!("[NetSocket] recv_from error: {}", e);
                    return None;
                }
            }
        }
    }

    #[cfg(test)]
    fn send_raw(&self, bytes: &[u8], addr: SocketAddr) {
        self.inner.send_to(bytes, addr).expect("raw send failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn malformed_datagram_does_not_hide_the_packets_after_it() {
        let receiver = NetSocket::bind("127.0.0.1:0").expect("bind receiver");
        let sender = NetSocket::bind("127.0.0.1:0").expect("bind sender");
        let to = receiver.local_addr();

        sender.send_raw(&[0xFF, 0x00], to); // too short to be a packet
        sender.send_to(&Packet::Ping { timestamp: 7 }, to).expect("send ping");
        std::thread::sleep(Duration::from_millis(50));

        let (packet, _) = receiver.recv_from().expect("the valid packet after the bad one");
        assert_eq!(packet, Packet::Ping { timestamp: 7 });
        assert!(receiver.recv_from().is_none(), "nothing else queued");
    }

    #[test]
    fn oversized_packet_is_refused_instead_of_sent_with_a_wrapped_length() {
        let sender = NetSocket::bind("127.0.0.1:0").expect("bind sender");
        let too_big = Packet::UserData { client_id: 1, data: vec![0; crate::packet::MAX_PAYLOAD_LEN] };
        let err = sender.send_to(&too_big, sender.local_addr()).unwrap_err();
        assert!(err.to_string().contains("at most"), "{err}");
    }
}
