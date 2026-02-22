//! Streaming archive engine — writer and reader.
//!
//! # Writer
//! [`SixCyWriter`] accepts files one at a time, splits them into chunks,
//! compresses (and optionally encrypts) each chunk, and writes it as a
//! self-describing DATA block.  Identical chunks are deduplicated via CAS
//! (content-addressable storage, keyed on BLAKE3 of uncompressed content).
//! A full INDEX block is written at the end; the superblock is patched in
//! place at offset 0 on `finalize()`.
//!
//! # Reader (normal path)
//! [`SixCyReader`] reads the superblock, performs an upfront codec
//! availability check (fail hard if any required codec is missing — no
//! negotiation), then seeks to the INDEX block to build the file list.
//!
//! # Reader (reconstruction path)
//! If the INDEX block is absent or corrupt, `SixCyReader::scan_blocks()`
//! reconstructs the block list by reading every block header sequentially.
//! File names are unknown in that case and are synthesised as
//! `"file_{file_id:08x}"`.  Solid-block file names cannot be recovered
//! without the INDEX block.
//!
//! # Endianness
//! All binary I/O is strictly little-endian; see `block.rs` and
//! `superblock.rs` for field-level documentation.  No runtime negotiation
//! is ever performed.

use std::io::{self, Read, Write, Seek, SeekFrom};
use std::collections::HashMap;
use crate::superblock::{Superblock, SUPERBLOCK_SIZE};
use crate::block::{encode_block, decode_block, BlockHeader, BlockType, FILE_ID_SHARED};
use crate::index::{FileIndex, FileIndexRecord, BlockRef};
use crate::codec::CodecId;
use crate::recovery::{RecoveryMap, RecoveryCheckpoint};
use chrono::Utc;

/// Default chunk size: 4 MiB.
pub const DEFAULT_CHUNK_SIZE:        usize = 4 * 1024 * 1024;
/// Default Zstd compression level.
pub const DEFAULT_COMPRESSION_LEVEL: i32   = 3;

// ── Writer ───────────────────────────────────────────────────────────────────

pub struct SixCyWriter<W: Write + Seek> {
    writer:            W,
    pub superblock:    Superblock,
    pub index:         FileIndex,
    pub recovery_map:  RecoveryMap,

    // Solid-mode accumulation
    solid_buffer:      Vec<u8>,
    solid_codec:       Option<CodecId>,
    /// (file_id, intra_offset, intra_length, content_hash)
    solid_file_ranges: Vec<(u32, u64, u64, [u8; 32])>,

    // CAS: BLAKE3(uncompressed chunk) → (archive_offset, compressed_payload_len)
    block_dedup:       HashMap<[u8; 32], (u64, u64)>,

    pub chunk_size:        usize,
    pub compression_level: i32,
    pub encryption_key:    Option<[u8; 32]>,
}

impl<W: Write + Seek> SixCyWriter<W> {
    pub fn new(writer: W) -> io::Result<Self> {
        Self::with_options(writer, DEFAULT_CHUNK_SIZE, DEFAULT_COMPRESSION_LEVEL, None)
    }

    pub fn with_options(
        mut writer:        W,
        chunk_size:        usize,
        compression_level: i32,
        encryption_key:    Option<[u8; 32]>,
    ) -> io::Result<Self> {
        let sb = Superblock::new();
        writer.seek(SeekFrom::Start(0))?;
        writer.write_all(&[0u8; SUPERBLOCK_SIZE])?; // reserved; overwritten on finalize
        Ok(Self {
            writer,
            superblock:        sb,
            index:             FileIndex::default(),
            recovery_map:      RecoveryMap::default(),
            solid_buffer:      Vec::new(),
            solid_codec:       None,
            solid_file_ranges: Vec::new(),
            block_dedup:       HashMap::new(),
            chunk_size:        chunk_size.max(1),
            compression_level,
            encryption_key,
        })
    }

    // ── Solid mode ──────────────────────────────────────────────────────────

    /// Begin accumulating files into a single compressed solid block.
    /// Flushes any open solid session first.
    pub fn start_solid_session(&mut self, codec: CodecId) -> io::Result<()> {
        self.flush_solid_session()?;
        self.solid_codec = Some(codec);
        Ok(())
    }

    /// Compress the accumulated solid buffer as one SOLID block and update
    /// every pending file's block_refs with correct intra-block ranges.
    pub fn flush_solid_session(&mut self) -> io::Result<()> {
        let codec = match self.solid_codec.take() {
            Some(c) => c,
            None    => return Ok(()),
        };
        if self.solid_buffer.is_empty() {
            self.solid_file_ranges.clear();
            return Ok(());
        }

        self.superblock.add_required_codec(codec);

        let (header, payload) = encode_block(
            BlockType::Solid,
            FILE_ID_SHARED,
            0,
            &self.solid_buffer,
            codec,
            self.compression_level,
            self.encryption_key.as_ref(),
        ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let archive_offset = self.writer.stream_position()?;
        let payload_len    = payload.len() as u64;
        header.write(&mut self.writer)?;
        self.writer.write_all(&payload)?;

        for (file_id, intra_offset, intra_length, content_hash) in
            self.solid_file_ranges.drain(..)
        {
            if let Some(rec) = self.index.records.iter_mut().find(|r| r.id == file_id) {
                rec.block_refs.push(BlockRef {
                    content_hash,
                    archive_offset,
                    intra_offset,
                    intra_length,
                });
                rec.compressed_size = payload_len;
            }
        }
        self.solid_buffer.clear();
        Ok(())
    }

    // ── File ingestion ───────────────────────────────────────────────────────

    /// Add a file to the archive.
    ///
    /// **Solid mode**: data accumulates in the buffer; block_refs are filled
    /// by the next `flush_solid_session`.
    ///
    /// **Normal mode**: data is split into `chunk_size` chunks.  Each unique
    /// chunk is written once (CAS deduplication); subsequent identical chunks
    /// receive a BlockRef pointing at the existing block.
    pub fn add_file(
        &mut self,
        name:  String,
        data:  &[u8],
        codec: CodecId,
    ) -> io::Result<()> {
        let file_id = self.index.records.len() as u32;

        if self.solid_codec.is_some() {
            // ── Solid mode ──────────────────────────────────────────────────
            let intra_offset = self.solid_buffer.len() as u64;
            let intra_length = data.len() as u64;
            let content_hash: [u8; 32] = blake3::hash(data).into();

            self.solid_file_ranges.push((file_id, intra_offset, intra_length, content_hash));
            self.solid_buffer.extend_from_slice(data);

            self.index.records.push(FileIndexRecord {
                id:              file_id,
                parent_id:       0,
                name,
                block_refs:      Vec::new(),
                original_size:   data.len() as u64,
                compressed_size: 0,
                metadata:        HashMap::new(),
            });
            return Ok(());
        }

        // ── Normal (chunked CAS) mode ────────────────────────────────────────
        self.superblock.add_required_codec(codec);

        let mut record = FileIndexRecord {
            id:              file_id,
            parent_id:       0,
            name,
            block_refs:      Vec::new(),
            original_size:   data.len() as u64,
            compressed_size: 0,
            metadata:        HashMap::new(),
        };

        for (chunk_idx, chunk) in data.chunks(self.chunk_size).enumerate() {
            let file_offset:  u64       = (chunk_idx * self.chunk_size) as u64;
            let content_hash: [u8; 32]  = blake3::hash(chunk).into();

            if let Some(&(existing_offset, comp_len)) = self.block_dedup.get(&content_hash) {
                // CAS hit — reuse existing block, no new I/O.
                record.block_refs.push(BlockRef {
                    content_hash,
                    archive_offset: existing_offset,
                    intra_offset:   0,
                    intra_length:   0,
                });
                record.compressed_size += comp_len;
            } else {
                // New chunk — compress, (optionally) encrypt, write.
                let (header, payload) = encode_block(
                    BlockType::Data,
                    file_id,
                    file_offset,
                    chunk,
                    codec,
                    self.compression_level,
                    self.encryption_key.as_ref(),
                ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                let archive_offset = self.writer.stream_position()?;
                let comp_len       = payload.len() as u64;
                header.write(&mut self.writer)?;
                self.writer.write_all(&payload)?;

                record.compressed_size += comp_len;
                self.block_dedup.insert(content_hash, (archive_offset, comp_len));
                record.block_refs.push(BlockRef {
                    content_hash,
                    archive_offset,
                    intra_offset: 0,
                    intra_length: 0,
                });
            }
        }

        self.recovery_map.checkpoints.push(RecoveryCheckpoint {
            archive_offset: self.writer.stream_position()?,
            last_file_id:   file_id,
            timestamp:      Utc::now().timestamp(),
        });

        self.index.records.push(record);
        Ok(())
    }

    // ── Finalization ─────────────────────────────────────────────────────────

    /// Flush any open solid session, write the INDEX block, then patch the
    /// superblock at offset 0.  Must be called exactly once.
    pub fn finalize(&mut self) -> io::Result<()> {
        self.flush_solid_session()?;

        // Merkle root over all content hashes.
        self.index.compute_root_hash();

        // Serialize the FileIndex.
        let index_payload = self.index.to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Write the INDEX block — codec=None (stored verbatim), unencrypted.
        let (idx_header, idx_on_disk) = encode_block(
            BlockType::Index,
            FILE_ID_SHARED,
            0,
            &index_payload,
            CodecId::Zstd,           // compress the index with Zstd always
            DEFAULT_COMPRESSION_LEVEL,
            None,                     // index is never encrypted
        ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let index_offset = self.writer.stream_position()?;
        idx_header.write(&mut self.writer)?;
        self.writer.write_all(&idx_on_disk)?;

        // Write the RecoveryMap (JSON blob, no block wrapper needed).
        let recovery_bytes = self.recovery_map.to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let recovery_offset = self.writer.stream_position()?;
        // Write recovery map length prefix (LE u64) then data.
        self.writer.write_all(&(recovery_bytes.len() as u64).to_le_bytes())?;
        self.writer.write_all(&recovery_bytes)?;

        // Patch the superblock.
        self.superblock.index_offset = index_offset;
        self.superblock.index_size   = idx_on_disk.len() as u64;
        if self.encryption_key.is_some() {
            self.superblock.flags |= crate::superblock::SB_FLAG_ENCRYPTED;
        }
        // recovery_map_offset stored in superblock for diagnostics
        // (superblock doesn't have the field in v3; stored in RecoveryCheckpoint)
        let _ = recovery_offset; // acknowledged

        self.writer.seek(SeekFrom::Start(0))?;
        self.superblock.write(&mut self.writer)?;

        Ok(())
    }
}

// ── Reader ───────────────────────────────────────────────────────────────────

pub struct SixCyReader<R: Read + Seek> {
    reader:             R,
    pub superblock:     Superblock,
    pub index:          FileIndex,
    pub decryption_key: Option<[u8; 32]>,
}

impl<R: Read + Seek> SixCyReader<R> {
    pub fn new(reader: R) -> io::Result<Self> {
        Self::with_key(reader, None)
    }

    /// Open an archive.  Performs an upfront codec availability check —
    /// fails immediately if the superblock lists a codec UUID not available
    /// in this build.  No partial opening, no negotiation.
    pub fn with_key(mut reader: R, decryption_key: Option<[u8; 32]>) -> io::Result<Self> {
        // Superblock::read already calls check_codecs() internally.
        let sb = Superblock::read(&mut reader)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Read and decompress the INDEX block.
        reader.seek(SeekFrom::Start(sb.index_offset))?;
        let idx_header = BlockHeader::read(&mut reader)?;
        let mut idx_payload = vec![0u8; idx_header.comp_size as usize];
        reader.read_exact(&mut idx_payload)?;

        let idx_raw = decode_block(&idx_header, &idx_payload, None)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let index = FileIndex::from_bytes(&idx_raw)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(Self { reader, superblock: sb, index, decryption_key })
    }

    // ── Block reconstruction (no INDEX) ──────────────────────────────────────

    /// Reconstruct the file list by scanning every block header sequentially
    /// from `SUPERBLOCK_SIZE` onward.
    ///
    /// Used when the INDEX block is missing or corrupt.  File names are
    /// synthesised as `"file_{file_id:08x}"`.  Solid-block contents cannot
    /// be recovered without the INDEX; solid blocks are noted but their
    /// intra-file ranges are not reconstructed.
    ///
    /// Returns the reconstructed [`FileIndex`] without modifying `self.index`.
    pub fn scan_blocks(&mut self) -> io::Result<FileIndex> {
        self.reader.seek(SeekFrom::Start(SUPERBLOCK_SIZE as u64))?;

        // file_id → Vec<(file_offset, BlockRef)>
        let mut chunks: HashMap<u32, Vec<(u64, BlockRef)>> = HashMap::new();
        let mut orig_sizes: HashMap<u32, u64> = HashMap::new();

        loop {
            let pos = match self.reader.stream_position() {
                Ok(p) => p,
                Err(_) => break,
            };

            let header = match BlockHeader::read(&mut self.reader) {
                Ok(h)  => h,
                Err(_) => break,   // EOF or corruption — stop scan here
            };

            // Skip the payload bytes to reach the next block.
            let skip = header.comp_size as u64;
            match self.reader.seek(SeekFrom::Current(skip as i64)) {
                Ok(_)  => {},
                Err(_) => break,
            }

            match header.block_type {
                BlockType::Index => break, // reached the end sentinel
                BlockType::Solid => {
                    // Solid block — we know its position but not which files
                    // it contains (intra-offsets are in the INDEX).
                    // Record it under the sentinel file_id for diagnostics.
                }
                BlockType::Data => {
                    let fid = header.file_id;
                    // Track the maximum observed file extent.
                    let end = header.file_offset + header.orig_size as u64;
                    let cur = orig_sizes.entry(fid).or_insert(0);
                    if end > *cur { *cur = end; }

                    let block_ref = BlockRef {
                        content_hash:   header.content_hash,
                        archive_offset: pos,
                        intra_offset:   0,
                        intra_length:   0,
                    };
                    chunks.entry(fid)
                        .or_default()
                        .push((header.file_offset, block_ref));
                }
            }
        }

        // Sort each file's chunks by file_offset and build FileIndexRecords.
        let mut records: Vec<FileIndexRecord> = chunks.into_iter().map(|(fid, mut v)| {
            v.sort_by_key(|(off, _)| *off);
            let refs: Vec<BlockRef> = v.into_iter().map(|(_, r)| r).collect();
            let size = *orig_sizes.get(&fid).unwrap_or(&0);
            FileIndexRecord::from_scan(fid, size, refs)
        }).collect();
        records.sort_by_key(|r| r.id);

        let mut idx = FileIndex { records, root_hash: [0u8; 32] };
        idx.compute_root_hash();
        Ok(idx)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn read_block_at(&mut self, offset: u64) -> io::Result<(BlockHeader, Vec<u8>)> {
        self.reader.seek(SeekFrom::Start(offset))?;
        let header = BlockHeader::read(&mut self.reader)?;
        let mut payload = vec![0u8; header.comp_size as usize];
        self.reader.read_exact(&mut payload)?;
        Ok((header, payload))
    }

    fn decompress_ref(&mut self, br: &BlockRef) -> io::Result<Vec<u8>> {
        let (header, payload) = self.read_block_at(br.archive_offset)?;
        let decompressed = decode_block(&header, &payload, self.decryption_key.as_ref())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        if br.is_solid_slice() {
            let start = br.intra_offset as usize;
            let end   = start + br.intra_length as usize;
            if end > decompressed.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!(
                    "Solid intra range {start}..{end} exceeds decompressed size {}",
                    decompressed.len()
                )));
            }
            Ok(decompressed[start..end].to_vec())
        } else {
            Ok(decompressed)
        }
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Return the complete contents of a file by record ID.
    pub fn unpack_file(&mut self, file_id: u32) -> io::Result<Vec<u8>> {
        let record = self.index.records.iter()
            .find(|r| r.id == file_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        let refs = record.block_refs.clone();
        let mut out = Vec::with_capacity(record.original_size as usize);
        for br in &refs {
            out.extend(self.decompress_ref(br)?);
        }
        Ok(out)
    }

    /// Random-access read spanning chunk boundaries.
    ///
    /// Fills `buf` with bytes starting at `offset` within the file identified
    /// by `file_id`.  Reads continue across block boundaries until `buf` is
    /// full or EOF is reached.  Returns bytes copied.
    pub fn read_at(&mut self, file_id: u32, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let record = self.index.records.iter()
            .find(|r| r.id == file_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        if offset >= record.original_size || buf.is_empty() {
            return Ok(0);
        }

        let refs = record.block_refs.clone();
        let mut file_pos    = 0u64;
        let mut buf_written = 0usize;

        for br in &refs {
            if buf_written == buf.len() { break; }

            let block = self.decompress_ref(br)?;
            let block_len = block.len() as u64;
            let block_end = file_pos + block_len;

            // Skip blocks entirely before the requested offset.
            if block_end <= offset {
                file_pos = block_end;
                continue;
            }

            let read_start = if offset > file_pos {
                (offset - file_pos) as usize
            } else {
                // Offset is before or at this block start — cover the overlap.
                0
            };
            let to_copy = (buf.len() - buf_written).min(block.len() - read_start);
            buf[buf_written..buf_written + to_copy]
                .copy_from_slice(&block[read_start..read_start + to_copy]);

            buf_written += to_copy;
            file_pos     = block_end;
        }

        Ok(buf_written)
    }
}
