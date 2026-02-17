use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};
use uuid::Uuid;
use thiserror::Error;
pub const MAGIC: &[u8; 4] = b".6cy";
pub const VERSION: u32 = 1;
#[derive(Error, Debug)]
pub enum SuperblockError {
    #[error("Invalid magic number")]
    InvalidMagic,
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u32),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}
#[derive(Debug, Clone)]
pub struct Superblock {
    pub magic: [u8; 4],
    pub version: u32,
    pub uuid: Uuid,
    pub index_offset: u64,
    pub recovery_map_offset: u64,
    pub feature_bitmap: u64,
}
impl Superblock {
    pub fn new() -> Self {
        Self {
            magic: *MAGIC,
            version: VERSION,
            uuid: Uuid::new_v4(),
            index_offset: 0,
            recovery_map_offset: 0,
            feature_bitmap: 0,
        }
    }
    pub fn write<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_u32::<LittleEndian>(self.version)?;
        writer.write_all(self.uuid.as_bytes())?;
        writer.write_u64::<LittleEndian>(self.index_offset)?;
        writer.write_u64::<LittleEndian>(self.recovery_map_offset)?;
        writer.write_u64::<LittleEndian>(self.feature_bitmap)?;
        Ok(())
    }
    pub fn read<R: Read>(mut reader: R) -> Result<Self, SuperblockError> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(SuperblockError::InvalidMagic);
        }
        let version = reader.read_u32::<LittleEndian>()?;
        if version != VERSION {
            return Err(SuperblockError::UnsupportedVersion(version));
        }
        let mut uuid_bytes = [0u8; 16];
        reader.read_exact(&mut uuid_bytes)?;
        let uuid = Uuid::from_bytes(uuid_bytes);
        let index_offset = reader.read_u64::<LittleEndian>()?;
        let recovery_map_offset = reader.read_u64::<LittleEndian>()?;
        let feature_bitmap = reader.read_u64::<LittleEndian>()?;
        Ok(Self {
            magic,
            version,
            uuid,
            index_offset,
            recovery_map_offset,
            feature_bitmap,
        })
    }
}
