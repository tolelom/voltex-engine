//! UDP server endpoint. Manages connected clients, dispatches `ServerEvent`s
//! to the caller, and forwards game packets between the wire and the
//! application.

use std::collections::HashMap;
use std::net::SocketAddr;

use crate::packet::Packet;
use crate::socket::NetSocket;

/// Information about a connected client.
pub struct ClientInfo {
    /// Server-assigned client id.
    pub id: u32,
    /// Peer socket address.
    pub addr: SocketAddr,
    /// Display name as reported in the `Connect` packet.
    pub name: String,
}

/// Events produced by the server during polling.
pub enum ServerEvent {
    /// A new client completed its handshake.
    ClientConnected {
        /// Newly assigned client id.
        client_id: u32,
        /// Self-reported display name from the `Connect` packet.
        name: String,
    },
    /// A client left (received Disconnect or timed out).
    ClientDisconnected {
        /// Id of the client that left.
        client_id: u32,
    },
    /// A non-control packet arrived from `client_id`.
    PacketReceived {
        /// Sender's client id.
        client_id: u32,
        /// Decoded packet payload.
        packet: Packet,
    },
}

/// A non-blocking UDP server that manages multiple clients.
pub struct NetServer {
    socket: NetSocket,
    clients: HashMap<u32, ClientInfo>,
    addr_to_id: HashMap<SocketAddr, u32>,
    next_id: u32,
}

impl NetServer {
    /// Bind the server to the given address.
    pub fn new(addr: &str) -> crate::Result<Self> {
        let socket = NetSocket::bind(addr)?;
        Ok(NetServer {
            socket,
            clients: HashMap::new(),
            addr_to_id: HashMap::new(),
            next_id: 1,
        })
    }

    /// Return the local address the server is listening on.
    pub fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr()
    }

    /// Poll for incoming packets and return any resulting server events.
    pub fn poll(&mut self) -> Vec<ServerEvent> {
        let mut events = Vec::new();

        while let Some((packet, addr)) = self.socket.recv_from() {
            match &packet {
                Packet::Connect { client_name } => {
                    // Assign a new id and send Accept
                    let id = self.next_id;
                    self.next_id += 1;
                    let name = client_name.clone();

                    let info = ClientInfo {
                        id,
                        addr,
                        name: name.clone(),
                    };
                    self.clients.insert(id, info);
                    self.addr_to_id.insert(addr, id);

                    let accept = Packet::Accept { client_id: id };
                    if let Err(e) = self.socket.send_to(&accept, addr) {
                        eprintln!("[NetServer] Failed to send Accept to {}: {}", addr, e);
                    }

                    events.push(ServerEvent::ClientConnected { client_id: id, name });
                }
                Packet::Disconnect { client_id } => {
                    // Only the client itself may end its session: the id in the
                    // packet must belong to the address that sent it. Otherwise any
                    // peer could disconnect someone else by guessing their id.
                    let id = *client_id;
                    let sent_by_owner = self.clients.get(&id).is_some_and(|info| info.addr == addr);
                    if sent_by_owner {
                        self.clients.remove(&id);
                        self.addr_to_id.remove(&addr);
                        events.push(ServerEvent::ClientDisconnected { client_id: id });
                    } else if self.clients.contains_key(&id) {
                        eprintln!(
                            "[NetServer] Ignored Disconnect for client {} from foreign addr {}",
                            id, addr
                        );
                    }
                }
                _ => {
                    // Map address to client id
                    if let Some(&client_id) = self.addr_to_id.get(&addr) {
                        events.push(ServerEvent::PacketReceived {
                            client_id,
                            packet: packet.clone(),
                        });
                    } else {
                        eprintln!("[NetServer] Packet from unknown addr {}", addr);
                    }
                }
            }
        }

        events
    }

    /// Send a packet to every connected client.
    pub fn broadcast(&self, packet: &Packet) {
        for info in self.clients.values() {
            if let Err(e) = self.socket.send_to(packet, info.addr) {
                eprintln!("[NetServer] broadcast failed for client {}: {}", info.id, e);
            }
        }
    }

    /// Send a packet to a specific client by id.
    pub fn send_to_client(&self, id: u32, packet: &Packet) {
        if let Some(info) = self.clients.get(&id) {
            if let Err(e) = self.socket.send_to(packet, info.addr) {
                eprintln!("[NetServer] send_to_client {} failed: {}", id, e);
            }
        }
    }

    /// Iterates over all connected clients.
    pub fn clients(&self) -> impl Iterator<Item = &ClientInfo> {
        self.clients.values()
    }

    /// Returns the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ClientEvent, NetClient};
    use std::time::Duration;

    #[test]
    fn test_integration_connect_and_userdata() {
        // Step 1: Start server on OS-assigned port
        let mut server = NetServer::new("127.0.0.1:0").expect("server bind failed");
        let server_addr = server.local_addr();

        // Step 2: Create client on OS-assigned port, point at server
        let mut client = NetClient::new("127.0.0.1:0", server_addr, "TestClient")
            .expect("client bind failed");

        // Step 3: Client sends Connect
        client.connect().expect("connect send failed");

        // Step 4: Give the packet time to travel
        std::thread::sleep(Duration::from_millis(50));

        // Step 5: Server poll → should get ClientConnected
        let server_events = server.poll();
        let mut connected_id = None;
        for event in &server_events {
            if let ServerEvent::ClientConnected { client_id, name } = event {
                connected_id = Some(*client_id);
                assert_eq!(name, "TestClient");
            }
        }
        assert!(connected_id.is_some(), "Server did not receive ClientConnected");

        // Step 6: Client poll → should get Connected
        std::thread::sleep(Duration::from_millis(50));
        let client_events = client.poll();
        let mut got_connected = false;
        for event in &client_events {
            if let ClientEvent::Connected { client_id } = event {
                assert_eq!(Some(*client_id), connected_id);
                got_connected = true;
            }
        }
        assert!(got_connected, "Client did not receive Connected event");

        // Step 7: Client sends UserData, server should receive it
        let cid = client.client_id().unwrap();
        let user_packet = Packet::UserData {
            client_id: cid,
            data: vec![1, 2, 3, 4],
        };
        client.send(user_packet.clone()).expect("send userdata failed");

        std::thread::sleep(Duration::from_millis(50));

        let server_events2 = server.poll();
        let mut got_packet = false;
        for event in server_events2 {
            if let ServerEvent::PacketReceived { client_id, packet } = event {
                assert_eq!(client_id, cid);
                assert_eq!(packet, user_packet);
                got_packet = true;
            }
        }
        assert!(got_packet, "Server did not receive UserData packet");

        // Cleanup: disconnect
        client.disconnect().expect("disconnect send failed");
    }

    /// Polls the server a few times so packets sent over loopback UDP arrive.
    fn poll_server(server: &mut NetServer) -> Vec<ServerEvent> {
        std::thread::sleep(Duration::from_millis(50));
        server.poll()
    }

    fn connect_client(server: &mut NetServer, name: &str) -> (NetClient, u32) {
        let mut client = NetClient::new("127.0.0.1:0", server.local_addr(), name)
            .expect("client bind failed");
        client.connect().expect("connect send failed");
        let client_id = poll_server(server)
            .into_iter()
            .find_map(|event| match event {
                ServerEvent::ClientConnected { client_id, .. } => Some(client_id),
                _ => None,
            })
            .expect("server did not accept the client");
        // Let the client read its Accept so it knows its id.
        std::thread::sleep(Duration::from_millis(50));
        client.poll();
        assert_eq!(client.client_id(), Some(client_id));
        (client, client_id)
    }

    #[test]
    fn disconnect_from_another_address_is_ignored() {
        let mut server = NetServer::new("127.0.0.1:0").expect("server bind failed");
        let (_victim, victim_id) = connect_client(&mut server, "Victim");

        // A different socket sends a Disconnect carrying the victim's id.
        let attacker = crate::socket::NetSocket::bind("127.0.0.1:0").expect("attacker bind failed");
        attacker
            .send_to(&Packet::Disconnect { client_id: victim_id }, server.local_addr())
            .expect("attacker send failed");

        let events = poll_server(&mut server);
        assert!(
            !events.iter().any(|e| matches!(e, ServerEvent::ClientDisconnected { .. })),
            "spoofed Disconnect must not remove the client"
        );
        assert!(server.clients().any(|c| c.id == victim_id), "victim must stay connected");
    }

    #[test]
    fn disconnect_from_the_client_itself_removes_it() {
        let mut server = NetServer::new("127.0.0.1:0").expect("server bind failed");
        let (mut client, client_id) = connect_client(&mut server, "Leaver");

        client.disconnect().expect("disconnect send failed");

        let events = poll_server(&mut server);
        assert!(events
            .iter()
            .any(|e| matches!(e, ServerEvent::ClientDisconnected { client_id: id } if *id == client_id)));
        assert!(!server.clients().any(|c| c.id == client_id));
    }
}
