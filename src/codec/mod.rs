//! Codec registry: frozen UUID identities + optional short-ID fast path.
//!
//! # Identity rules
//! Every codec is identified by a 16-byte UUID.  That UUID is:
//!   - Written into every block header on disk.
//!   - Declared in the superblock's `required_codecs` list.
//!   - The authoritative identity for plugin registration.
//!
//! Short IDs (u16) are an *in-process* fast path only.  They are never
//! written to disk in place of UUIDs, and are never negotiated at runtime.
//! A reader that cannot supply every required UUID MUST fail immediately.
//!
//! # Endianness
//! All codec IDs on disk are the raw 16 bytes of the UUID in little-endian
//! field order (RFC 4122 §4.1.2 wire format).  This is non-negotiable.

use std::io::{self, Read, Write};
use thiserror::Error;

// ── Frozen codec UUIDs ──────────────────────────────────────────────────────
//
// These values are permanent.  A UUID is NEVER reused, even if a codec is
// deprecated.  Parsers MUST reject unknown UUIDs unless the block is not in
// `required_codecs` (in which case the block can be skipped).

/// No compression — payload stored verbatim.
pub const UUID_NONE:   [u8; 16] = [
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
];
/// Zstandard — balanced speed/ratio (default).
/// UUID: b28a9d4f-5e3c-4a1b-8f2e-7c6d9b0e1a2f  (LE bytes)
pub const UUID_ZSTD:   [u8; 16] = [
    0x4f,0x9d,0x8a,0xb2, 0x3c,0x5e, 0x1b,0x4a,
    0x8f,0x2e, 0x7c,0x6d,0x9b,0x0e,0x1a,0x2f,
];
/// LZ4 — maximum throughput, lower ratio.
/// UUID: 3f7b2c8e-1a4d-4e9f-b6c3-5d8a2f7e0b1c  (LE bytes)
pub const UUID_LZ4:    [u8; 16] = [
    0x8e,0x2c,0x7b,0x3f, 0x4d,0x1a, 0x9f,0x4e,
    0xb6,0xc3, 0x5d,0x8a,0x2f,0x7e,0x0b,0x1c,
];
/// Brotli — high ratio, optimised for text/web content.
/// UUID: 9c1e5f3a-7b2d-4c8e-a5f1-2e6b9d0c3a7f  (LE bytes)
pub const UUID_BROTLI: [u8; 16] = [
    0x3a,0x5f,0x1e,0x9c, 0x2d,0x7b, 0x8e,0x4c,
    0xa5,0xf1, 0x2e,0x6b,0x9d,0x0c,0x3a,0x7f,
];
/// LZMA — highest ratio, slowest codec.
/// UUID: 4a8f2e1c-9b3d-4f7a-c2e8-6d5b1a0f3c9e  (LE bytes)
pub const UUID_LZMA:   [u8; 16] = [
    0x1c,0x2e,0x8f,0x4a, 0x3d,0x9b, 0x7a,0x4f,
    0xc2,0xe8, 0x6d,0x5b,0x1a,0x0f,0x3c,0x9e,
];

// ── Short IDs (in-process only, never written to disk) ───────────────────────

/// In-process numeric alias for a codec. Advisory only.
/// Value 0 means "no short ID assigned / use UUID lookup".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortId(pub u16);

pub const SHORT_NONE:   ShortId = ShortId(0);
pub const SHORT_ZSTD:   ShortId = ShortId(1);
pub const SHORT_LZ4:    ShortId = ShortId(2);
pub const SHORT_BROTLI: ShortId = ShortId(3);
pub const SHORT_LZMA:   ShortId = ShortId(4);

// ── CodecId enum ─────────────────────────────────────────────────────────────

/// Runtime codec discriminant.  Carries both the frozen UUID and an optional
/// in-process short ID for fast dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecId {
    None,
    Zstd,
    Lz4,
    Brotli,
    Lzma,
}

impl CodecId {
    /// Returns the frozen 16-byte UUID for this codec.
    /// This is the value written to disk and declared in the superblock.
    #[inline]
    pub fn uuid(self) -> [u8; 16] {
        match self {
            CodecId::None   => UUID_NONE,
            CodecId::Zstd   => UUID_ZSTD,
            CodecId::Lz4    => UUID_LZ4,
            CodecId::Brotli => UUID_BROTLI,
            CodecId::Lzma   => UUID_LZMA,
        }
    }

    /// Returns the in-process short ID (advisory only, never written to disk).
    #[inline]
    pub fn short_id(self) -> ShortId {
        match self {
            CodecId::None   => SHORT_NONE,
            CodecId::Zstd   => SHORT_ZSTD,
            CodecId::Lz4    => SHORT_LZ4,
            CodecId::Brotli => SHORT_BROTLI,
            CodecId::Lzma   => SHORT_LZMA,
        }
    }

    /// Resolve a UUID to a CodecId.
    /// Returns `None` if the UUID is not recognised by this build.
    pub fn from_uuid(uuid: &[u8; 16]) -> Option<Self> {
        match uuid {
            u if u == &UUID_NONE   => Some(CodecId::None),
            u if u == &UUID_ZSTD   => Some(CodecId::Zstd),
            u if u == &UUID_LZ4    => Some(CodecId::Lz4),
            u if u == &UUID_BROTLI => Some(CodecId::Brotli),
            u if u == &UUID_LZMA   => Some(CodecId::Lzma),
            _                      => None,
        }
    }

    /// Human-readable name (for diagnostics only — never parsed).
    pub fn name(self) -> &'static str {
        match self {
            CodecId::None   => "none",
            CodecId::Zstd   => "zstd",
            CodecId::Lz4    => "lz4",
            CodecId::Brotli => "brotli",
            CodecId::Lzma   => "lzma",
        }
    }

    /// Parse from a CLI string.
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none"   => Some(CodecId::None),
            "zstd"   => Some(CodecId::Zstd),
            "lz4"    => Some(CodecId::Lz4),
            "brotli" => Some(CodecId::Brotli),
            "lzma"   => Some(CodecId::Lzma),
            _        => None,
        }
    }

    /// Format the codec UUID as a hyphenated string (diagnostics only).
    pub fn uuid_str(self) -> String {
        uuid_to_string(&self.uuid())
    }
}

/// Format a raw 16-byte UUID (LE field order) as `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`.
pub fn uuid_to_string(bytes: &[u8; 16]) -> String {
    // Undo LE field order to get the canonical display order:
    // fields: time_low(4 BE), time_mid(2 BE), time_hi(2 BE), clock_seq(2), node(6)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[3],bytes[2],bytes[1],bytes[0],
        bytes[5],bytes[4],
        bytes[7],bytes[6],
        bytes[8],bytes[9],
        bytes[10],bytes[11],bytes[12],bytes[13],bytes[14],bytes[15],
    )
}

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum CodecError {
    #[error("Compression error: {0}")]
    Compression(String),
    #[error("Decompression error: {0}")]
    Decompression(String),
    #[error("Encryption error: {0}")]
    Encryption(String),
    /// Emitted when a required codec UUID is not available in this build.
    /// The UUID is formatted for display; decoding MUST NOT continue.
    #[error("Required codec not available (UUID {uuid}) — cannot decode without it")]
    UnavailableCodec { uuid: String },
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

// ── Codec trait ──────────────────────────────────────────────────────────────

pub trait Codec: Send + Sync {
    fn codec_id(&self) -> CodecId;
    fn compress(&self, data: &[u8], level: i32) -> Result<Vec<u8>, CodecError>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError>;
}

// ── Built-in codec implementations ──────────────────────────────────────────

pub struct NoneCodec;
impl Codec for NoneCodec {
    fn codec_id(&self) -> CodecId { CodecId::None }
    fn compress(&self, data: &[u8], _: i32) -> Result<Vec<u8>, CodecError> { Ok(data.to_vec()) }
    fn decompress(&self, data: &[u8])        -> Result<Vec<u8>, CodecError> { Ok(data.to_vec()) }
}

pub struct ZstdCodec;
impl Codec for ZstdCodec {
    fn codec_id(&self) -> CodecId { CodecId::Zstd }
    fn compress(&self, data: &[u8], level: i32) -> Result<Vec<u8>, CodecError> {
        zstd::encode_all(data, level).map_err(|e| CodecError::Compression(e.to_string()))
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        zstd::decode_all(data).map_err(|e| CodecError::Decompression(e.to_string()))
    }
}

pub struct Lz4Codec;
impl Codec for Lz4Codec {
    fn codec_id(&self) -> CodecId { CodecId::Lz4 }
    fn compress(&self, data: &[u8], _: i32) -> Result<Vec<u8>, CodecError> {
        Ok(lz4_flex::compress_prepend_size(data))
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        lz4_flex::decompress_size_prepended(data)
            .map_err(|e| CodecError::Decompression(e.to_string()))
    }
}

pub struct BrotliCodec;
impl Codec for BrotliCodec {
    fn codec_id(&self) -> CodecId { CodecId::Brotli }
    fn compress(&self, data: &[u8], level: i32) -> Result<Vec<u8>, CodecError> {
        let quality = level.clamp(0, 11) as u32;
        let mut out = Vec::new();
        {
            let mut w = brotli::CompressorWriter::new(&mut out, 4096, quality, 22);
            w.write_all(data).map_err(|e| CodecError::Compression(e.to_string()))?;
        }
        Ok(out)
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::new();
        brotli::Decompressor::new(data, 4096)
            .read_to_end(&mut out)
            .map_err(|e| CodecError::Decompression(e.to_string()))?;
        Ok(out)
    }
}

pub struct LzmaCodec;
impl Codec for LzmaCodec {
    fn codec_id(&self) -> CodecId { CodecId::Lzma }
    fn compress(&self, data: &[u8], _: i32) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(data), &mut out)
            .map_err(|e| CodecError::Compression(e.to_string()))?;
        Ok(out)
    }
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::new();
        lzma_rs::lzma_decompress(&mut std::io::Cursor::new(data), &mut out)
            .map_err(|e| CodecError::Decompression(e.to_string()))?;
        Ok(out)
    }
}

// ── Factory ──────────────────────────────────────────────────────────────────

/// Resolve a UUID to a built-in codec.
///
/// Returns `Err(CodecError::UnavailableCodec)` if the UUID is not recognised.
/// The caller MUST NOT fall back to any other codec — fail hard.
pub fn get_codec_by_uuid(uuid: &[u8; 16]) -> Result<Box<dyn Codec>, CodecError> {
    match CodecId::from_uuid(uuid) {
        Some(id) => get_codec(id),
        None => Err(CodecError::UnavailableCodec {
            uuid: uuid_to_string(uuid),
        }),
    }
}

/// Resolve a CodecId to a built-in codec.
pub fn get_codec(id: CodecId) -> Result<Box<dyn Codec>, CodecError> {
    match id {
        CodecId::None   => Ok(Box::new(NoneCodec)),
        CodecId::Zstd   => Ok(Box::new(ZstdCodec)),
        CodecId::Lz4    => Ok(Box::new(Lz4Codec)),
        CodecId::Brotli => Ok(Box::new(BrotliCodec)),
        CodecId::Lzma   => Ok(Box::new(LzmaCodec)),
    }
}
