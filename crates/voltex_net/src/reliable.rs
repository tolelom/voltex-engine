//! Reliable + ordered channels built on top of UDP packets. Adds sequence
//! numbers, ACKs, retransmission and (optionally) in-order delivery.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// A channel that provides reliable delivery over unreliable transport.
///
/// Assigns sequence numbers, tracks ACKs, estimates RTT,
/// and retransmits unacknowledged packets after 2x RTT.
pub struct ReliableChannel {
    next_sequence: u16,
    pending_acks: HashMap<u16, (Instant, Vec<u8>)>,
    received_seqs: HashSet<u16>,
    rtt: Duration,
    /// Outgoing ACK packets that need to be sent by the caller.
    outgoing_acks: Vec<u16>,
}

impl Default for ReliableChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl ReliableChannel {
    /// Creates an empty channel with an initial RTT estimate of 100ms.
    pub fn new() -> Self {
        ReliableChannel {
            next_sequence: 0,
            pending_acks: HashMap::new(),
            received_seqs: HashSet::new(),
            rtt: Duration::from_millis(100), // initial estimate
            outgoing_acks: Vec::new(),
        }
    }

    /// Returns the current RTT estimate.
    pub fn rtt(&self) -> Duration {
        self.rtt
    }

    /// Returns the number of packets awaiting acknowledgement.
    pub fn pending_count(&self) -> usize {
        self.pending_acks.len()
    }

    /// Prepare a reliable send. Returns (sequence_number, wrapped_data).
    /// The caller is responsible for actually transmitting the wrapped data.
    pub fn send_reliable(&mut self, data: &[u8]) -> (u16, Vec<u8>) {
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);

        // Build the reliable packet payload: [seq(2 LE), data...]
        let mut buf = Vec::with_capacity(2 + data.len());
        buf.extend_from_slice(&seq.to_le_bytes());
        buf.extend_from_slice(data);

        self.pending_acks.insert(seq, (Instant::now(), buf.clone()));

        (seq, buf)
    }

    /// Process a received reliable packet. Returns the payload data if this is
    /// not a duplicate, or None if already received. Queues an ACK to send.
    pub fn receive_and_ack(&mut self, sequence: u16, data: &[u8]) -> Option<Vec<u8>> {
        // Always queue an ACK, even for duplicates
        self.outgoing_acks.push(sequence);

        if self.received_seqs.contains(&sequence) {
            return None; // duplicate
        }

        self.received_seqs.insert(sequence);
        Some(data.to_vec())
    }

    /// Process an incoming ACK for a sequence we sent.
    pub fn process_ack(&mut self, sequence: u16) {
        if let Some((send_time, _)) = self.pending_acks.remove(&sequence) {
            let sample = send_time.elapsed();
            // Exponential moving average: rtt = 0.875 * rtt + 0.125 * sample
            self.rtt = Duration::from_secs_f64(
                0.875 * self.rtt.as_secs_f64() + 0.125 * sample.as_secs_f64(),
            );
        }
    }

    /// Drain any pending outgoing ACK sequence numbers.
    pub fn drain_acks(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.outgoing_acks)
    }

    /// Check for timed-out packets and return their data for retransmission.
    /// Resets the send_time for retransmitted packets.
    pub fn update(&mut self) -> Vec<Vec<u8>> {
        let timeout = self.rtt * 2;
        let now = Instant::now();
        let mut retransmits = Vec::new();

        for (_, (send_time, data)) in self.pending_acks.iter_mut() {
            if now.duration_since(*send_time) >= timeout {
                retransmits.push(data.clone());
                *send_time = now;
            }
        }

        retransmits
    }
}

/// A channel that delivers packets in order, built on top of ReliableChannel.
pub struct OrderedChannel {
    reliable: ReliableChannel,
    next_deliver: u16,
    buffer: HashMap<u16, Vec<u8>>,
}

impl Default for OrderedChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderedChannel {
    /// Creates an empty ordered channel (next deliver = 0).
    pub fn new() -> Self {
        OrderedChannel {
            reliable: ReliableChannel::new(),
            next_deliver: 0,
            buffer: HashMap::new(),
        }
    }

    /// Access the underlying reliable channel (e.g., for send_reliable, process_ack, update).
    pub fn reliable(&self) -> &ReliableChannel {
        &self.reliable
    }

    /// Access the underlying reliable channel mutably.
    pub fn reliable_mut(&mut self) -> &mut ReliableChannel {
        &mut self.reliable
    }

    /// Prepare a reliable, ordered send.
    pub fn send(&mut self, data: &[u8]) -> (u16, Vec<u8>) {
        self.reliable.send_reliable(data)
    }

    /// Receive a packet. Buffers out-of-order packets and returns all
    /// packets that can now be delivered in sequence order.
    pub fn receive(&mut self, sequence: u16, data: &[u8]) -> Vec<Vec<u8>> {
        let payload = self.reliable.receive_and_ack(sequence, data);

        if let Some(payload) = payload {
            self.buffer.insert(sequence, payload);
        }

        // Deliver as many consecutive packets as possible
        let mut delivered = Vec::new();
        while let Some(data) = self.buffer.remove(&self.next_deliver) {
            delivered.push(data);
            self.next_deliver = self.next_deliver.wrapping_add(1);
        }

        delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ReliableChannel tests ----

    #[test]
    fn test_send_receive_ack_roundtrip() {
        let mut sender = ReliableChannel::new();
        let mut receiver = ReliableChannel::new();

        let original = b"hello world";
        let (seq, _buf) = sender.send_reliable(original);
        assert_eq!(seq, 0);
        assert_eq!(sender.pending_count(), 1);

        // Receiver gets the packet
        let result = receiver.receive_and_ack(seq, original);
        assert_eq!(result, Some(original.to_vec()));

        // Receiver queued an ack
        let acks = receiver.drain_acks();
        assert_eq!(acks, vec![0]);

        // Sender processes the ack
        sender.process_ack(seq);
        assert_eq!(sender.pending_count(), 0);
    }

    #[test]
    fn test_duplicate_rejection() {
        let mut receiver = ReliableChannel::new();

        let data = b"payload";
        let result1 = receiver.receive_and_ack(0, data);
        assert!(result1.is_some());

        let result2 = receiver.receive_and_ack(0, data);
        assert!(result2.is_none(), "Duplicate should be rejected");

        // But ACK is still queued for both
        let acks = receiver.drain_acks();
        assert_eq!(acks.len(), 2);
    }

    #[test]
    fn test_sequence_numbers_increment() {
        let mut channel = ReliableChannel::new();

        let (s0, _) = channel.send_reliable(b"a");
        let (s1, _) = channel.send_reliable(b"b");
        let (s2, _) = channel.send_reliable(b"c");

        assert_eq!(s0, 0);
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert_eq!(channel.pending_count(), 3);
    }

    #[test]
    fn test_retransmission_on_timeout() {
        let mut channel = ReliableChannel::new();
        // Set a very short RTT so timeout (2*RTT) triggers quickly
        channel.rtt = Duration::from_millis(1);

        let (_seq, _buf) = channel.send_reliable(b"data");
        assert_eq!(channel.pending_count(), 1);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(10));

        let retransmits = channel.update();
        assert_eq!(retransmits.len(), 1, "Should retransmit 1 packet");
        // Packet is still pending (not acked)
        assert_eq!(channel.pending_count(), 1);
    }

    #[test]
    fn test_no_retransmission_before_timeout() {
        let mut channel = ReliableChannel::new();
        // Default RTT = 100ms, so timeout = 200ms
        let (_seq, _buf) = channel.send_reliable(b"data");

        // Immediately check — should not retransmit
        let retransmits = channel.update();
        assert!(retransmits.is_empty());
    }

    #[test]
    fn test_rtt_estimation() {
        let mut channel = ReliableChannel::new();
        let initial_rtt = channel.rtt();

        let (seq, _) = channel.send_reliable(b"x");
        std::thread::sleep(Duration::from_millis(5));
        channel.process_ack(seq);

        // RTT should have changed from initial value
        let new_rtt = channel.rtt();
        assert_ne!(initial_rtt, new_rtt, "RTT should be updated after ACK");
    }

    #[test]
    fn test_wrapping_sequence() {
        let mut channel = ReliableChannel::new();
        channel.next_sequence = u16::MAX;

        let (s1, _) = channel.send_reliable(b"a");
        assert_eq!(s1, u16::MAX);

        let (s2, _) = channel.send_reliable(b"b");
        assert_eq!(s2, 0); // wrapped
    }

    // ---- OrderedChannel tests ----

    #[test]
    fn test_ordered_in_order_delivery() {
        let mut channel = OrderedChannel::new();

        let delivered0 = channel.receive(0, b"first");
        assert_eq!(delivered0, vec![b"first".to_vec()]);

        let delivered1 = channel.receive(1, b"second");
        assert_eq!(delivered1, vec![b"second".to_vec()]);
    }

    #[test]
    fn test_ordered_out_of_order_delivery() {
        let mut channel = OrderedChannel::new();

        // Receive seq 1 first (out of order)
        let delivered = channel.receive(1, b"second");
        assert!(delivered.is_empty(), "Seq 1 should be buffered, waiting for 0");

        // Receive seq 2 (still missing 0)
        let delivered = channel.receive(2, b"third");
        assert!(delivered.is_empty());

        // Receive seq 0 — should deliver 0, 1, 2 in order
        let delivered = channel.receive(0, b"first");
        assert_eq!(delivered.len(), 3);
        assert_eq!(delivered[0], b"first");
        assert_eq!(delivered[1], b"second");
        assert_eq!(delivered[2], b"third");
    }

    #[test]
    fn test_ordered_gap_handling() {
        let mut channel = OrderedChannel::new();

        // Deliver 0
        let d = channel.receive(0, b"a");
        assert_eq!(d.len(), 1);

        // Skip 1, deliver 2
        let d = channel.receive(2, b"c");
        assert!(d.is_empty(), "Can't deliver 2 without 1");

        // Now deliver 1 — should flush both 1 and 2
        let d = channel.receive(1, b"b");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0], b"b");
        assert_eq!(d[1], b"c");
    }
}
