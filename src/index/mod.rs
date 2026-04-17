//! File index — reconstructible by scanning blocks.
use crate::merkle::MerkleTree;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockRef {
    pub content_hash: [u8; 32],
    pub archive_offset: u64,
    #[serde(default)]
    pub intra_offset: u64,
    #[serde(default)]
    pub intra_length: u64,
}

impl BlockRef {
    #[inline]
    pub fn is_solid_slice(&self) -> bool {
        self.intra_length > 0
    }
}

/// Metadata about one FEC stripe for a file.
///
/// Stored in the FileIndex so the reader knows which archive offsets contain
/// parity blocks for a given stripe, enabling targeted FEC recovery.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FecStripeRef {
    /// Index of the first data block (within `block_refs`) belonging to this stripe.
    pub first_data_block: usize,
    /// Number of data blocks in this stripe.
    pub data_count: usize,
    /// Archive offsets of the parity blocks for this stripe, in shard order.
    pub parity_offsets: Vec<u64>,
    /// Byte length of each (padded) shard in this stripe.
    pub shard_size: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileIndexRecord {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub block_refs: Vec<BlockRef>,
    pub original_size: u64,
    pub compressed_size: u64,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// FEC stripe descriptors.  Empty for files packed without FEC.
    #[serde(default)]
    pub fec_stripes: Vec<FecStripeRef>,
    /// Merkle inclusion proofs for each block in this file.
    /// Parallel to `block_refs`.  Empty for archives built without Merkle trees.
    #[serde(default)]
    pub merkle_proofs: Vec<crate::merkle::MerkleProof>,
}

impl FileIndexRecord {
    pub fn from_scan(file_id: u32, original_size: u64, refs: Vec<BlockRef>) -> Self {
        Self {
            id: file_id,
            parent_id: 0,
            name: format!("file_{file_id:08x}"),
            block_refs: refs,
            original_size,
            compressed_size: 0,
            metadata: HashMap::new(),
            fec_stripes: Vec::new(),
            merkle_proofs: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FileIndex {
    pub records: Vec<FileIndexRecord>,
    /// Flat BLAKE3 hash-of-hashes (legacy, always present for backward compat).
    pub root_hash: [u8; 32],
    /// Full binary Merkle tree over all block content hashes.
    /// `None` for archives built without the `--merkle` flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merkle_tree: Option<MerkleTree>,
}

impl FileIndex {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Compute the flat (legacy) root hash: sequential BLAKE3 over all
    /// `content_hash` values in record-order, block-order.
    pub fn compute_root_hash(&mut self) {
        let mut h = blake3::Hasher::new();
        for rec in &self.records {
            for br in &rec.block_refs {
                h.update(&br.content_hash);
            }
        }
        self.root_hash = h.finalize().into();
    }

    /// Build a binary Merkle tree over all block content hashes and store it
    /// in `self.merkle_tree`.  Also fills `merkle_proofs` in each record.
    ///
    /// This method also calls `compute_root_hash` to keep the flat root in
    /// sync.  Call it once during `finalize()`.
    pub fn build_merkle_tree(&mut self) {
        self.compute_root_hash();

        // Collect all content hashes in canonical order (record → block).
        let all_hashes: Vec<[u8; 32]> = self
            .records
            .iter()
            .flat_map(|r| r.block_refs.iter().map(|br| br.content_hash))
            .collect();

        if all_hashes.is_empty() {
            return;
        }

        let tree = MerkleTree::build(&all_hashes);
        let root = tree.root();

        // Generate and attach per-block inclusion proofs.
        let mut global_idx = 0usize;
        for rec in &mut self.records {
            rec.merkle_proofs.clear();
            for _ in &rec.block_refs {
                if let Some(proof) = tree.proof(global_idx) {
                    rec.merkle_proofs.push(proof);
                }
                global_idx += 1;
            }
        }

        self.merkle_tree = Some(tree);
        // Also store the Merkle root as the root_hash for this archive.
        self.root_hash = root;
    }

    /// Verify that the stored Merkle tree is consistent with block content
    /// hashes.  Returns `true` if every block proof verifies against the root.
    ///
    /// Returns `false` if no Merkle tree is present (legacy archive).
    pub fn verify_merkle(&self) -> bool {
        let tree = match &self.merkle_tree {
            Some(t) => t,
            None => return false,
        };
        let root = tree.root();
        let leaf_count = tree.leaf_count();

        let mut global_idx = 0usize;
        for rec in &self.records {
            for (i, br) in rec.block_refs.iter().enumerate() {
                let proof = match rec.merkle_proofs.get(i) {
                    Some(p) => p,
                    None => return false,
                };
                if !proof.verify(&br.content_hash, global_idx, leaf_count, &root) {
                    return false;
                }
                global_idx += 1;
            }
        }
        true
    }
}
