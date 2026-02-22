//! Block format v1 — fully self-describing, mandatory checksums.
//!
//! # On-disk layout (84 bytes header, all fields little-endian)
//!
//! ```text
//! Offset  Size  Field
//!    0      4   magic        = 0x424C434B  ("BLCK", LE u32)
//!    4      2   header_version = 1         (LE u16, bumped on layout change)
//!    6      2   header_size  = 84          (LE u16, skip unknown extensions)
//!    8      2   block_type   0=Data 1=Index 2=Solid  (LE u16)
//!   10      2   flags        0x0001=Encrypted        (LE u16)
//!   12     16   codec_uuid   frozen 16-byte UUID     (LE field order)
//!   28      4   file_id      0xFFFF_FFFF = solid/idx (LE u32)
//!   32      8   file_offset  in decompressed file    (LE u64)
//!   40      4   orig_size    uncompressed bytes      (LE u32)
//!   44      4   comp_size    on-disk bytes           (LE u32)
//!   48     32   content_hash BLAKE3 of plaintext
//!   80      4   header_crc32 CRC32([0..80])  ← LAST   (LE u32)
//! ```
//!
//! # Endianness
//! Every numeric field is little-endian.  This is non-negotiable and encoded
//! in the format version.  A future big-endian variant would carry a distinct
//! magic number.
//!
//! # Checksums
//! `header_crc32` covers all 80 bytes before it.  This detects header
//! corruption before any seek or allocation is attempted.  Payload integrity
//! is verified separately via `content_hash` (BLAKE3 of uncompressed data)
//! after decompression.  Both checks are mandatory; there is no opt-out.
//!
//! # Index reconstruction
//! Every DATA block embeds `file_id`, `file_offset`, `orig_size`, and
//! `content_hash`.  A scanner can rebuild the full block list by reading
//! headers sequentially without decompressing payloads.  Solid blocks and the
//! Index block must still be parsed for file-name recovery; see `io_stream`.

use std::io::{self, Read, Write};
use crate::codec::{CodecId, get_codec_by_uuid, CodecError, uuid_to_string};
use crc32fast::Hasher;

// ── Constants ────────────────────────────────────────────────────────────────

/// On-disk magic for every block header.  LE u32.
pub const BLOCK_MAGIC: u32 = 0x424C_434B;  // "BLCK"

/// Current block header layout version.
pub const BLOCK_HEADER_VERSION: u16 = 1;

/// Fixed byte size of the block header (including the trailing header_crc32).
pub const BLOCK_HEADER_SIZE: usize = 84;

/// `file_id` sentinel: this block does not belong to a single file.
pub const FILE_ID_SHARED: u32 = 0xFFFF_FFFF;

// ── Block type ───────────────────────────────────────────────────────────────

/// Discriminates the role of a block within the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlockType {
    /// Normal data block (one chunk of one file).
    Data  = 0,
    /// Index block — payload is the file-name/metadata table.
    Index = 1,
    /// Solid block — payload contains multiple concatenated files.
    Solid = 2,
}

impl BlockType {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0 => Some(BlockType::Data),
            1 => Some(BlockType::Index),
            2 => Some(BlockType::Solid),
            _ => None,
        }
    }
}

// ── Flags ────────────────────────────────────────────────────────────────────

/// Payload is AES-256-GCM encrypted (nonce prepended).
pub const FLAG_ENCRYPTED: u16 = 0x0001;

// ── Block header ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BlockHeader {
    // Structural
    pub header_version: u16,           // = BLOCK_HEADER_VERSION
    pub block_type:     BlockType,
    pub flags:          u16,
    // Codec identity — UUID is authoritative, never negotiated
    pub codec_uuid:     [u8; 16],
    // Data location
    pub file_id:        u32,
    pub file_offset:    u64,
    // Sizes
    pub orig_size:      u32,           // uncompressed
    pub comp_size:      u32,           // on-disk (post compress + encrypt)
    // Integrity
    pub content_hash:   [u8; 32],      // BLAKE3 of uncompressed plaintext
    // header_crc32 is computed/verified internally — not stored as a field
    // to prevent callers from accidentally setting it to a wrong value.
}

impl BlockHeader {
    /// Write the 84-byte header.  `header_crc32` is computed here.
    pub fn write<W: Write>(&self, mut w: W) -> io::Result<()> {
        // Accumulate into a buffer so we can CRC it all at once.
        let mut buf = [0u8; BLOCK_HEADER_SIZE];
        let mut pos = 0;

        macro_rules! put_u32le { ($v:expr) => {{
            buf[pos..pos+4].copy_from_slice(&($v as u32).to_le_bytes()); pos += 4;
        }}}
        macro_rules! put_u16le { ($v:expr) => {{
            buf[pos..pos+2].copy_from_slice(&($v as u16).to_le_bytes()); pos += 2;
        }}}
        macro_rules! put_u64le { ($v:expr) => {{
            buf[pos..pos+8].copy_from_slice(&($v as u64).to_le_bytes()); pos += 8;
        }}}
        macro_rules! put_bytes { ($b:expr) => {{
            let b: &[u8] = $b; buf[pos..pos+b.len()].copy_from_slice(b); pos += b.len();
        }}}

        put_u32le!(BLOCK_MAGIC);
        put_u16le!(BLOCK_HEADER_VERSION);
        put_u16le!(BLOCK_HEADER_SIZE as u16);
        put_u16le!(self.block_type as u16);
        put_u16le!(self.flags);
        put_bytes!(&self.codec_uuid);
        put_u32le!(self.file_id);
        put_u64le!(self.file_offset);
        put_u32le!(self.orig_size);
        put_u32le!(self.comp_size);
        put_bytes!(&self.content_hash);

        assert_eq!(pos, 80, "header body must be exactly 80 bytes before CRC");

        // Compute and append header_crc32 over the preceding 80 bytes.
        let mut h = Hasher::new();
        h.update(&buf[..80]);
        let crc = h.finalize();
        buf[80..84].copy_from_slice(&crc.to_le_bytes());

        w.write_all(&buf)
    }

    /// Read and validate an 84-byte block header.
    ///
    /// Returns `Err(InvalidData)` on any mismatch — magic, version, CRC32, or
    /// an unknown block type.  The caller MUST NOT attempt payload reads if
    /// this returns an error.
    pub fn read<R: Read>(mut r: R) -> io::Result<Self> {
        let mut buf = [0u8; BLOCK_HEADER_SIZE];
        r.read_exact(&mut buf)?;

        // 1. Verify header CRC32 first — cheapest possible check.
        let mut h = Hasher::new();
        h.update(&buf[..80]);
        let expected_crc = h.finalize();
        let stored_crc   = u32::from_le_bytes(buf[80..84].try_into().unwrap());
        if stored_crc != expected_crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Block header CRC32 mismatch: expected {expected_crc:#010x}, got {stored_crc:#010x}"),
            ));
        }

        // 2. Validate magic.
        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if magic != BLOCK_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid block magic: expected {BLOCK_MAGIC:#010x}, got {magic:#010x}"),
            ));
        }

        // 3. Validate header version — we know how to read v1.
        let header_version = u16::from_le_bytes(buf[4..6].try_into().unwrap());
        if header_version != BLOCK_HEADER_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported block header version {header_version} (this build handles v{BLOCK_HEADER_VERSION})"),
            ));
        }

        // 4. header_size lets future readers skip extensions we don't know.
        let header_size = u16::from_le_bytes(buf[6..8].try_into().unwrap());
        if (header_size as usize) < BLOCK_HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Block header_size {header_size} < minimum {BLOCK_HEADER_SIZE}"),
            ));
        }

        // 5. Parse block type.
        let block_type_raw = u16::from_le_bytes(buf[8..10].try_into().unwrap());
        let block_type = BlockType::from_u16(block_type_raw).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData,
                format!("Unknown block_type {block_type_raw}"))
        })?;

        let flags       = u16::from_le_bytes(buf[10..12].try_into().unwrap());
        let codec_uuid: [u8; 16] = buf[12..28].try_into().unwrap();
        let file_id     = u32::from_le_bytes(buf[28..32].try_into().unwrap());
        let file_offset = u64::from_le_bytes(buf[32..40].try_into().unwrap());
        let orig_size   = u32::from_le_bytes(buf[40..44].try_into().unwrap());
        let comp_size   = u32::from_le_bytes(buf[44..48].try_into().unwrap());
        let content_hash: [u8; 32] = buf[48..80].try_into().unwrap();

        Ok(Self {
            header_version,
            block_type,
            flags,
            codec_uuid,
            file_id,
            file_offset,
            orig_size,
            comp_size,
            content_hash,
        })
    }

    #[inline] pub fn is_encrypted(&self) -> bool { self.flags & FLAG_ENCRYPTED != 0 }
    #[inline] pub fn codec_id(&self)     -> Option<CodecId> { CodecId::from_uuid(&self.codec_uuid) }
    #[inline] pub fn codec_uuid_str(&self) -> String { uuid_to_string(&self.codec_uuid) }
}

// ── encode_block ──────────────────────────────────────────────────────────────

/// Compress (and optionally encrypt) a chunk of data, returning a fully
/// populated [`BlockHeader`] and the on-disk payload.
///
/// `content_hash` in the header is always BLAKE3 of the **original
/// uncompressed** plaintext — independent of encryption and compression.
/// This makes it suitable as a CAS key and a final integrity check.
pub fn encode_block(
    block_type:     BlockType,
    file_id:        u32,
    file_offset:    u64,
    data:           &[u8],
    codec_id:       CodecId,
    level:          i32,
    encryption_key: Option<&[u8; 32]>,
) -> Result<(BlockHeader, Vec<u8>), CodecError> {
    // BLAKE3 of original plaintext — CAS identity, stored in header.
    let content_hash: [u8; 32] = blake3::hash(data).into();

    // Compress.
    let codec   = get_codec_by_uuid(&codec_id.uuid())?;
    let mut payload = codec.compress(data, level)?;

    // Optionally encrypt the compressed payload.
    let mut flags = 0u16;
    if let Some(key) = encryption_key {
        payload = crate::crypto::encrypt(key, &payload)
            .map_err(|e| CodecError::Encryption(e.to_string()))?;
        flags |= FLAG_ENCRYPTED;
    }

    let header = BlockHeader {
        header_version: BLOCK_HEADER_VERSION,
        block_type,
        flags,
        codec_uuid:   codec_id.uuid(),
        file_id,
        file_offset,
        orig_size:    data.len() as u32,
        comp_size:    payload.len() as u32,
        content_hash,
    };

    Ok((header, payload))
}

// ── decode_block ──────────────────────────────────────────────────────────────

/// Verify, decrypt (if needed), and decompress a block payload.
///
/// Verification order (no opt-outs):
///   1. Decrypt (if FLAG_ENCRYPTED) — GCM tag verifies ciphertext integrity
///   2. Decompress via the UUID named in the header
///   3. BLAKE3 of decompressed output == `header.content_hash`
///
/// If step 3 fails the decompressor produced wrong output — treat as
/// corruption regardless of which codec was used.
pub fn decode_block(
    header:         &BlockHeader,
    payload:        &[u8],
    decryption_key: Option<&[u8; 32]>,
) -> Result<Vec<u8>, CodecError> {
    // 1. Decrypt if flagged — GCM tag covers the ciphertext.
    let compressed = if header.is_encrypted() {
        let key = decryption_key.ok_or_else(|| {
            CodecError::Encryption("Block is encrypted but no decryption key was provided".into())
        })?;
        crate::crypto::decrypt(key, payload)
            .map_err(|e| CodecError::Encryption(e.to_string()))?
    } else {
        payload.to_vec()
    };

    // 2. Decompress using the UUID embedded in the header.
    //    Fails hard if the UUID is not available in this build.
    let codec        = get_codec_by_uuid(&header.codec_uuid)?;
    let decompressed = codec.decompress(&compressed)?;

    // 3. BLAKE3 content hash — mandatory final check.
    let actual_hash: [u8; 32] = blake3::hash(&decompressed).into();
    if actual_hash != header.content_hash {
        return Err(CodecError::Decompression(format!(
            "BLAKE3 content hash mismatch (got {}, expected {})",
            hex::encode(actual_hash),
            hex::encode(header.content_hash),
        )));
    }

    Ok(decompressed)
}
