use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

/// A one-time keypair for this session.
/// The secret is consumed during key exchange — can never be reused.
pub struct SessionKeypair {
    secret: EphemeralSecret,
    pub public: PublicKey,
}

impl SessionKeypair {
    /// Generate a fresh keypair for this session.
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        SessionKeypair { secret, public }
    }

    /// Derive a shared secret from our secret and the peer's public key.
    /// Both sides independently compute the same secret — server never sees it.
    pub fn derive_shared_secret(self, peer_public: PublicKey) -> SessionCipher {
        let shared = self.secret.diffie_hellman(&peer_public);
        SessionCipher::from_shared_secret(shared)
    }

    /// Return public key as bytes for sending over the wire.
    pub fn public_bytes(&self) -> [u8; 32] {
        self.public.to_bytes()
    }
}

/// A symmetric cipher derived from the shared secret.
/// Used to encrypt/decrypt all relay traffic.
pub struct SessionCipher {
    cipher: ChaCha20Poly1305,
}

impl SessionCipher {
    fn from_shared_secret(secret: SharedSecret) -> Self {
        // Use the raw shared secret bytes as the ChaCha20 key.
        // Both peers derive the same key independently.
        let cipher = ChaCha20Poly1305::new_from_slice(secret.as_bytes())
            .expect("SharedSecret is always 32 bytes");
        SessionCipher { cipher }
    }

    /// Encrypt a plaintext message.
    /// Returns nonce + ciphertext — nonce is needed for decryption.
    #[allow(dead_code)]
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Prepend the 12-byte nonce to the ciphertext
        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    /// Decrypt a message produced by encrypt().
    /// Expects nonce prepended to ciphertext.
    pub fn decrypt(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if data.len() < 12 {
            anyhow::bail!("Message too short to contain nonce");
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed — message may be tampered"))
    }
}
