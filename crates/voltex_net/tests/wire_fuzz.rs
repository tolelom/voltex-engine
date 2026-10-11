//! Wire-format fuzz: every byte the server/client reads comes from the
//! network. `Packet::from_bytes` and `PacketCipher::decrypt` must return
//! `Err` (or a packet that re-encodes to the same bytes) on arbitrary and
//! mutated input — never panic.

use std::panic::{catch_unwind, AssertUnwindSafe};
use voltex_net::{Packet, PacketCipher};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn samples() -> Vec<Packet> {
    vec![
        Packet::Connect { client_name: "player é 1".into() },
        Packet::Accept { client_id: 7 },
        Packet::Disconnect { client_id: 7 },
        Packet::Ping { timestamp: 123_456_789 },
        Packet::Pong { timestamp: 987_654_321 },
        Packet::UserData { client_id: 3, data: (0..40).collect() },
        Packet::Reliable { sequence: 65_535, data: vec![1, 2, 3] },
        Packet::Ack { sequence: 9 },
        Packet::Snapshot { tick: 42, data: vec![0xAB; 100] },
    ]
}

#[test]
fn packet_decoder_never_panics_and_accepted_packets_round_trip() {
    let mut rng = XorShift(0x9A7E_0001);
    let encoded: Vec<Vec<u8>> = samples().iter().map(Packet::to_bytes).collect();
    for i in 0..30_000 {
        let mut b = encoded[i % encoded.len()].clone();
        match rng.next() % 3 {
            0 => {
                for _ in 0..1 + rng.next() % 4 {
                    let at = rng.next() as usize % b.len();
                    b[at] = rng.next() as u8;
                }
            }
            1 => b.truncate(rng.next() as usize % b.len()),
            _ => b = (0..rng.next() % 64).map(|_| rng.next() as u8).collect(),
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(p) = Packet::from_bytes(&b) {
                let again = Packet::from_bytes(&p.to_bytes()).expect("re-encoded packet decodes");
                assert_eq!(again.to_bytes(), p.to_bytes(), "unstable re-encode");
            }
        }));
        assert!(outcome.is_ok(), "mutation {i} panicked Packet::from_bytes ({b:?})");
    }
}

#[test]
fn cipher_decrypt_never_panics_on_tampered_ciphertext() {
    let mut rng = XorShift(0xC1F3_0002);
    let mut sender = PacketCipher::new(b"shared secret key");
    for i in 0..20_000 {
        let plain: Vec<u8> = (0..rng.next() % 80).map(|_| rng.next() as u8).collect();
        let mut ct = sender.encrypt(&plain);
        match rng.next() % 3 {
            0 if !ct.is_empty() => {
                let at = rng.next() as usize % ct.len();
                ct[at] ^= 1 << (rng.next() % 8);
            }
            1 => ct.truncate(rng.next() as usize % (ct.len() + 1)),
            _ => ct = (0..rng.next() % 40).map(|_| rng.next() as u8).collect(),
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut receiver = PacketCipher::new(b"shared secret key");
            let _ = receiver.decrypt(&ct);
        }));
        assert!(outcome.is_ok(), "case {i} panicked PacketCipher::decrypt");
    }
}
