//! # Zenith Archive Protocol v2.0.0
//!
//! The immutable anchor located at offset 0.
//!
//! Version 2 Introduces Zenith-Grade resilience flags and Polymorphic Magic.

use crate::codec::{uuid_to_string, CodecId};
use crc32fast::Hasher;
use std::io::{self, Read, Write};
use thiserror::Error;
use uuid::Uuid;

pub const MAGIC: &[u8; 4] = b".6cy";
pub const FORMAT_VERSION: u32 = 2;
pub const MIN_FORMAT_VERSION: u32 = 1;
pub const SUPERBLOCK_SIZE: usize = 512; // Expanded for Zenith metadata

// ── Zenith Resilience Flags ──────────────────────────────────────────────────

pub const FLAG_ENCRYPTED: u32 = 0x0001;
pub const FLAG_SHARDED: u32 = 0x0002;
pub const FLAG_PQC_KEM: u32 = 0x0004;
pub const FLAG_VDF_SENTINEL: u32 = 0x0008;

#[derive(Error, Debug)]
pub enum SuperblockError {
    #[error("Polymorphic Magic Mismatch — Zenith header rejected")]
    InvalidMagic,
    #[error("Unsupported Zenith version {0} (min {MIN_FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("Sentinel CRC mismatch — archive integrity compromised")]
    Crc32Mismatch,
    #[error("Codec count {0} exceeds the superblock capacity")]
    TooManyCodecs(usize),
    #[error("Required Zenith Codec {uuid} is absent in this build")]
    UnavailableCodec { uuid: String },
    #[error("IO Collision: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone)]
pub struct Superblock {
    pub magic: [u8; 4],
    pub format_version: u32,
    pub archive_uuid: Uuid,
    pub flags: u32,
    pub index_offset: u64,
    pub index_size: u64,
    pub required_codec_uuids: Vec<[u8; 16]>,
}

impl Superblock {
    pub fn new() -> Self {
        Self {
            magic: *MAGIC,
            format_version: FORMAT_VERSION,
            archive_uuid: Uuid::new_v4(),
            flags: 0,
            index_offset: 0,
            index_size: 0,
            required_codec_uuids: Vec::new(),
        }
    }

    pub fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        let mut body = Vec::with_capacity(SUPERBLOCK_SIZE);

        body.extend_from_slice(&self.magic);
        body.extend_from_slice(&self.format_version.to_le_bytes());
        body.extend_from_slice(self.archive_uuid.as_bytes());
        body.extend_from_slice(&self.flags.to_le_bytes());
        body.extend_from_slice(&self.index_offset.to_le_bytes());
        body.extend_from_slice(&self.index_size.to_le_bytes());
        body.extend_from_slice(&(self.required_codec_uuids.len() as u16).to_le_bytes());
        for uuid_bytes in &self.required_codec_uuids {
            body.extend_from_slice(uuid_bytes);
        }

        let mut h = Hasher::new();
        h.update(&body);
        body.extend_from_slice(&h.finalize().to_le_bytes());

        assert!(body.len() <= SUPERBLOCK_SIZE, "Zenith Superblock overflow");
        body.resize(SUPERBLOCK_SIZE, 0u8);
        w.write_all(&body)
    }

    pub fn read<R: Read>(mut r: R) -> Result<Self, SuperblockError> {
        let mut buf = [0u8; SUPERBLOCK_SIZE];
        r.read_exact(&mut buf)?;

        if &buf[0..4] != MAGIC {
            return Err(SuperblockError::InvalidMagic);
        }

        let format_version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        if format_version < MIN_FORMAT_VERSION {
            return Err(SuperblockError::UnsupportedVersion(format_version));
        }

        let archive_uuid = Uuid::from_bytes(buf[8..24].try_into().unwrap());
        let flags = u32::from_le_bytes(buf[24..28].try_into().unwrap());
        let index_offset = u64::from_le_bytes(buf[28..36].try_into().unwrap());
        let index_size = u64::from_le_bytes(buf[36..44].try_into().unwrap());
        let codec_count = u16::from_le_bytes(buf[44..46].try_into().unwrap()) as usize;

        let uuid_end = 46 + codec_count * 16;
        if uuid_end + 4 > SUPERBLOCK_SIZE {
            return Err(SuperblockError::TooManyCodecs(codec_count));
        }
        let mut required_codec_uuids = Vec::with_capacity(codec_count);
        for i in 0..codec_count {
            let start = 46 + i * 16;
            required_codec_uuids.push(buf[start..start + 16].try_into().unwrap());
        }

        let stored_crc = u32::from_le_bytes(buf[uuid_end..uuid_end + 4].try_into().unwrap());
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
        sb.check_codecs()?;
        Ok(sb)
    }

    pub fn check_codecs(&self) -> Result<(), SuperblockError> {
        for u in &self.required_codec_uuids {
            if CodecId::from_uuid(u).is_none() {
                return Err(SuperblockError::UnavailableCodec {
                    uuid: uuid_to_string(u),
                });
            }
        }
        Ok(())
    }

    pub fn add_required_codec(&mut self, codec_id: CodecId) {
        if codec_id == CodecId::None {
            return;
        }
        let u = codec_id.uuid();
        if !self
            .required_codec_uuids
            .iter()
            .any(|existing| existing == &u)
        {
            self.required_codec_uuids.push(u);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crafted superblock whose codec_count overruns the fixed 512-byte
    /// buffer must be rejected, not panic on an out-of-bounds slice.
    #[test]
    fn read_rejects_oversized_codec_count() {
        let mut buf = vec![0u8; SUPERBLOCK_SIZE];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..8].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        buf[44..46].copy_from_slice(&u16::MAX.to_le_bytes());

        match Superblock::read(std::io::Cursor::new(buf)) {
            Err(SuperblockError::TooManyCodecs(count)) => {
                assert_eq!(count, u16::MAX as usize);
            }
            other => panic!("expected TooManyCodecs, got {other:?}"),
        }
    }

    /// The largest codec_count that still fits must not be rejected by the
    /// bounds check (it fails later on the CRC, proving the guard is not
    /// off-by-one and does not reject valid layouts).
    #[test]
    fn read_allows_max_fitting_codec_count() {
        let max_fit = (SUPERBLOCK_SIZE - 46 - 4) / 16;
        let mut buf = vec![0u8; SUPERBLOCK_SIZE];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..8].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        buf[44..46].copy_from_slice(&(max_fit as u16).to_le_bytes());

        assert!(!matches!(
            Superblock::read(std::io::Cursor::new(buf)),
            Err(SuperblockError::TooManyCodecs(_))
        ));
    }
}
