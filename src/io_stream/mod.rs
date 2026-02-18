use std::io::{self, Read, Write, Seek, SeekFrom};
use crate::superblock::Superblock;
use crate::block::{encode_block, DataBlockHeader};
use crate::index::{FileIndex, BlockRef};
use crate::codec::CodecId;
use std::collections::HashMap;

use crate::recovery::{RecoveryMap, RecoveryCheckpoint};
use chrono::Utc;

pub struct SixCyWriter<W: Write + Seek> {
    writer: W,
    pub superblock: Superblock,
    pub index: FileIndex,
    pub recovery_map: RecoveryMap,
    solid_buffer: Vec<u8>,
    solid_codec: Option<CodecId>,
    // CAS: Map content hash to offset in the current archive
    block_dedup: HashMap<[u8; 32], u64>,
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
        let mut index_bytes = vec![0u8; sb.index_size as usize];
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
        for block_ref in &record.block_refs {
            if block_ref.archive_id.is_some() {
                return Err(io::Error::new(io::ErrorKind::Other, "Cross-archive referencing not yet implemented in reader"));
            }
            self.reader.seek(SeekFrom::Start(block_ref.offset))?;
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
        let record = self.index.records.iter().find(|r| r.id == file_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        if offset >= record.original_size {
            return Ok(0);
        }

        let mut current_block_start = 0u64;
        for block_ref in &record.block_refs {
            self.reader.seek(SeekFrom::Start(block_ref.offset))?;
            let header = DataBlockHeader::read(&mut self.reader)?;
            
            let mut payload = vec![0u8; header.block_size as usize];
            self.reader.read_exact(&mut payload)?;
            let decompressed = crate::block::decode_block(&header, &payload)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            
            let block_len = decompressed.len() as u64;
            if offset >= current_block_start && offset < current_block_start + block_len {
                let relative_offset = (offset - current_block_start) as usize;
                let available = (block_len - relative_offset as u64) as usize;
                let to_copy = buf.len().min(available);
                buf[..to_copy].copy_from_slice(&decompressed[relative_offset..relative_offset + to_copy]);
                return Ok(to_copy);
            }
            current_block_start += block_len;
        }
        
        Ok(0)
    }
}

impl<W: Write + Seek> SixCyWriter<W> {
    pub fn new(mut writer: W) -> io::Result<Self> {
        let sb = Superblock::new();
        writer.seek(SeekFrom::Start(0))?;
        let dummy = vec![0u8; 128];
        writer.write_all(&dummy)?;
        
        Ok(Self {
            writer,
            superblock: sb,
            index: FileIndex::default(),
            recovery_map: RecoveryMap::default(),
            solid_buffer: Vec::new(),
            solid_codec: None,
            block_dedup: HashMap::new(),
        })
    }

    pub fn start_solid_session(&mut self, codec_id: CodecId) -> io::Result<()> {
        self.flush_solid_session()?;
        self.solid_codec = Some(codec_id);
        Ok(())
    }

    pub fn flush_solid_session(&mut self) -> io::Result<()> {
        if let Some(codec_id) = self.solid_codec {
            if !self.solid_buffer.is_empty() {
                let cid_val = codec_id as u16;
                if !self.superblock.required_codecs.contains(&cid_val) {
                    self.superblock.required_codecs.push(cid_val);
                }

                let (header, payload) = encode_block(0xFFFFFFFF, 0, &self.solid_buffer, codec_id, 3)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                
                let _current_offset = self.writer.stream_position()?;
                header.write(&mut self.writer)?;
                self.writer.write_all(&payload)?;

                self.solid_buffer.clear();
            }
        }
        self.solid_codec = None;
        Ok(())
    }

    pub fn add_file(&mut self, name: String, data: &[u8], codec_id: CodecId) -> io::Result<()> {
        if let Some(_s_codec) = self.solid_codec {
            let file_id = self.index.records.len() as u32;
            let record = crate::index::FileIndexRecord {
                id: file_id,
                parent_id: 0,
                name,
                block_refs: Vec::new(),
                original_size: data.len() as u64,
                compressed_size: 0,
                metadata: std::collections::HashMap::new(),
            };
            self.index.records.push(record);
            self.solid_buffer.extend_from_slice(data);
            Ok(())
        } else {
            let cid_val = codec_id as u16;
            if !self.superblock.required_codecs.contains(&cid_val) {
                self.superblock.required_codecs.push(cid_val);
            }
            
            let file_id = self.index.records.len() as u32;
            let mut record = crate::index::FileIndexRecord {
                id: file_id,
                parent_id: 0,
                name,
                block_refs: Vec::new(),
                original_size: data.len() as u64,
                compressed_size: 0,
                metadata: std::collections::HashMap::new(),
            };

            // CAS: Check for deduplication
            let content_hash: [u8; 32] = blake3::hash(data).into();
            if let Some(&offset) = self.block_dedup.get(&content_hash) {
                record.block_refs.push(BlockRef {
                    hash: content_hash,
                    offset,
                    archive_id: None,
                });
                println!("Deduplicated block: {}", hex::encode(content_hash));
            } else {
                let (header, payload) = encode_block(file_id, 0, data, codec_id, 3)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                let current_offset = self.writer.stream_position()?;
                header.write(&mut self.writer)?;
                self.writer.write_all(&payload)?;

                record.block_refs.push(BlockRef {
                    hash: content_hash,
                    offset: current_offset,
                    archive_id: None,
                });
                record.compressed_size = payload.len() as u64;
                self.block_dedup.insert(content_hash, current_offset);
            }

            self.index.records.push(record);

            let current_pos = self.writer.stream_position()?;
            self.recovery_map.checkpoints.push(RecoveryCheckpoint {
                archive_offset: current_pos,
                last_file_id: file_id,
                timestamp: Utc::now().timestamp(),
            });

            Ok(())
        }
    }

    pub fn finalize(&mut self) -> io::Result<()> {
        self.flush_solid_session()?;
        
        // Calculate root hash for remote verification
        let mut hasher = blake3::Hasher::new();
        for record in &self.index.records {
            for block_ref in &record.block_refs {
                hasher.update(&block_ref.hash);
            }
        }
        self.index.root_hash = hasher.finalize().into();

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
        self.superblock.index_size = index_bytes.len() as u64;
        
        self.writer.seek(SeekFrom::Start(0))?;
        self.superblock.write(&mut self.writer)?;
        
        Ok(())
    }
}
