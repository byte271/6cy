use serde::{Serialize, Deserialize};
use std::collections::HashMap;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileIndexRecord {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub offsets: Vec<u64>, 
    pub original_size: u64,
    pub compressed_size: u64,
    pub metadata: HashMap<String, String>,
}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FileIndex {
    pub records: Vec<FileIndexRecord>,
}
impl FileIndex {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
