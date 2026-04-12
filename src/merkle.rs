//! # Verifiable Integrity Layer: Merkle Proofs
//!
//! Implements a Binary Merkle Tree over per-block BLAKE3 content hashes.
//! The Merkle root, stored in the Zenith Anchor, allows for sub-linear
//! verifiable inclusion proofs and zero-trust integrity audits.
//!
//! # Structure
//!
//! Leaf nodes are the `content_hash` values from each DATA block, in the
//! order they appear in the archive.  Internal nodes are computed as:
//!
//! ```text
//! node = BLAKE3(b"\x01" || left_child || right_child)
//! ```
//!
//! Leaf nodes are prefixed with `b"\x00"` to prevent second-preimage attacks
//! (see RFC 6962 §2.1):
//!
//! ```text
//! leaf = BLAKE3(b"\x00" || content_hash)
//! ```
//!
//! An odd number of leaves at any level is handled by promoting the unpaired
//! node up unchanged (no duplication of the last node).
//!
//! # Root hash
//!
//! The `root` field replaces the flat `root_hash` in `FileIndex` for archives
//! that use the Merkle tree.  The values are *not* interchangeable: a flat
//! hash-of-hashes and a Merkle root of the same leaf set produce different
//! digests.  Archives without FEC continue to use the flat root_hash for
//! backward compatibility; FEC-enabled archives use the Merkle root.
//!
//! # Proofs
//!
//! A [`MerkleProof`] proves that a given leaf is included in the tree at a
//! specific index.  Verification requires only the leaf hash, the index, the
//! total leaf count, and the proof path — not the full tree.
//!
//! ```rust,ignore
//! let tree  = MerkleTree::build(&leaf_hashes);
//! let proof = tree.proof(leaf_index);
//! assert!(proof.verify(&leaf_hashes[leaf_index], leaf_index, tree.leaf_count(), &tree.root()));
//! ```

use serde::{Deserialize, Serialize};

// ── Domain-separation prefixes (RFC 6962) ────────────────────────────────────

const LEAF_PREFIX: u8 = 0x00;
const INNER_PREFIX: u8 = 0x01;

// ── Hashing helpers ───────────────────────────────────────────────────────────

#[inline]
fn hash_leaf(content_hash: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(&[LEAF_PREFIX]);
    h.update(content_hash);
    h.finalize().into()
}

#[inline]
fn hash_inner(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(&[INNER_PREFIX]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

// ── MerkleTree ────────────────────────────────────────────────────────────────

/// A complete binary Merkle tree built over an ordered sequence of 32-byte
/// leaf hashes.
///
/// All levels of the tree are stored in `levels[0]` (leaves, post-prefix
/// hash) through `levels[depth]` (root).  This allows O(log n) proof
/// generation by direct index arithmetic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    /// All levels of the tree.  `levels[0]` = hashed leaves.
    /// `levels[last]` = single root node.
    ///
    /// Each level is the set of nodes at that height; if the number of nodes
    /// is odd, the last node is carried up unchanged (no duplication).
    levels: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    /// Build a Merkle tree from an ordered slice of content hashes.
    ///
    /// The input slice must not be empty; call sites should ensure at least
    /// one leaf exists before calling this.
    ///
    /// # Panics
    ///
    /// Panics if `leaves` is empty.
    pub fn build(leaves: &[[u8; 32]]) -> Self {
        assert!(
            !leaves.is_empty(),
            "MerkleTree::build: cannot build from zero leaves"
        );

        // Level 0: hash each leaf with the domain-separation prefix.
        let level0: Vec<[u8; 32]> = leaves.iter().map(hash_leaf).collect();
        let mut levels = vec![level0];

        // Build each successive level until we reach the root (single node).
        loop {
            let current = levels.last().unwrap();
            if current.len() == 1 {
                break;
            }

            let mut next = Vec::with_capacity((current.len() + 1) / 2);
            let mut i = 0;
            while i < current.len() {
                if i + 1 < current.len() {
                    next.push(hash_inner(&current[i], &current[i + 1]));
                } else {
                    // Odd node: carry up unchanged.
                    next.push(current[i]);
                }
                i += 2;
            }
            levels.push(next);
        }

        Self { levels }
    }

    /// The root hash of this tree.
    pub fn root(&self) -> [u8; 32] {
        *self.levels.last().unwrap().last().unwrap()
    }

    /// Number of leaves in this tree (= number of blocks).
    pub fn leaf_count(&self) -> usize {
        self.levels[0].len()
    }

    /// Generate an inclusion proof for leaf at `index`.
    ///
    /// Returns `None` if `index >= leaf_count()`.
    pub fn proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaf_count() {
            return None;
        }

        let mut path = Vec::new();
        let mut idx = index;

        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };

            // If there is no sibling (odd node at this level), no sibling hash
            // is added — the verifier must apply the same carry-up rule.
            if sibling_idx < level.len() {
                path.push(ProofNode {
                    hash: level[sibling_idx],
                });
            }
            idx /= 2;
        }

        Some(MerkleProof {
            path,
            leaf_count: self.leaf_count(),
        })
    }

    /// Verify that `content_hash` is the leaf at `index` in a tree with root
    /// `expected_root` containing `leaf_count` leaves.
    ///
    /// This is a convenience method equivalent to
    /// `proof.verify(content_hash, index, expected_root)`.
    pub fn verify(
        content_hash: &[u8; 32],
        index: usize,
        leaf_count: usize,
        proof: &MerkleProof,
        expected_root: &[u8; 32],
    ) -> bool {
        proof.verify(content_hash, index, leaf_count, expected_root)
    }
}

// ── MerkleProof ───────────────────────────────────────────────────────────────

/// A single node in a [`MerkleProof`] path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofNode {
    /// Hash of the sibling node at this level.
    pub hash: [u8; 32],
}

/// An inclusion proof demonstrating that a specific leaf is part of a Merkle
/// tree with a known root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Ordered proof path from the leaf toward the root.
    pub path: Vec<ProofNode>,
    /// Total number of leaves in the tree this proof was generated from.
    pub leaf_count: usize,
}

impl MerkleProof {
    /// Verify that `content_hash` at position `index` in a tree with
    /// `leaf_count` leaves produces `expected_root`.
    ///
    /// Returns `true` if and only if the proof is valid.
    pub fn verify(
        &self,
        content_hash: &[u8; 32],
        index: usize,
        leaf_count: usize,
        expected_root: &[u8; 32],
    ) -> bool {
        if index >= leaf_count {
            return false;
        }
        if self.leaf_count != leaf_count {
            return false;
        }

        // Start at the leaf.
        let mut current = hash_leaf(content_hash);
        let mut idx = index;

        // Replay the carry-up rule from the tree builder.
        let mut remaining_at_level = leaf_count;
        let mut path_iter = self.path.iter();

        while remaining_at_level > 1 {
            // Even nodes have a right sibling only if one exists at this level.
            // Odd nodes always have a left sibling (the node before them).
            let has_sibling = if idx % 2 == 0 {
                idx + 1 < remaining_at_level
            } else {
                true
            };

            if has_sibling {
                let node = match path_iter.next() {
                    Some(n) => n,
                    None => return false, // proof too short
                };
                // Sibling is on the right if current idx is even; else on the left.
                current = if idx % 2 == 0 {
                    hash_inner(&current, &node.hash)
                } else {
                    hash_inner(&node.hash, &current)
                };
            }
            // Odd node: no sibling, current carries up unchanged.

            idx /= 2;
            remaining_at_level = (remaining_at_level + 1) / 2;
        }

        // There should be no leftover proof nodes.
        if path_iter.next().is_some() {
            return false;
        }

        &current == expected_root
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_leaf(n: u8) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = n;
        h
    }

    /// Single-leaf tree: root is hash_leaf(leaf0).
    #[test]
    fn single_leaf() {
        let leaf = make_leaf(42);
        let tree = MerkleTree::build(&[leaf]);
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.root(), hash_leaf(&leaf));
    }

    /// Two-leaf tree: root = hash_inner(hash_leaf(L0), hash_leaf(L1)).
    #[test]
    fn two_leaves() {
        let l0 = make_leaf(1);
        let l1 = make_leaf(2);
        let tree = MerkleTree::build(&[l0, l1]);
        let expected = hash_inner(&hash_leaf(&l0), &hash_leaf(&l1));
        assert_eq!(tree.root(), expected);
    }

    /// Root changes when any leaf changes.
    #[test]
    fn root_changes_on_leaf_mutation() {
        let leaves: Vec<[u8; 32]> = (0..8).map(make_leaf).collect();
        let tree1 = MerkleTree::build(&leaves);
        let mut leaves2 = leaves.clone();
        leaves2[3][1] = 0xFF;
        let tree2 = MerkleTree::build(&leaves2);
        assert_ne!(tree1.root(), tree2.root());
    }

    /// All proofs for a tree must verify correctly.
    #[test]
    fn all_proofs_verify() {
        for n in [1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32] {
            let leaves: Vec<[u8; 32]> = (0..n).map(|i| make_leaf(i as u8)).collect();
            let tree = MerkleTree::build(&leaves);
            let root = tree.root();

            for i in 0..n {
                let proof = tree.proof(i).expect("proof must exist for valid index");
                assert!(
                    proof.verify(&leaves[i], i, n, &root),
                    "proof failed for leaf {i} in tree of {n} leaves",
                );
            }
        }
    }

    /// A tampered leaf must not verify.
    #[test]
    fn tampered_leaf_fails() {
        let leaves: Vec<[u8; 32]> = (0..8).map(make_leaf).collect();
        let tree = MerkleTree::build(&leaves);
        let root = tree.root();
        let proof = tree.proof(3).unwrap();

        let mut bad_leaf = leaves[3];
        bad_leaf[0] ^= 0xFF;
        assert!(!proof.verify(&bad_leaf, 3, 8, &root));
    }

    /// A proof for one index must not verify at a different index.
    #[test]
    fn wrong_index_fails() {
        let leaves: Vec<[u8; 32]> = (0..8).map(make_leaf).collect();
        let tree = MerkleTree::build(&leaves);
        let root = tree.root();
        let proof = tree.proof(2).unwrap();
        assert!(!proof.verify(&leaves[2], 3, 8, &root));
    }

    /// Out-of-bounds proof request returns None.
    #[test]
    fn out_of_bounds_proof() {
        let leaves = vec![make_leaf(0)];
        let tree = MerkleTree::build(&leaves);
        assert!(tree.proof(1).is_none());
    }

    /// Round-trip through serde must be lossless.
    #[test]
    fn serde_roundtrip() {
        let leaves: Vec<[u8; 32]> = (0..5).map(make_leaf).collect();
        let tree = MerkleTree::build(&leaves);
        let root = tree.root();

        let json = serde_json::to_string(&tree).unwrap();
        let tree2: MerkleTree = serde_json::from_str(&json).unwrap();
        assert_eq!(tree2.root(), root);
    }
}
