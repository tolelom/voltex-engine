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
    pub fn bind(addr: &str) -> Result<Self, String> {
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

    /// Serialize and send a packet to the given address.
    pub fn send_to(&self, packet: &Packet, addr: SocketAddr) -> Result<(), String> {
        let bytes = packet.to_bytes();
        self.inner
            .send_to(&bytes, addr)
            .map_err(|e| format!("send_to failed: {}", e))?;
        Ok(())
    }

    /// Try to receive one packet. Returns None on WouldBlock (no data available).
    pub fn recv_from(&self) -> Option<(Packet, SocketAddr)> {
        let mut buf = vec![0u8; MAX_PACKET_SIZE];
        match self.inner.recv_from(&mut buf) {
            Ok((len, addr)) => {
                match Packet::from_bytes(&buf[..len]) {
                    Ok(packet) => Some((packet, addr)),
                    Err(e) => {
                        eprintln!("[NetSocket] Failed to parse packet from {}: {}", addr, e);
                        None
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(e) => {
                eprintln!("[NetSocket] recv_from error: {}", e);
                None
            }
        }
    }
}
