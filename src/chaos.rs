//! # Block Scrambling Utilities
//!
//! This module provides utilities for deterministically shuffling block order
//! based on a cryptographic key. This can be used to obscure the relationship
//! between logical file structure and physical archive layout.
//!
//! ## Current Status
//!
//! **This module provides shuffling utilities only.** The actual integration
//! with archive reading/writing (to create scrambled archives) is not
//! implemented. These functions can be used to generate permutation maps
//! for custom archive processing.
//!
//! ## Usage
//!
//! ```ignore
//! use sixcy::chaos::{generate_shuffled_map, invert_shuffled_map};
//!
//! let key = [0u8; 32]; // Key must be exactly 32 bytes
//! let n = 100; // Number of blocks
//!
//! // Generate a permutation: logical_index -> physical_index
//! let forward = generate_shuffled_map(n, &key);
//!
//! // Generate inverse: physical_index -> logical_index
//! let inverse = invert_shuffled_map(&forward);
//! ```
//!
//! ## Security Notes
//!
//! The shuffling uses ChaCha8 as a pseudorandom number generator seeded with
//! the provided key. This is deterministic — the same key always produces the
//! same permutation. The security of any obfuscation depends on:
//!
//! 1. Key secrecy
//! 2. Key randomness
//! 3. The block cipher underlying ChaCha8
//!
//! ## Limitations
//!
//! This is not encryption. The shuffled data can be recovered by anyone
//! who knows the key or who can identify the original format patterns.

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Generates a deterministic permutation for N items based on a 32-byte key.
///
/// Uses ChaCha8 seeded with the key to produce a cryptographically seeded
/// shuffle. The same key always produces the same output.
///
/// # Arguments
///
/// * `n` - Number of items to permute
/// * `key` - Exactly 32 bytes of key material
///
/// # Returns
///
/// A vector of indices `[0, 1, ..., n-1]` shuffled according to the key.
///
/// # Example
///
/// ```ignore
/// let key = derive_key_from_password(password);
/// let map = generate_shuffled_map(num_blocks, &key);
/// // map[i] gives the physical position of logical block i
/// ```
pub fn generate_shuffled_map(n: usize, key: &[u8; 32]) -> Vec<usize> {
    let mut map: Vec<usize> = (0..n).collect();
    let mut rng = ChaCha8Rng::from_seed(*key);
    map.shuffle(&mut rng);
    map
}

/// Inverts a permutation map to get the reverse mapping.
///
/// Given a forward permutation `map` where `map[i]` is the new position of
/// original item `i`, returns an inverse where `inverse[p]` is the original
/// position of item now at position `p`.
///
/// # Arguments
///
/// * `map` - Forward permutation from `generate_shuffled_map`
///
/// # Returns
///
/// An inverse permutation vector.
pub fn invert_shuffled_map(map: &[usize]) -> Vec<usize> {
    let mut inverse = vec![0; map.len()];
    for (logical, &physical) in map.iter().enumerate() {
        inverse[physical] = logical;
    }
    inverse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shuffle_is_deterministic() {
        let key = [42u8; 32];
        let a = generate_shuffled_map(100, &key);
        let b = generate_shuffled_map(100, &key);
        assert_eq!(a, b);
    }

    #[test]
    fn test_shuffle_is_permutation() {
        let key = [1u8; 32];
        let map = generate_shuffled_map(50, &key);

        // Check all values 0..49 are present exactly once
        let mut sorted = map.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn test_inverse_roundtrip() {
        let key = [99u8; 32];
        let n = 25;
        let forward = generate_shuffled_map(n, &key);
        let inverse = invert_shuffled_map(&forward);

        // Applying forward then inverse should return original indices
        for i in 0..n {
            assert_eq!(inverse[forward[i]], i);
        }
    }

    #[test]
    fn test_inverse_of_inverse() {
        let key = [7u8; 32];
        let n = 30;
        let forward = generate_shuffled_map(n, &key);
        let inverse = invert_shuffled_map(&forward);
        let reinverse = invert_shuffled_map(&inverse);
        assert_eq!(forward, reinverse);
    }
}
