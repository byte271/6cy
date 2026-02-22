//! Index-bypass recovery scanner — reconstruct archives without the INDEX block.
//!
//! # How it works
//!
//! The scanner reads forward from `SUPERBLOCK_SIZE` (offset 256), inspecting
//! each 84-byte block header independently.  It does NOT need the INDEX block,
//! the RecoveryMap, or any prior state.  Every block is self-describing; the
//! scanner only needs the `header_crc32` to hold for a block to be usable.
//!
//! ## Recovery modes
//!
//! | Mode | Description |
//! |------|-------------|
//! | `Full` | All blocks intact; file list reconstructed completely |
//! | `Partial` | Some blocks corrupt/missing; files may be truncated |
//! | `HeaderOnly` | Only block headers survived; file list known but no data |
//! | `Catastrophic` | Fewer than 50% of headers valid; results unreliable |
//!
//! ## Block health
//!
//! Each scanned block receives a `BlockHealth` score:
//! - `Healthy` — header CRC32 valid, payload size plausible
//! - `HeaderCorrupt` — CRC32 mismatch; block is skipped
//! - `TruncatedPayload` — header valid but fewer bytes follow than `comp_size` declares
//! - `UnknownCodec` — header valid but `codec_uuid` is not in registry
//!
//! ## Progress
//!
//! `scan()` accepts an optional `ProgressFn` callback called after every block.
//! The callback receives `(bytes_scanned, total_bytes_estimate)`.
//! Pass `None` to disable progress reporting.

use std::io::{self, Read, Seek, SeekFrom};
use std::collections::HashMap;

use crate::block::{BlockHeader, BlockType, BLOCK_HEADER_SIZE};
use crate::codec::CodecId;
use crate::index::{FileIndex, FileIndexRecord, BlockRef};
use crate::superblock::SUPERBLOCK_SIZE;

// ── Types ─────────────────────────────────────────────────────────────────────

/// The health verdict for one scanned block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockHealth {
    /// Header CRC32 valid; payload reachable.
    Healthy,
    /// Header CRC32 failed — block cannot be trusted.
    HeaderCorrupt,
    /// Header valid but fewer bytes follow than `comp_size` declares.
    TruncatedPayload { declared: u32, available: u64 },
    /// Header valid, codec UUID not in this build's registry.
    UnknownCodec { uuid_hex: String },
}

impl BlockHealth {
    pub fn is_usable(&self) -> bool {
        matches!(self, BlockHealth::Healthy)
    }
}

/// Diagnostic record for one scanned block position.
#[derive(Debug, Clone)]
pub struct ScannedBlock {
    /// Absolute byte offset of this block header in the archive.
    pub archive_offset: u64,
    /// Parsed header (available even when health is `HeaderCorrupt` for
    /// partial diagnostics — may contain garbage fields in that case).
    pub header:         Option<BlockHeader>,
    /// Health verdict.
    pub health:         BlockHealth,
}

impl ScannedBlock {
    pub fn is_usable(&self) -> bool {
        self.health.is_usable() && self.header.is_some()
    }
}

/// Overall quality of the recovery scan result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryQuality {
    /// All blocks healthy; file list complete.
    Full,
    /// Some blocks corrupt; some files may be truncated.
    Partial,
    /// No payload data recoverable; file list only.
    HeaderOnly,
    /// <50% of blocks healthy; results unreliable.
    Catastrophic,
}

/// Complete report produced by `scan()`.
#[derive(Debug)]
pub struct RecoveryReport {
    /// Total blocks scanned (healthy + corrupt).
    pub total_scanned:   usize,
    /// Blocks that passed the header CRC32 check.
    pub healthy_blocks:  usize,
    /// Blocks with a bad header CRC32.
    pub corrupt_blocks:  usize,
    /// Blocks with a valid header but truncated payload.
    pub truncated_blocks: usize,
    /// Blocks with an unrecognised codec UUID.
    pub unknown_codec_blocks: usize,
    /// Bytes of archive file scanned.
    pub bytes_scanned:   u64,
    /// Per-block diagnostic records.
    pub block_log:       Vec<ScannedBlock>,
    /// Reconstructed file index (may be partial).
    pub index:           FileIndex,
    /// Estimated total bytes recoverable (sum of `orig_size` for healthy blocks).
    pub recoverable_bytes: u64,
    /// Overall quality rating.
    pub quality:         RecoveryQuality,
}

impl RecoveryReport {
    /// Percentage of blocks that are healthy (0.0–100.0).
    pub fn health_pct(&self) -> f64 {
        if self.total_scanned == 0 { return 100.0; }
        self.healthy_blocks as f64 / self.total_scanned as f64 * 100.0
    }

    /// Summary line for display.
    pub fn summary(&self) -> String {
        format!(
            "{:?} recovery: {}/{} blocks healthy ({:.1}%), \
             {} file(s) reconstructed, {:.2} MiB recoverable",
            self.quality,
            self.healthy_blocks,
            self.total_scanned,
            self.health_pct(),
            self.index.records.len(),
            self.recoverable_bytes as f64 / 1024.0 / 1024.0,
        )
    }
}

// ── Progress callback ─────────────────────────────────────────────────────────

pub type ProgressFn<'a> = dyn FnMut(u64 /*scanned*/, u64 /*total_estimate*/) + 'a;

// ── Scanner ───────────────────────────────────────────────────────────────────

/// Scan an archive stream for recoverable blocks without using the INDEX block.
///
/// # Arguments
/// * `reader`         — seekable stream positioned anywhere; will be rewound.
/// * `file_size_hint` — archive file size (for progress estimation). Pass 0 to skip.
/// * `progress`       — optional progress callback; called after each block.
///
/// # Returns
/// A [`RecoveryReport`] regardless of how many blocks are readable.  This
/// function does not return `Err` due to corrupt data — all errors are encoded
/// as `BlockHealth` variants in the report.  Only genuine I/O errors (e.g.,
/// permission denied) propagate as `io::Error`.
pub fn scan<R, F>(
    reader:         &mut R,
    file_size_hint: u64,
    mut progress:   Option<&mut F>,
) -> io::Result<RecoveryReport>
where
    R: Read + Seek,
    F: FnMut(u64, u64),
{
    reader.seek(SeekFrom::Start(SUPERBLOCK_SIZE as u64))?;

    // Per-file chunk accumulation: file_id → Vec<(file_offset, ScannedBlock)>
    let mut chunks: HashMap<u32, Vec<(u64, ScannedBlock)>> = HashMap::new();
    let mut orig_sizes: HashMap<u32, u64> = HashMap::new();
    let mut block_log: Vec<ScannedBlock> = Vec::new();

    let mut total_scanned        = 0usize;
    let mut healthy_blocks       = 0usize;
    let mut corrupt_blocks       = 0usize;
    let mut truncated_blocks     = 0usize;
    let mut unknown_codec_blocks = 0usize;
    let mut recoverable_bytes    = 0u64;
    let mut bytes_scanned        = SUPERBLOCK_SIZE as u64;

    loop {
        let pos = reader.stream_position()?;

        // Try to read a full 84-byte header.
        let mut hdr_buf = [0u8; BLOCK_HEADER_SIZE];
        match reader.read_exact(&mut hdr_buf) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        bytes_scanned += BLOCK_HEADER_SIZE as u64;
        total_scanned += 1;

        // Attempt to parse.  BlockHeader::read re-reads from a cursor; we
        // already have the bytes, so parse from the buffer directly.
        let parse_result = BlockHeader::read(std::io::Cursor::new(&hdr_buf));

        match parse_result {
            Err(_) => {
                // Header CRC32 or magic failed.
                corrupt_blocks += 1;
                let sb = ScannedBlock {
                    archive_offset: pos,
                    header: None,
                    health: BlockHealth::HeaderCorrupt,
                };
                block_log.push(sb);

                // Skip one byte and retry — allows finding the next valid
                // header after a run of corruption.
                reader.seek(SeekFrom::Start(pos + 1))?;
                bytes_scanned = pos + 1 + BLOCK_HEADER_SIZE as u64; // approx
            }
            Ok(header) => {
                // Header parsed.  Now assess codec and payload availability.
                let comp_size  = header.comp_size as u64;
                let block_type = header.block_type;

                // Check codec availability.
                let health = if CodecId::from_uuid(&header.codec_uuid).is_none()
                    && header.codec_uuid != crate::codec::UUID_NONE
                {
                    unknown_codec_blocks += 1;
                    BlockHealth::UnknownCodec {
                        uuid_hex: hex::encode(&header.codec_uuid),
                    }
                } else {
                    // Check payload truncation.
                    let stream_pos = reader.stream_position()?;
                    let remaining  = if file_size_hint > stream_pos {
                        file_size_hint - stream_pos
                    } else {
                        // Seek to end to get remaining bytes.
                        let end = reader.seek(SeekFrom::End(0))?;
                        let _ = reader.seek(SeekFrom::Start(stream_pos))?;
                        if end > stream_pos { end - stream_pos } else { 0 }
                    };

                    if remaining < comp_size {
                        truncated_blocks += 1;
                        BlockHealth::TruncatedPayload {
                            declared:  header.comp_size,
                            available: remaining,
                        }
                    } else {
                        healthy_blocks   += 1;
                        recoverable_bytes += header.orig_size as u64;
                        BlockHealth::Healthy
                    }
                };

                let usable = health.is_usable() && block_type == BlockType::Data;

                // Record in per-file accumulator if usable DATA block.
                if usable {
                    let fid = header.file_id;
                    let end = header.file_offset + header.orig_size as u64;
                    let sz  = orig_sizes.entry(fid).or_insert(0);
                    if end > *sz { *sz = end; }

                    let sb = ScannedBlock {
                        archive_offset: pos,
                        header: Some(header.clone()),
                        health: health.clone(),
                    };
                    chunks.entry(fid).or_default().push((header.file_offset, sb));
                }

                let sb = ScannedBlock {
                    archive_offset: pos,
                    header: Some(header.clone()),
                    health,
                };
                block_log.push(sb);

                // Seek past payload.
                if reader.seek(SeekFrom::Current(comp_size as i64)).is_err() {
                    break;
                }
                bytes_scanned += comp_size;

                // Stop at INDEX block — it marks the end of data blocks.
                if block_type == BlockType::Index {
                    break;
                }
            }
        }

        // Progress callback.
        if let Some(ref mut cb) = progress {
            let estimate = if file_size_hint > 0 { file_size_hint } else { bytes_scanned * 2 };
            cb(bytes_scanned, estimate);
        }
    }

    // Build FileIndexRecords from accumulated chunks.
    let mut records: Vec<FileIndexRecord> = chunks
        .into_iter()
        .map(|(fid, mut v)| {
            v.sort_by_key(|(off, _)| *off);
            let refs: Vec<BlockRef> = v
                .into_iter()
                .map(|(_, sb)| BlockRef {
                    content_hash:   sb.header.as_ref().map(|h| h.content_hash).unwrap_or([0u8; 32]),
                    archive_offset: sb.archive_offset,
                    intra_offset:   0,
                    intra_length:   0,
                })
                .collect();
            let size = *orig_sizes.get(&fid).unwrap_or(&0);
            FileIndexRecord::from_scan(fid, size, refs)
        })
        .collect();
    records.sort_by_key(|r| r.id);

    let mut index = FileIndex { records, root_hash: [0u8; 32] };
    index.compute_root_hash();

    // Determine quality.
    let quality = if total_scanned == 0 {
        RecoveryQuality::Catastrophic
    } else {
        let pct = healthy_blocks as f64 / total_scanned as f64;
        match (index.records.is_empty(), pct) {
            (true, _) => RecoveryQuality::HeaderOnly,
            (_, p) if p >= 0.95 => RecoveryQuality::Full,
            (_, p) if p >= 0.50 => RecoveryQuality::Partial,
            _ => RecoveryQuality::Catastrophic,
        }
    };

    Ok(RecoveryReport {
        total_scanned,
        healthy_blocks,
        corrupt_blocks,
        truncated_blocks,
        unknown_codec_blocks,
        bytes_scanned,
        block_log,
        index,
        recoverable_bytes,
        quality,
    })
}

/// Convenience: scan a file at `path` and return the report.
pub fn scan_file(path: &std::path::Path) -> io::Result<RecoveryReport> {
    let mut f    = std::fs::File::open(path)?;
    let size     = f.metadata()?.len();
    scan::<_, fn(u64, u64)>(&mut f, size, None)
}

/// Extract all recoverable DATA blocks from `src` into new archive `dst`.
///
/// Only `Healthy` DATA blocks are copied.  The resulting archive will have a
/// fresh superblock and index built from the recovered blocks.
///
/// Returns the [`RecoveryReport`] from scanning `src`.
pub fn extract_recoverable<R, W>(
    src:            &mut R,
    dst:            &mut W,
    decryption_key: Option<&[u8; 32]>,
) -> io::Result<RecoveryReport>
where
    R: Read + Seek,
    W: std::io::Write + Seek,
{
    use crate::io_stream::{SixCyWriter, DEFAULT_COMPRESSION_LEVEL};
    use crate::codec::CodecId;
    use crate::block::decode_block;

    let size   = src.seek(SeekFrom::End(0))?;
    let report = scan::<_, fn(u64, u64)>(src, size, None)?;

    let mut writer = SixCyWriter::with_options(
        dst,
        4 * 1024 * 1024,
        DEFAULT_COMPRESSION_LEVEL,
        None,
    )?;

    // Group healthy blocks by file_id and sort by file_offset.
    let mut by_file: HashMap<u32, Vec<&ScannedBlock>> = HashMap::new();
    for sb in report.block_log.iter().filter(|sb| sb.is_usable()) {
        if let Some(h) = &sb.header {
            if h.block_type == BlockType::Data {
                by_file.entry(h.file_id).or_default().push(sb);
            }
        }
    }

    let mut file_ids: Vec<u32> = by_file.keys().copied().collect();
    file_ids.sort_unstable();

    for fid in file_ids {
        let mut blocks = by_file.remove(&fid).unwrap();
        blocks.sort_by_key(|sb| sb.header.as_ref().map(|h| h.file_offset).unwrap_or(0));

        let name = format!("recovered_file_{fid:08x}");
        let mut data: Vec<u8> = Vec::new();

        for sb in blocks {
            let h = sb.header.as_ref().unwrap();
            src.seek(SeekFrom::Start(sb.archive_offset + crate::block::BLOCK_HEADER_SIZE as u64))?;
            let mut payload = vec![0u8; h.comp_size as usize];
            src.read_exact(&mut payload)?;

            match decode_block(h, &payload, decryption_key) {
                Ok(chunk) => data.extend(chunk),
                Err(_)    => {
                    // Decompression failed despite header being valid — skip.
                    continue;
                }
            }
        }

        if !data.is_empty() {
            writer.add_file(name, &data, CodecId::Zstd)?;
        }
    }

    writer.finalize()?;
    Ok(report)
}
