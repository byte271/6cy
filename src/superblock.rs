//! Superblock — format anchor at offset 0.
//!
//! # On-disk layout (256 bytes, all fields little-endian)
//!
//! ```text
//! Offset  Size  Field
//!    0      4   magic              = ".6cy"  (4 ASCII bytes, not LE)
//!    4      4   format_version     = 3       (LE u32)
//!    8     16   archive_uuid       unique per archive
//!   24      4   flags              0x01=encrypted  (LE u32)
//!   28      8   index_offset       byte offset of the INDEX block header (LE u64)
//!   36      8   index_size         compressed INDEX payload bytes (LE u64)
//!   44      2   required_codec_count (LE u16)
//!   46   N×16   required_codec_uuids (N × 16 raw bytes, LE field order)
//!  46+N×16  4   header_crc32       CRC32 of all preceding bytes (LE u32)
//!   ...    ...  zero padding to exactly 256 bytes
//! ```
//!
//! # Codec declaration
//! `required_codec_uuids` lists every codec UUID that appears in DATA or
//! SOLID blocks.  A decoder MUST fail immediately if it cannot supply every
//! listed UUID.  There is no negotiation, no fallback, no partial decode.
//! The UUID list is written during `finalize()`; it is empty while packing.
//!
//! # Endianness
//! All numeric fields are little-endian.  The magic is four ASCII bytes.
//! This is frozen for format_version 3 and above.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};
use uuid::Uuid;
use crc32fast::Hasher;
use thiserror::Error;
use crate::codec::{CodecId, uuid_to_string};

pub const MAGIC:              &[u8; 4] = b".6cy";
pub const FORMAT_VERSION:     u32      = 3;
pub const MIN_FORMAT_VERSION: u32      = 3;  // v1/v2 are not forward-compatible
pub const SUPERBLOCK_SIZE:    usize    = 256;

/// Archive-level flag: at least one block is AES-256-GCM encrypted.
pub const SB_FLAG_ENCRYPTED: u32 = 0x0001;

#[derive(Error, Debug)]
pub enum SuperblockError {
    #[error("Invalid magic number — not a .6cy archive")]
    InvalidMagic,
    #[error("Unsupported format version {0} (minimum supported: {MIN_FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("Superblock header_crc32 mismatch — file is corrupted")]
    Crc32Mismatch,
    /// Emitted when a required codec UUID is not provided by this build.
    /// The archive CANNOT be decoded; there is no fallback.
    #[error("Required codec UUID {uuid} is not available — cannot open archive")]
    UnavailableCodec { uuid: String },
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone)]
pub struct Superblock {
    pub magic:                 [u8; 4],
    pub format_version:        u32,
    pub archive_uuid:          Uuid,
    pub flags:                 u32,
    pub index_offset:          u64,
    pub index_size:            u64,
    /// Each entry is the raw 16-byte UUID (LE field order) of a required codec.
    /// Written during `finalize()`; empty while packing is in progress.
    pub required_codec_uuids:  Vec<[u8; 16]>,
}

impl Superblock {
    pub fn new() -> Self {
        Self {
            magic:                *MAGIC,
            format_version:       FORMAT_VERSION,
            archive_uuid:         Uuid::new_v4(),
            flags:                0,
            index_offset:         0,
            index_size:           0,
            required_codec_uuids: Vec::new(),
        }
    }

    /// Write the superblock and pad to exactly `SUPERBLOCK_SIZE` bytes.
    ///
    /// `header_crc32` covers all bytes from offset 0 up to (but not including)
    /// the CRC field itself.  The padding after the CRC is not covered.
    pub fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        // Build the variable-length portion in a buffer first so we can CRC it.
        let mut body = Vec::with_capacity(SUPERBLOCK_SIZE);

        body.extend_from_slice(&self.magic);                                       // 4
        body.extend_from_slice(&self.format_version.to_le_bytes());                // 4
        body.extend_from_slice(self.archive_uuid.as_bytes());                      // 16
        body.extend_from_slice(&self.flags.to_le_bytes());                         // 4
        body.extend_from_slice(&self.index_offset.to_le_bytes());                  // 8
        body.extend_from_slice(&self.index_size.to_le_bytes());                    // 8
        body.extend_from_slice(&(self.required_codec_uuids.len() as u16).to_le_bytes()); // 2
        for uuid_bytes in &self.required_codec_uuids {
            body.extend_from_slice(uuid_bytes);                                    // 16 each
        }
        // Fixed pre-CRC size: 4+4+16+4+8+8+2 = 46; + 16*n for codecs.

        // Compute CRC32 of everything so far and append it.
        let mut h = Hasher::new();
        h.update(&body);
        body.extend_from_slice(&h.finalize().to_le_bytes()); // 4

        // Pad to exactly SUPERBLOCK_SIZE with zeros.
        assert!(body.len() <= SUPERBLOCK_SIZE,
            "Superblock body {} B exceeds reserved {} B — too many required codecs",
            body.len(), SUPERBLOCK_SIZE);
        body.resize(SUPERBLOCK_SIZE, 0u8);

        w.write_all(&body)
    }

    /// Read, validate magic, version, and CRC32, then check codec availability.
    ///
    /// Returns `UnavailableCodec` if any required UUID is not in this build.
    /// The caller MUST NOT attempt to decode blocks in that case.
    pub fn read<R: Read>(mut r: R) -> Result<Self, SuperblockError> {
        let mut buf = [0u8; SUPERBLOCK_SIZE];
        r.read_exact(&mut buf)?;

        // Magic.
        if &buf[0..4] != MAGIC {
            return Err(SuperblockError::InvalidMagic);
        }

        // Version — fail hard if below minimum.
        let format_version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        if format_version < MIN_FORMAT_VERSION {
            return Err(SuperblockError::UnsupportedVersion(format_version));
        }

        let archive_uuid = Uuid::from_bytes(buf[8..24].try_into().unwrap());
        let flags        = u32::from_le_bytes(buf[24..28].try_into().unwrap());
        let index_offset = u64::from_le_bytes(buf[28..36].try_into().unwrap());
        let index_size   = u64::from_le_bytes(buf[36..44].try_into().unwrap());
        let codec_count  = u16::from_le_bytes(buf[44..46].try_into().unwrap()) as usize;

        // Parse codec UUIDs.
        let uuid_end = 46 + codec_count * 16;
        if uuid_end + 4 > SUPERBLOCK_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData,
                "required_codec_count overflows superblock").into());
        }
        let mut required_codec_uuids = Vec::with_capacity(codec_count);
        for i in 0..codec_count {
            let start = 46 + i * 16;
            let u: [u8; 16] = buf[start..start+16].try_into().unwrap();
            required_codec_uuids.push(u);
        }

        // Verify CRC32 — covers buf[0..uuid_end].
        let stored_crc   = u32::from_le_bytes(buf[uuid_end..uuid_end+4].try_into().unwrap());
        let mut h = Hasher::new();
        h.update(&buf[..uuid_end]);
        if h.finalize() != stored_crc {
            return Err(SuperblockError::Crc32Mismatch);
        }

        let sb = Self {
            magic: *MAGIC,
            format_version,
            archive_uuid,
            flags,
            index_offset,
            index_size,
            required_codec_uuids,
        };

        // Codec availability check — fail now, not at block decode time.
        sb.check_codecs()?;

        Ok(sb)
    }

    /// Verify that every required codec UUID is available in this build.
    /// Returns the first unavailable UUID if any are missing.
    pub fn check_codecs(&self) -> Result<(), SuperblockError> {
        for uuid_bytes in &self.required_codec_uuids {
            if CodecId::from_uuid(uuid_bytes).is_none() {
                return Err(SuperblockError::UnavailableCodec {
                    uuid: uuid_to_string(uuid_bytes),
                });
            }
        }
        Ok(())
    }

    /// Register a codec UUID as required (called by the writer when a new
    /// codec appears in a block).  Duplicate entries are deduplicated.
    pub fn add_required_codec(&mut self, codec_id: CodecId) {
        if codec_id == CodecId::None {
            return; // None codec requires no decoder capability
        }
        let uuid = codec_id.uuid();
        if !self.required_codec_uuids.iter().any(|u| u == &uuid) {
            self.required_codec_uuids.push(uuid);
        }
    }
}
