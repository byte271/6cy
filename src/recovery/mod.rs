use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecoveryCheckpoint {
    pub archive_offset: u64,
    pub last_file_id: u32,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RecoveryMap {
    pub checkpoints: Vec<RecoveryCheckpoint>,
}

impl RecoveryMap {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
