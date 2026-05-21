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

    /// Encrypt data in-place. Prepends 8-byte sequence number.
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
        self.recv_counter = seq + 1;

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
    /// Generate a simple auth token from player_id + secret.
    pub fn generate(player_id: u32, secret: &[u8], expires_at: f64) -> Self {
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
    pub fn validate(&self, secret: &[u8], current_time: f64) -> bool {
        if current_time > self.expires_at { return false; }
        let expected = AuthToken::generate(self.player_id, secret, self.expires_at);
        self.token == expected.token
    }
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
}
