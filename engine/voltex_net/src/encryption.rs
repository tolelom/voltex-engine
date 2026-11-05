//! Minimal symmetric "encryption" primitives — XOR + sequence counter, no
//! real cryptographic guarantees. Suitable for casual tamper resistance and
//! anti-replay during development. Replace with a real AEAD before relying on
//! it for security.

/// Simple XOR cipher with rotating key + sequence counter.
pub struct PacketCipher {
    key: Vec<u8>,
    send_counter: u64,
    recv_counter: u64,
}

impl PacketCipher {
    /// Creates a new cipher seeded with `key`. Panics if `key` is empty.
    pub fn new(key: &[u8]) -> Self {
        assert!(!key.is_empty(), "encryption key must not be empty");
        PacketCipher { key: key.to_vec(), send_counter: 0, recv_counter: 0 }
    }

    /// Encrypt `plaintext` into a new buffer: 8-byte sequence number + XORed bytes.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Vec<u8> {
        let mut output = Vec::with_capacity(8 + plaintext.len());
        // Prepend sequence counter
        output.extend_from_slice(&self.send_counter.to_le_bytes());
        // XOR plaintext with key derived from counter + base key
        let derived = self.derive_key(self.send_counter);
        for (i, &byte) in plaintext.iter().enumerate() {
            output.push(byte ^ derived[i % derived.len()]);
        }
        self.send_counter += 1;
        output
    }

    /// Decrypt data. Validates sequence number.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> crate::Result<Vec<u8>> {
        if ciphertext.len() < 8 {
            return Err("packet too short".into());
        }
        let seq = u64::from_le_bytes(ciphertext[0..8].try_into().unwrap());

        // Anti-replay: sequence must be >= expected
        if seq < self.recv_counter {
            return Err(format!("replay detected: got seq {}, expected >= {}", seq, self.recv_counter).into());
        }
        // Adversary-supplied `seq = u64::MAX` used to wrap recv_counter to 0
        // via `seq + 1`, which (debug) panicked or (release) caused every
        // subsequent legitimate packet to be flagged as a replay. Saturating
        // add keeps the counter pinned at MAX — strictly safer and the
        // protocol is already terminated at that point.
        self.recv_counter = seq.saturating_add(1);

        let derived = self.derive_key(seq);
        let mut plaintext = Vec::with_capacity(ciphertext.len() - 8);
        for (i, &byte) in ciphertext[8..].iter().enumerate() {
            plaintext.push(byte ^ derived[i % derived.len()]);
        }
        Ok(plaintext)
    }

    /// Derive a key from the base key + counter.
    fn derive_key(&self, counter: u64) -> Vec<u8> {
        let counter_bytes = counter.to_le_bytes();
        self.key.iter().enumerate().map(|(i, &k)| {
            k.wrapping_add(counter_bytes[i % 8])
        }).collect()
    }
}

/// Simple token-based authentication.
pub struct AuthToken {
    /// Player identifier the token is issued to.
    pub player_id: u32,
    /// Opaque token bytes (XOR-keyed payload).
    pub token: Vec<u8>,
    /// Unix-seconds expiry timestamp.
    pub expires_at: f64,
}

impl AuthToken {
    /// Generate a simple auth token from player_id + secret. Panics if
    /// `secret` is empty (same contract as [`PacketCipher::new`]).
    pub fn generate(player_id: u32, secret: &[u8], expires_at: f64) -> Self {
        assert!(!secret.is_empty(), "auth secret must not be empty");
        let mut token = Vec::new();
        token.extend_from_slice(&player_id.to_le_bytes());
        token.extend_from_slice(&expires_at.to_le_bytes());
        // Simple HMAC-like: XOR with secret
        for (i, byte) in token.iter_mut().enumerate() {
            *byte ^= secret[i % secret.len()];
        }
        AuthToken { player_id, token, expires_at }
    }

    /// Validate token against secret.
    ///
    /// Returns `false` if the token has expired, if `current_time` or
    /// `expires_at` is NaN (NaN comparisons always return `false`, so a
    /// NaN `current_time` used to slip past the expiry check), or if the
    /// HMAC bytes don't match. An empty `secret` is rejected rather than
    /// panicking. Token bytes are compared in constant time.
    pub fn validate(&self, secret: &[u8], current_time: f64) -> bool {
        if secret.is_empty() {
            return false;
        }
        if !current_time.is_finite() || !self.expires_at.is_finite() {
            return false;
        }
        if current_time > self.expires_at { return false; }
        let expected = AuthToken::generate(self.player_id, secret, self.expires_at);
        constant_time_eq(&self.token, &expected.token)
    }
}

/// Slice equality whose running time does not depend on where the first
/// differing byte is (lengths are not secret and compared directly).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = b"secret_key_1234";
        let mut encryptor = PacketCipher::new(key);
        let mut decryptor = PacketCipher::new(key);
        let msg = b"hello world";
        let encrypted = encryptor.encrypt(msg);
        let decrypted = decryptor.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, msg);
    }

    #[test]
    fn test_encrypted_differs_from_plain() {
        let mut cipher = PacketCipher::new(b"key");
        let msg = b"test message";
        let encrypted = cipher.encrypt(msg);
        assert_ne!(&encrypted[8..], msg); // ciphertext differs
    }

    #[test]
    fn test_replay_rejected() {
        let key = b"key";
        let mut enc = PacketCipher::new(key);
        let mut dec = PacketCipher::new(key);
        let pkt1 = enc.encrypt(b"first");
        let pkt2 = enc.encrypt(b"second");
        let _ = dec.decrypt(&pkt2).unwrap(); // accept pkt2 (seq=1)
        let result = dec.decrypt(&pkt1); // pkt1 has seq=0 < expected 2
        assert!(result.is_err());
    }

    #[test]
    fn test_auth_token_valid() {
        let secret = b"server_secret";
        let token = AuthToken::generate(42, secret, 1000.0);
        assert!(token.validate(secret, 999.0));
    }

    #[test]
    fn test_auth_token_expired() {
        let secret = b"server_secret";
        let token = AuthToken::generate(42, secret, 1000.0);
        assert!(!token.validate(secret, 1001.0));
    }

    #[test]
    fn test_auth_token_wrong_secret() {
        let token = AuthToken::generate(42, b"correct", 1000.0);
        assert!(!token.validate(b"wronggg", 999.0));
    }

    #[test]
    fn decrypt_with_max_u64_seq_does_not_panic_or_wrap() {
        // Pre-fix, an adversarial packet header of [0xFF; 8] (seq = u64::MAX)
        // caused recv_counter = seq + 1 to wrap to 0 (debug: panic, release:
        // future legitimate packets rejected as replays).
        let key = b"key";
        let mut dec = PacketCipher::new(key);
        let mut fake = vec![0xFFu8; 8]; // seq = u64::MAX
        fake.extend_from_slice(&[0; 4]); // dummy ciphertext body
        let _ = dec.decrypt(&fake);
        // recv_counter must be pinned at u64::MAX, not wrapped to 0.
        assert_eq!(dec.recv_counter, u64::MAX);
    }

    #[test]
    fn auth_token_rejects_nan_current_time() {
        // Pre-fix, NaN > expires_at is false (all NaN comparisons are false),
        // so a NaN current_time slipped past the expiry check.
        let secret = b"secret";
        let token = AuthToken::generate(42, secret, 100.0);
        assert!(!token.validate(secret, f64::NAN));
        assert!(!token.validate(secret, f64::INFINITY));
        // Token with NaN expiry should also fail.
        let bad_token = AuthToken { player_id: 1, token: vec![1, 2], expires_at: f64::NAN };
        assert!(!bad_token.validate(secret, 0.0));
    }

    #[test]
    fn constant_time_eq_matches_slice_equality() {
        assert!(constant_time_eq(b"abcd", b"abcd"));
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"abcd", b"abce"), "last byte differs");
        assert!(!constant_time_eq(b"abcd", b"xbcd"), "first byte differs");
        assert!(!constant_time_eq(b"abc", b"abcd"), "length differs");
    }

    #[test]
    fn validate_rejects_tampered_token_bytes() {
        let secret = b"s3cret";
        let mut token = AuthToken::generate(9, secret, 100.0);
        let last = token.token.len() - 1;
        token.token[last] ^= 0x01;
        assert!(!token.validate(secret, 0.0));
    }

    #[test]
    fn validate_with_empty_secret_is_false_not_a_panic() {
        let token = AuthToken::generate(1, b"k", 100.0);
        assert!(!token.validate(b"", 0.0));
    }

    #[test]
    #[should_panic(expected = "auth secret must not be empty")]
    fn generate_with_empty_secret_panics_with_a_clear_message() {
        let _ = AuthToken::generate(1, b"", 100.0);
    }
}
