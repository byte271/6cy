use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};
use crate::codec::{CodecId, get_codec, CodecError};
use crc32fast::Hasher;
pub const BLOCK_MAGIC: u32 = 0x424C434B; 
#[derive(Debug, Clone)]
pub struct DataBlockHeader {
    pub magic: u32,
    pub block_size: u32,
    pub file_id: u32,
    pub file_offset: u64,
    pub codec_id: u8,
    pub level: i8,
    pub flags: u16,
    pub checksum: u32,
}
impl DataBlockHeader {
    pub fn write<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_u32::<LittleEndian>(self.magic)?;
        writer.write_u32::<LittleEndian>(self.block_size)?;
        writer.write_u32::<LittleEndian>(self.file_id)?;
        writer.write_u64::<LittleEndian>(self.file_offset)?;
        writer.write_u8(self.codec_id)?;
        writer.write_i8(self.level)?;
        writer.write_u16::<LittleEndian>(self.flags)?;
        writer.write_u32::<LittleEndian>(self.checksum)?;
        Ok(())
    }
    pub fn read<R: Read>(mut reader: R) -> io::Result<Self> {
        Ok(Self {
            magic: reader.read_u32::<LittleEndian>()?,
            block_size: reader.read_u32::<LittleEndian>()?,
            file_id: reader.read_u32::<LittleEndian>()?,
            file_offset: reader.read_u64::<LittleEndian>()?,
            codec_id: reader.read_u8()?,
            level: reader.read_i8()?,
            flags: reader.read_u16::<LittleEndian>()?,
            checksum: reader.read_u32::<LittleEndian>()?,
        })
    }
}
pub fn encode_block(file_id: u32, file_offset: u64, data: &[u8], codec_id: CodecId, level: i32) -> Result<(DataBlockHeader, Vec<u8>), CodecError> {
    let codec = get_codec(codec_id)?;
    let compressed_payload = codec.compress(data, level)?;
    let mut hasher = Hasher::new();
    hasher.update(&compressed_payload);
    let checksum = hasher.finalize();
    let header = DataBlockHeader {
        magic: BLOCK_MAGIC,
        block_size: compressed_payload.len() as u32,
        file_id,
        file_offset,
        codec_id: codec_id as u8,
        level: level as i8,
        flags: 0,
        checksum,
    };
    Ok((header, compressed_payload))
}
pub fn decode_block(header: &DataBlockHeader, payload: &[u8]) -> Result<Vec<u8>, CodecError> {
    let mut hasher = Hasher::new();
    hasher.update(payload);
    if hasher.finalize() != header.checksum {
        return Err(CodecError::Decompression("Checksum mismatch".to_string()));
    }
    let codec = get_codec(CodecId::from(header.codec_id))?;
    codec.decompress(payload)
}
