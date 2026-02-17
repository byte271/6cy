use std::io::{self, Read, Write, Seek, SeekFrom};
use crate::superblock::Superblock;
use crate::block::{encode_block, DataBlockHeader};
use crate::index::FileIndex;
use crate::codec::CodecId;
use crate::recovery::{RecoveryMap, RecoveryCheckpoint};
use chrono::Utc;
pub struct SixCyWriter<W: Write + Seek> {
    writer: W,
    pub superblock: Superblock,
    pub index: FileIndex,
    pub recovery_map: RecoveryMap,
}
pub struct SixCyReader<R: Read + Seek> {
    reader: R,
    pub superblock: Superblock,
    pub index: FileIndex,
}
impl<R: Read + Seek> SixCyReader<R> {
    pub fn new(mut reader: R) -> io::Result<Self> {
        let sb = Superblock::read(&mut reader)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        reader.seek(SeekFrom::Start(sb.index_offset))?;
        let mut index_bytes = vec![0u8; sb.feature_bitmap as usize];
        reader.read_exact(&mut index_bytes)?;
        let index = FileIndex::from_bytes(&index_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Self {
            reader,
            superblock: sb,
            index,
        })
    }
    pub fn unpack_file(&mut self, file_id: u32) -> io::Result<Vec<u8>> {
        let record = self.index.records.iter().find(|r| r.id == file_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;
        let mut full_data = Vec::new();
        for &offset in &record.offsets {
            self.reader.seek(SeekFrom::Start(offset))?;
            let header = DataBlockHeader::read(&mut self.reader)?;
            let mut payload = vec![0u8; header.block_size as usize];
            self.reader.read_exact(&mut payload)?;
            let decompressed = crate::block::decode_block(&header, &payload)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            full_data.extend(decompressed);
        }
        Ok(full_data)
    }
    pub fn read_at(&mut self, file_id: u32, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let data = self.unpack_file(file_id)?;
        let start = offset as usize;
        if start >= data.len() {
            return Ok(0);
        }
        let end = (start + buf.len()).min(data.len());
        let len = end - start;
        buf[..len].copy_from_slice(&data[start..end]);
        Ok(len)
    }
}
impl<W: Write + Seek> SixCyWriter<W> {
    pub fn new(mut writer: W) -> io::Result<Self> {
        let sb = Superblock::new();
        writer.seek(SeekFrom::Start(0))?;
        let dummy = vec![0u8; 64]; 
        writer.write_all(&dummy)?;
        Ok(Self {
            writer,
            superblock: sb,
            index: FileIndex::default(),
            recovery_map: RecoveryMap::default(),
        })
    }
    pub fn add_file(&mut self, name: String, data: &[u8], codec_id: CodecId) -> io::Result<()> {
        let file_id = self.index.records.len() as u32;
        let mut record = crate::index::FileIndexRecord {
            id: file_id,
            parent_id: 0,
            name,
            offsets: Vec::new(),
            original_size: data.len() as u64,
            compressed_size: 0,
            metadata: std::collections::HashMap::new(),
        };
        let (header, payload) = encode_block(file_id, 0, data, codec_id, 3)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let current_offset = self.writer.stream_position()?;
        record.offsets.push(current_offset);
        record.compressed_size = payload.len() as u64;
        header.write(&mut self.writer)?;
        self.writer.write_all(&payload)?;
        self.index.records.push(record);
        let current_pos = self.writer.stream_position()?;
        self.recovery_map.checkpoints.push(RecoveryCheckpoint {
            archive_offset: current_pos,
            last_file_id: file_id,
            timestamp: Utc::now().timestamp(),
        });
        Ok(())
    }
    pub fn finalize(&mut self) -> io::Result<()> {
        let index_offset = self.writer.stream_position()?;
        let index_bytes = self.index.to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        self.writer.write_all(&index_bytes)?;
        let recovery_offset = self.writer.stream_position()?;
        let recovery_bytes = self.recovery_map.to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        self.writer.write_all(&recovery_bytes)?;
        self.superblock.index_offset = index_offset;
        self.superblock.recovery_map_offset = recovery_offset;
        self.superblock.feature_bitmap = index_bytes.len() as u64; 
        self.writer.seek(SeekFrom::Start(0))?;
        self.superblock.write(&mut self.writer)?;
        Ok(())
    }
}
