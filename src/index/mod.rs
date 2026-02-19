//! File index — reconstructible by scanning blocks.
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockRef {
    pub content_hash:   [u8; 32],
    pub archive_offset: u64,
    #[serde(default)]
    pub intra_offset:   u64,
    #[serde(default)]
    pub intra_length:   u64,
}

impl BlockRef {
    #[inline]
    pub fn is_solid_slice(&self) -> bool { self.intra_length > 0 }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileIndexRecord {
    pub id:              u32,
    pub parent_id:       u32,
    pub name:            String,
    pub block_refs:      Vec<BlockRef>,
    pub original_size:   u64,
    pub compressed_size: u64,
    #[serde(default)]
    pub metadata:        HashMap<String, String>,
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
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FileIndex {
    pub records:   Vec<FileIndexRecord>,
    pub root_hash: [u8; 32],
}

impl FileIndex {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
    pub fn compute_root_hash(&mut self) {
        let mut h = blake3::Hasher::new();
        for rec in &self.records {
            for br in &rec.block_refs {
                h.update(&br.content_hash);
            }
        }
        self.root_hash = h.finalize().into();
    }
}
