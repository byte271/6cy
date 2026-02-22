//! # sixcy — .6cy container format reference implementation
//!
//! Format guarantees (frozen in v3):
//! - All numeric fields are little-endian; never negotiated
//! - Every block is self-describing: magic, version, codec UUID, sizes, BLAKE3
//! - Every block header carries a mandatory CRC32; corrupt headers abort reads
//! - Codec identity is a frozen 16-byte UUID; short IDs are in-process only
//! - The container superblock declares all required codecs; decoders fail
//!   immediately if any UUID is unavailable — no partial decode, no fallback
//! - The INDEX block is at the end; the full block list is reconstructible by
//!   scanning forward from `SUPERBLOCK_SIZE` without the INDEX
//! - The plugin C ABI (`plugin.rs`) is stable at `SIXCY_PLUGIN_ABI_VERSION=1`

pub mod superblock;
pub mod codec;
pub mod crypto;
pub mod block;
pub mod index;
pub mod recovery;
pub mod io_stream;
pub mod archive;
pub mod plugin;
pub mod perf;

// Flat re-exports for the most common types.
pub use superblock::Superblock;
pub use codec::{CodecId, get_codec, get_codec_by_uuid, CodecError};
pub use block::{BlockHeader, BlockType, encode_block, decode_block,
                BLOCK_HEADER_SIZE, BLOCK_MAGIC};
pub use index::{FileIndex, FileIndexRecord, BlockRef};
pub use crypto::{derive_key, CryptoError};
pub use archive::{Archive, PackOptions, FileInfo};
pub use plugin::{SixcyCodecPlugin, PluginCodec, SIXCY_PLUGIN_ABI_VERSION};
pub use recovery::{RecoveryReport, RecoveryQuality, BlockHealth, scan_file};
