//! AES-256-GCM encryption and Argon2id key derivation for .6cy archives.
//!
//! Key derivation: Argon2id(password, salt=archive_uuid_bytes) → 32-byte key
//! Encryption:     AES-256-GCM, nonce prepended to ciphertext
//!
//! Encrypted payload layout: [ nonce (12 B) | ciphertext | GCM tag (16 B) ]

use argon2::{Argon2, Algorithm, Version, Params};
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng};
use aes_gcm::Aes256Gcm;
use thiserror::Error;

/// Byte length of the AES-GCM nonce prepended to every encrypted payload.
pub const NONCE_LEN: usize = 12;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed — wrong password or corrupted data")]
    DecryptionFailed,
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("Encrypted payload too short (minimum {NONCE_LEN} bytes)")]
    TooShort,
    #[error("Block is encrypted but no decryption key was provided")]
    MissingKey,
}

/// Derive a 256-bit encryption key from a password and a salt using Argon2id.
///
/// `salt` should be the 16-byte archive UUID, giving each archive a unique key
/// even when the same password is reused across archives.
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    // Use Argon2id with conservative parameters suitable for archive encryption.
    let params = Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    Ok(key)
}

/// Encrypt `plaintext` with AES-256-GCM using a random nonce.
///
/// Returns `nonce (12 B) || ciphertext || GCM-tag (16 B)`.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| CryptoError::EncryptionFailed)?;
    let nonce = Aes256Gcm::generate_nonce(&mut AeadOsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(nonce.as_slice());
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt an AES-256-GCM payload produced by [`encrypt`].
///
/// Input must start with the 12-byte nonce followed by ciphertext + GCM tag.
pub fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < NONCE_LEN {
        return Err(CryptoError::TooShort);
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| CryptoError::DecryptionFailed)?;
    let nonce = aes_gcm::Nonce::from_slice(&data[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &data[NONCE_LEN..])
        .map_err(|_| CryptoError::DecryptionFailed)
}
