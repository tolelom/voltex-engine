use std::net::SocketAddr;

use crate::packet::Packet;
use crate::socket::NetSocket;

/// Events produced by the client during polling.
pub enum ClientEvent {
    Connected { client_id: u32 },
    Disconnected,
    PacketReceived { packet: Packet },
}

/// A non-blocking UDP client.
pub struct NetClient {
    socket: NetSocket,
    server_addr: SocketAddr,
    client_id: Option<u32>,
    name: String,
}

impl NetClient {
    /// Create and bind a new client socket.
    ///
    /// - `local_addr`: local bind address (e.g. "127.0.0.1:0")
    /// - `server_addr`: the server's `SocketAddr`
    /// - `name`: client display name used in Connect packet
    pub fn new(local_addr: &str, server_addr: SocketAddr, name: &str) -> Result<Self, String> {
        let socket = NetSocket::bind(local_addr)?;
        Ok(NetClient {
            socket,
            server_addr,
            client_id: None,
            name: name.to_string(),
        })
    }

    /// Send a Connect packet to the server.
    pub fn connect(&self) -> Result<(), String> {
        let packet = Packet::Connect {
            client_name: self.name.clone(),
        };
        self.socket.send_to(&packet, self.server_addr)
    }

    /// Poll for incoming packets and return any resulting client events.
    pub fn poll(&mut self) -> Vec<ClientEvent> {
        let mut events = Vec::new();

        while let Some((packet, _addr)) = self.socket.recv_from() {
            match &packet {
                Packet::Accept { client_id } => {
                    self.client_id = Some(*client_id);
                    events.push(ClientEvent::Connected {
                        client_id: *client_id,
                    });
                }
                Packet::Disconnect { .. } => {
                    self.client_id = None;
                    events.push(ClientEvent::Disconnected);
                }
                _ => {
                    events.push(ClientEvent::PacketReceived {
                        packet: packet.clone(),
                    });
                }
            }
        }

        events
    }

    /// Send an arbitrary packet to the server.
    pub fn send(&self, packet: Packet) -> Result<(), String> {
        self.socket.send_to(&packet, self.server_addr)
    }

    /// Returns true if the client has received an Accept from the server.
    pub fn is_connected(&self) -> bool {
        self.client_id.is_some()
    }

    /// Returns the client id assigned by the server, or None if not yet connected.
    pub fn client_id(&self) -> Option<u32> {
        self.client_id
    }

    /// Send a Disconnect packet to the server and clear local state.
    pub fn disconnect(&mut self) -> Result<(), String> {
        if let Some(id) = self.client_id {
            let packet = Packet::Disconnect { client_id: id };
            self.socket.send_to(&packet, self.server_addr)?;
            self.client_id = None;
        }
        Ok(())
    }
}
