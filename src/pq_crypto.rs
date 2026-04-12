//! # Post-Quantum Cryptography Interface
//!
//! This module provides interface definitions for post-quantum key encapsulation
//! mechanisms (KEM). The goal is to allow future integration of NIST-standardized
//! algorithms such as Kyber once they are stable and widely available in Rust.
//!
//! ## Current Status
//!
//! **This module is a placeholder.** The functions return `Unsupported` errors.
//! No actual post-quantum cryptography is implemented in this release.
//!
//! ## Planned Integration
//!
//! When implemented, this module will provide hybrid key encapsulation combining:
//! - Classical: X25519 or AES-256-GCM (for compatibility)
//! - Post-quantum: Kyber-768 (CRYSTALS-Kyber)
//!
//! The hybrid approach ensures security against both classical and quantum attacks
//! while maintaining compatibility with systems that only support classical crypto.
//!
//! ## References
//!
//! - [NIST Post-Quantum Cryptography Standardization](https://csrc.nist.gov/projects/post-quantum-cryptography)
//! - [CRYSTALS-Kyber](https://pq-crystals.org/kyber/)

use std::io;

/// KEM identity string for display purposes.
pub const KEM_ID: &str = "KYBER-768-AES256";

/// Placeholder struct for post-quantum KEM operations.
///
/// # Current Status
///
/// All methods return `Err(io::ErrorKind::Unsupported)`.
pub struct Kem;

/// Oort-Kem placeholder for future NIST standard integration.
impl Kem {
    /// Generates a new hybrid keypair.
    ///
    /// # Returns
    ///
    /// Returns `Err(io::ErrorKind::Unsupported)` - not implemented.
    pub fn new_keypair() -> io::Result<(Vec<u8>, Vec<u8>)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Post-quantum KEM not yet implemented",
        ))
    }

    /// Encapsulates a shared secret for a given public key.
    ///
    /// # Returns
    ///
    /// Returns `Err(io::ErrorKind::Unsupported)` - not implemented.
    pub fn encapsulate(public_key: &[u8]) -> io::Result<(Vec<u8>, Vec<u8>)> {
        let _ = public_key;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Post-quantum KEM not yet implemented",
        ))
    }

    /// Decapsulates a shared secret using a private key and ciphertext.
    ///
    /// # Returns
    ///
    /// Returns `Err(io::ErrorKind::Unsupported)` - not implemented.
    pub fn decapsulate(private_key: &[u8], ciphertext: &[u8]) -> io::Result<Vec<u8>> {
        let _ = (private_key, ciphertext);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Post-quantum KEM not yet implemented",
        ))
    }
}
