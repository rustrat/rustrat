use crate::error::*;
use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

// Rustrat uses 256 bit keys for de/encryption
// The public and private keys are also 256 bits
pub type SharedKey = [u8; 32];
pub type PublicKey = [u8; 32];
pub type PrivateKey = [u8; 32];

pub fn get_shared_key(private_key: PrivateKey, public_key: PublicKey) -> SharedKey {
    let own_secret = x25519_dalek::StaticSecret::from(private_key);
    let their_public = x25519_dalek::PublicKey::from(public_key);

    own_secret.diffie_hellman(&their_public).to_bytes()
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Encrypted {
    nonce: [u8; 24],
    ciphertext: Vec<u8>,
}

impl Encrypted {
    pub fn decrypt(&self, shared_key: SharedKey) -> Result<Vec<u8>> {
        let key = Key::from_slice(&shared_key);
        let cipher = XChaCha20Poly1305::new(key);
        let nonce = XNonce::from_slice(&self.nonce);

        let plaintext = cipher.decrypt(nonce, self.ciphertext.as_ref())?;

        Ok(plaintext)
    }

    pub fn from_byte_array<T: AsRef<[u8]>, CR: rand::Rng + rand::CryptoRng>(
        shared_key: SharedKey,
        data: T,
        rng: &mut CR,
    ) -> Result<Encrypted> {
        let mut raw_nonce = [0u8; 24];
        rng.fill(&mut raw_nonce);

        let key = Key::from_slice(&shared_key);
        let nonce = XNonce::from_slice(&raw_nonce);
        let cipher = XChaCha20Poly1305::new(key);

        let ciphertext = cipher.encrypt(nonce, data.as_ref())?;

        Ok(Encrypted {
            nonce: raw_nonce,
            ciphertext: ciphertext,
        })
    }
}
