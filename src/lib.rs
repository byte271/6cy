//! # sixcy — .6cy container format reference implementation
//!
//! Format guarantees:
//! - All numeric fields are little-endian
//! - Every block is self-describing: magic, version, codec UUID, sizes, BLAKE3
//! - Every block header carries a mandatory CRC32; corrupt headers abort reads
//! - Codec identity is a frozen 16-byte UUID; short IDs are in-process only
//! - The container superblock declares all required codecs; decoders fail
//!   immediately if any UUID is unavailable — no partial decode, no fallback
//! - The INDEX block is at the end; the full block list is reconstructible by
//!   scanning forward from `SUPERBLOCK_SIZE` without the INDEX
//! - The plugin C ABI (`plugin.rs`) is stable at `SIXCY_PLUGIN_ABI_VERSION=1`

pub mod archive;
pub mod block;
pub mod cdc;
pub mod chaos;
pub mod codec;
pub mod crypto;
pub mod fec;
pub mod index;
pub mod io_stream;
pub mod merkle;
pub mod perf;
pub mod plugin;
pub mod pq_crypto;
pub mod recovery;
pub mod sharding;
pub mod stego;
pub mod superblock;

// Re-exports for the most common types.
pub use archive::{Archive, FileInfo, PackOptions};
pub use block::{
    decode_block, encode_block, BlockHeader, BlockType, BLOCK_HEADER_SIZE, BLOCK_MAGIC,
};
pub use cdc::{ChunkStrategy, ChunkerConfig, StreamChunker};
pub use codec::{get_codec, get_codec_by_uuid, CodecError, CodecId};
pub use crypto::{derive_key, CryptoError};
pub use fec::{encode_parity_shards, reconstruct_data_shards, FecBlockHeader, FecConfig};
pub use index::{BlockRef, FileIndex, FileIndexRecord};
pub use merkle::{MerkleProof, MerkleTree};
pub use plugin::{PluginCodec, SixcyCodecPlugin, SIXCY_PLUGIN_ABI_VERSION};
pub use recovery::{scan_file, BlockHealth, RecoveryQuality, RecoveryReport};
pub use superblock::Superblock;
