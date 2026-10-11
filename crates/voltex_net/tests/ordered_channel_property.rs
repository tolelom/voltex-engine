//! Randomized end-to-end property for `OrderedChannel` over a hostile
//! transport: packets and ACKs are dropped, duplicated and reordered, the
//! sender retransmits whatever is unacknowledged, and the u16 sequence space
//! wraps (70k messages). The receiver must deliver every message exactly
//! once, in send order, and the sender must end with nothing pending.

use std::collections::BTreeMap;
use voltex_net::OrderedChannel;

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn chance(&mut self, percent: u32) -> bool {
        self.next() % 100 < percent
    }
}

/// In-flight packets keyed by the tick they arrive on.
struct Network<T> {
    queue: BTreeMap<(u64, u64), T>,
    counter: u64,
}
impl<T: Clone> Network<T> {
    fn new() -> Self {
        Network { queue: BTreeMap::new(), counter: 0 }
    }
    fn send(&mut self, rng: &mut XorShift, now: u64, item: T, max_delay: u64) {
        if rng.chance(20) {
            return; // dropped
        }
        let copies = if rng.chance(10) { 2 } else { 1 };
        for _ in 0..copies {
            let delay = 1 + rng.next() as u64 % max_delay;
            self.counter += 1;
            self.queue.insert((now + delay, self.counter), item.clone());
        }
    }
    fn arrivals(&mut self, now: u64) -> Vec<T> {
        let later = self.queue.split_off(&(now + 1, 0));
        std::mem::replace(&mut self.queue, later).into_values().collect()
    }
}

fn run(seed: u32, messages: u32, max_delay: u64) {
    let mut rng = XorShift(seed);
    let mut sender = OrderedChannel::new();
    let mut receiver = OrderedChannel::new();
    let mut to_receiver: Network<(u16, Vec<u8>)> = Network::new();
    let mut to_sender: Network<u16> = Network::new();
    // Sender-side copy of unacked payloads, for retransmission.
    let mut unacked: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut delivered: Vec<u32> = Vec::new();
    let mut next_msg = 0u32;
    let mut now = 0u64;

    while delivered.len() < messages as usize {
        now += 1;
        assert!(now < 2_000_000, "seed {seed:x}: stalled at {} delivered", delivered.len());
        // Send up to 4 new messages per tick, keeping < 512 in flight.
        for _ in 0..4 {
            if next_msg < messages && unacked.len() < 512 {
                let (seq, wrapped) = sender.send(&next_msg.to_le_bytes());
                let payload = wrapped[2..].to_vec();
                unacked.insert(seq, payload.clone());
                to_receiver.send(&mut rng, now, (seq, payload), max_delay);
                next_msg += 1;
            }
        }
        // Retransmit every unacked message every 8 ticks.
        if now.is_multiple_of(8) {
            for (&seq, payload) in &unacked {
                to_receiver.send(&mut rng, now, (seq, payload.clone()), max_delay);
            }
        }
        for (seq, payload) in to_receiver.arrivals(now) {
            for msg in receiver.receive(seq, &payload) {
                delivered.push(u32::from_le_bytes(msg.try_into().unwrap()));
            }
        }
        for ack in receiver.reliable_mut().drain_acks() {
            to_sender.send(&mut rng, now, ack, max_delay);
        }
        for ack in to_sender.arrivals(now) {
            sender.reliable_mut().process_ack(ack);
            unacked.remove(&ack);
        }
    }
    let want: Vec<u32> = (0..messages).collect();
    if delivered != want {
        let first_bad = delivered.iter().zip(&want).position(|(a, b)| a != b);
        panic!("seed {seed:x}: delivery diverged at index {first_bad:?} (len {} vs {messages})", delivered.len());
    }
    // Let stragglers (late duplicates) land: nothing more may be delivered.
    for _ in 0..max_delay + 1 {
        now += 1;
        for (seq, payload) in to_receiver.arrivals(now) {
            let extra = receiver.receive(seq, &payload);
            assert!(extra.is_empty(), "seed {seed:x}: re-delivered a late duplicate (seq {seq})");
        }
        for ack in to_sender.arrivals(now) {
            sender.reliable_mut().process_ack(ack);
        }
    }
}

#[test]
fn ordered_channel_delivers_exactly_once_in_order_over_a_hostile_network() {
    for seed in [1u32, 0xBEEF, 0x5EED_1234] {
        run(seed, 70_000, 40);
    }
}

#[test]
fn very_late_duplicate_is_not_buffered_for_the_next_wrap() {
    // A duplicate of an already-delivered sequence that arrives after the
    // 1024-entry de-dup window has moved on used to be stored in the reorder
    // buffer. One u16 wrap later it was delivered in place of the real
    // message with that sequence number, which then sat in the buffer.
    let mut sender = OrderedChannel::new();
    let mut receiver = OrderedChannel::new();
    let mut packets = Vec::new();
    for i in 0u32..70_000 {
        let (seq, wrapped) = sender.send(&i.to_le_bytes());
        packets.push((seq, wrapped[2..].to_vec()));
    }
    let mut delivered = Vec::new();
    for (i, (seq, payload)) in packets.iter().enumerate() {
        delivered.extend(receiver.receive(*seq, payload));
        if i == 2000 {
            // Stale copy of message 5 shows up ~2000 sequences late.
            let (old_seq, old_payload) = &packets[5];
            assert!(receiver.receive(*old_seq, old_payload).is_empty());
        }
    }
    let got: Vec<u32> = delivered.into_iter().map(|m| u32::from_le_bytes(m.try_into().unwrap())).collect();
    let want: Vec<u32> = (0..70_000).collect();
    let first_bad = got.iter().zip(&want).position(|(a, b)| a != b);
    assert!(got == want, "diverged at {first_bad:?} (len {})", got.len());
}
