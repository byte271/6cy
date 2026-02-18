use serde::{Serialize, Deserialize, Deserializer};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockRef {
    pub hash: [u8; 32],
    pub offset: u64,
    pub archive_id: Option<String>, // For cross-archive referencing
}

/// Serialize FileIndexRecord using the current "block_refs" field name.
/// Deserialize with backward compatibility: accepts both old "offsets: Vec<u64>"
/// (written by pre-CAS versions) and new "block_refs: Vec<BlockRef>" layouts.
#[derive(Debug, Serialize, Clone)]
pub struct FileIndexRecord {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub block_refs: Vec<BlockRef>,
    pub original_size: u64,
    pub compressed_size: u64,
    pub metadata: HashMap<String, String>,
}

// Helper used only during deserialization to accept both old and new field names.
#[derive(Deserialize)]
struct FileIndexRecordRaw {
    id: u32,
    parent_id: u32,
    name: String,
    // New format: rich BlockRef objects
    #[serde(default)]
    block_refs: Option<Vec<BlockRef>>,
    // Old format: plain byte offsets (spec field name "offsets")
    #[serde(default)]
    offsets: Option<Vec<u64>>,
    original_size: u64,
    compressed_size: u64,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

impl<'de> Deserialize<'de> for FileIndexRecord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = FileIndexRecordRaw::deserialize(deserializer)?;

        // Prefer new block_refs; fall back to converting legacy offsets.
        let block_refs = if let Some(refs) = raw.block_refs {
            refs
        } else if let Some(offsets) = raw.offsets {
            // Convert legacy Vec<u64> offsets to stub BlockRefs.
            // Hashes are zeroed — CAS dedup won't fire for these entries,
            // but offset-based unpacking still works correctly.
            offsets
                .into_iter()
                .map(|offset| BlockRef {
                    hash: [0u8; 32],
                    offset,
                    archive_id: None,
                })
                .collect()
        } else {
            return Err(serde::de::Error::custom(
                "FileIndexRecord must have either 'block_refs' or 'offsets'",
            ));
        };

        Ok(FileIndexRecord {
            id: raw.id,
            parent_id: raw.parent_id,
            name: raw.name,
            block_refs,
            original_size: raw.original_size,
            compressed_size: raw.compressed_size,
            metadata: raw.metadata,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FileIndex {
    pub records: Vec<FileIndexRecord>,
    pub root_hash: [u8; 32], // Merkle root of all block hashes for remote verification
}

impl FileIndex {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
