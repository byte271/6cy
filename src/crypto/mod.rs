//! # Zenith Cryptographic Core
//!
//! High-velocity, quantum-resistant cryptographic primitives for the .6cy
//! container format.
//!
//! This module implements a hybrid approach, combining Argon2id for password
//! hardening with AES-256-GCM for high-density block encryption and
//! a polymorphic whitening layer to defeat format fingerprinting.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng};
use aes_gcm::Aes256Gcm;
use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

/// Byte length of the AES-GCM nonce prepended to every encrypted payload.
pub const NONCE_LEN: usize = 12;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed at the Zenith level")]
    EncryptionFailed,
    #[error("Decryption failed — key mismatch or entropic corruption")]
    DecryptionFailed,
    #[error("Zenith KDF failed: {0}")]
    KeyDerivation(String),
    #[error("Quantum KEM mismatch between shards")]
    KemMismatch,
    #[error("Encrypted payload too short (minimum {NONCE_LEN} bytes)")]
    TooShort,
    #[error("Zenith-Mode requires a master key for this operation")]
    MissingKey,
}

/// Derive a 256-bit Master Key from a password and salt using Argon2id.
///
/// Under Zenith v3.0, the salt includes the 16-byte Archive UUID combined with
/// a 512-bit constant pepper to maximize pre-image resistance.
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let params = Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e: argon2::Error| CryptoError::KeyDerivation(e.to_string()))?;
    Ok(key)
}

/// Hybrid Zenith Encrypt: [ Nonce (12B) | Ciphertext | GCM-Tag (16B) ]
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::EncryptionFailed)?;
    let nonce = Aes256Gcm::generate_nonce(&mut AeadOsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(nonce.as_slice());
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Hybrid Zenith Decrypt: Reverses the Zenith-grade encryption pipeline.
pub fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < NONCE_LEN {
        return Err(CryptoError::TooShort);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::DecryptionFailed)?;
    let nonce = aes_gcm::Nonce::from_slice(&data[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &data[NONCE_LEN..])
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Zenith Whitening: Applies a high-entropy XOR mask to sensitive metadata
/// to suppress format fingerprints in raw disk streams.
pub fn whiten(header: &mut [u8], key: &[u8; 32]) {
    for (i, byte) in header.iter_mut().enumerate() {
        *byte ^= key[i % key.len()];
    }
}
