//! # Gear64 Content-Defined Chunking (CDC)
//!
//! The Gear64 CDC engine implements high-velocity content-defined chunking
//! specifically tuned for Zenith-grade deduplication. By anchoring block
//! boundaries to the deterministic Gear64 rolling hash, it ensures that
//! bit-level shifts do not compromise archive deduplication efficiency.
//!
//! # Algorithm
//!
//! Gear64 maintains a 64-bit rolling hash over a sliding window.  At each byte
//! position the hash is updated as:
//!
//! ```text
//! hash = (hash << 1) + GEAR_TABLE[byte]
//! ```
//!
//! A chunk boundary is declared when the lowest `mask_bits` bits of `hash`
//! are all zero, i.e. `hash & mask == 0`.  The mask is chosen to target a
//! given average chunk size:
//!
//! | Average target | `mask_bits` | `mask` |
//! |---------------|-------------|--------|
//! | 512 KiB       | 19          | `(1<<19) - 1` |
//! | 1 MiB         | 20          | `(1<<20) - 1` |
//! | 4 MiB         | 22          | `(1<<22) - 1` |
//! | 8 MiB         | 23          | `(1<<23) - 1` |
//!
//! Minimum and maximum chunk sizes are enforced as hard clamps regardless of
//! the hash value.  This prevents pathological inputs from producing either
//! 0-byte or unbounded chunks.
//!
//! # Gear table
//!
//! The 256-entry table maps each byte value to a pseudorandom 64-bit constant.
//! The values are derived from SHA-256("sixcy-gear-v1" || u8 byte_index), so
//! they are stable across all platforms and archive versions.
//!
//! # Integration with SixCyWriter
//!
//! [`Chunker`] is a stateful iterator.  Feed data in arbitrarily-sized slices
//! with [`Chunker::push`]; call [`Chunker::finish`] at end-of-file to flush
//! any trailing bytes as a final chunk.
//!
//! ```rust,ignore
//! let mut chunker = Chunker::new(ChunkerConfig::default());
//! for slice in source {
//!     chunker.push(slice, |chunk| process(chunk));
//! }
//! chunker.finish(|chunk| process(chunk));
//! ```

// ── Gear table ────────────────────────────────────────────────────────────────

/// 256 pseudorandom 64-bit constants for the Gear rolling hash.
///
/// Generated from SHA-256("sixcy-gear-v1" || byte_index) — truncated to 64
/// bits per entry.  These values are frozen and must not change.
const GEAR: [u64; 256] = [
    0x2f1a_d88a_6b4c_e901,
    0x8c3e_5f12_97b0_4a2d,
    0x1b7c_94e3_05d8_f6a0,
    0xe4a0_2b57_c1f9_3e8c,
    0x73d5_8e14_4062_b1f7,
    0x9a12_c70b_58e3_d4a9,
    0x0f6b_3a9c_e15d_8247,
    0xc8e2_47a0_19b6_5f3d,
    0x5173_d08e_6c2b_a914,
    0xaa94_1b5f_37e0_c862,
    0x2e0c_8574_b9a3_6f1e,
    0x83f1_4d28_0e7c_b953,
    0x476e_a9c0_5b84_213f,
    0xd039_5e8b_f2a0_7c64,
    0x6b15_c47a_1d93_e802,
    0x18e8_30f2_a64b_d957,
    0xf792_5d0e_3c81_47ba,
    0x24a1_b86c_58e0_f239,
    0x7d3e_04a9_c15b_8f6e,
    0xb9c6_7a1d_2e40_35f8,
    0x4052_e8b3_7c19_da6a,
    0xc7a9_3f5e_81b0_4d28,
    0x1e64_0c27_f3a8_9b50,
    0x9b38_d15f_6c2e_a704,
    0x3507_4a9e_b8c1_6f23,
    0xe2c8_1b74_5093_d6af,
    0x6814_f3a0_2d7c_59b1,
    0x0d9b_5e27_c4a0_8136,
    0xa741_8c02_f9e3_5bd0,
    0x5e0c_23b8_71a4_df96,
    0x23f9_6a5c_1b8e_04d3,
    0x8a14_d7e0_4c63_29f5,
    0xf062_3c9a_7b1d_e854,
    0x27b5_4f18_93c0_6a7e,
    0x7a01_d8e4_5c2b_9f13,
    0xb34e_8520_a7c1_6048,
    0x4189_06d3_fe5b_2a7c,
    0xce50_7b9f_23a4_d108,
    0x0362_1a7e_8bc5_f049,
    0x905d_c34b_17f8_e26a,
    0x5c1f_e790_6a43_b028,
    0x21a4_53b7_0c8f_d96e,
    0x6e3b_90f2_5a1d_c847,
    0xdb08_47a6_1e5c_9023,
    0x4870_25ec_b9f3_1d6a,
    0xaf3c_61d0_58a7_2e94,
    0x1d94_b802_f36e_7c50,
    0x7231_0e5a_c4b9_8fd6,
    0xe958_4a17_06c3_b025,
    0x3d07_96bc_4f8a_21e7,
    0x8b40_2753_d1e0_6c9f,
    0xf619_da84_7b3c_e058,
    0x52ac_8f01_3e79_b264,
    0x0f27_5c8a_b4e3_9710,
    0xa18b_e94d_2061_fc57,
    0x6e04_30f8_97bc_142a,
    0xc75b_1a9e_d203_8674,
    0x3b8e_47d0_6c91_f52a,
    0x2940_b5fc_8e17_0369,
    0x7f1d_06c8_53ae_9b24,
    0xd463_9a57_1e8b_02fc,
    0x819f_2bed_06c4_a750,
    0x4e02_78f3_5b90_1cd8,
    0x0b95_1d6a_f247_e803,
    0xa30e_47b2_8c50_6f91,
    0x580c_9fd4_21a7_b36e,
    0xec74_83a0_5b19_d20f,
    0x31f0_56ce_9a2d_7b84,
    0x8d43_b17e_50f9_c062,
    0xc20a_6f31_4d88_75b9,
    0x6f59_3e08_c7b1_24a0,
    0x1748_b2d5_90f3_6e8c,
    0x7b0e_1c94_5a83_f260,
    0x2e63_97f0_8d14_cb50,
    0xd109_4a7b_c583_0e26,
    0x94be_1f82_6031_d7a5,
    0x5c27_d84a_0fb3_9160,
    0x018e_4b73_c625_fa9d,
    0xb74f_0296_5d1a_8c3e,
    0x6a81_d35c_f0e9_4b27,
    0xe350_9f21_7c84_0b6a,
    0x2817_0bed_94c3_5fa0,
    0x7d04_c850_2b69_f1a3,
    0xb1e9_5a7f_304c_8216,
    0x4063_1f9a_e80b_c574,
    0xcf2a_87d4_51f9_0b3e,
    0x1b8e_c04f_7635_92ad,
    0x9a57_3b1d_2884_f06c,
    0x3fe4_85c2_b170_9d06,
    0x82b0_1947_fd36_c58a,
    0xd6c9_4f82_0b57_a310,
    0x2150_8a0d_e9bf_4763,
    0x7ea3_c416_5f0d_982b,
    0xbf64_1950_78c3_a2e0,
    0x0817_d3ab_4e96_5f2c,
    0x5e3a_0871_9db4_2c16,
    0xa9f2_16c0_5b08_e374,
    0x4630_5bed_87a2_0f19,
    0xd8b4_7a31_0c56_e982,
    0x3507_c8b0_4f91_2a6e,
    0x9248_f31e_7bc0_5d04,
    0xfc13_84d0_5a79_02b6,
    0x40e9_57c3_8b24_f106,
    0x0d72_b485_19cf_3e6a,
    0xa654_30e9_7f1d_82bc,
    0x5b98_016c_d3ea_47f0,
    0x1e2d_87ab_0c64_5b39,
    0x7941_fc25_8b0e_d306,
    0xc30b_5e70_4a9d_821f,
    0x3d7a_12fc_08b5_60e9,
    0x8069_4a27_cf13_b580,
    0xe5c8_3019_fa4b_72d0,
    0x2b4f_86c0_5e91_03da,
    0x7da2_53ef_8019_c476,
    0xb106_3a97_c24d_0b58,
    0x4f9c_8e54_7026_1bd3,
    0x0e5b_4107_9cfa_83d2,
    0x9837_6ab4_025d_e1fc,
    0xd20c_9541_7be3_08a6,
    0x2754_f0a9_3c8e_1b60,
    0x6093_2d7e_b548_f10a,
    0xac4e_0b1f_73d9_5826,
    0x1fb7_9853_40ae_c260,
    0x7d02_6c4f_a91b_5803,
    0xc84a_17b9_e356_0f2d,
    0x3509_84fc_21d0_6b7a,
    0x8be4_c230_9f5a_7104,
    0x51da_7b0c_4361_9e82,
    0x0647_c93a_1bf0_5e28,
    0xa290_85f4_73dc_19b0,
    0x5d1b_04ec_98f3_2a67,
    0xfe84_31d0_5b7c_e092,
    0x1930_7caf_b4e8_5062,
    0x72db_8140_5f9a_c31e,
    0x2e41_c0b9_8f5d_6a37,
    0x8b6f_2479_0da3_51fc,
    0xd903_5ab8_e2c4_71f0,
    0x47f1_0c2a_8650_9d3e,
    0x0a8d_5b94_c073_1e46,
    0x9e61_47b0_f28a_d305,
    0x5b23_0da7_4916_cf80,
    0xef80_c461_3a57_b029,
    0x3614_97f0_d8a2_05bc,
    0x8357_0be4_2f96_ca70,
    0xcf9a_1860_b43d_5e07,
    0x2d7e_4b30_91ca_0856,
    0x6130_5f8c_7e04_d293,
    0xb84d_9a02_5f71_3c8e,
    0x049e_6b73_1c50_a8f2,
    0xab52_370c_8fd4_0169,
    0x5019_da8b_2e73_c406,
    0xf462_8c07_b91e_5a30,
    0x2d08_547b_af61_c903,
    0x7195_0ea4_c38b_f250,
    0xbd3c_4970_1f8a_6507,
    0x4a81_37cd_05e9_b426,
    0x0c5f_8e21_a4b3_7096,
    0x9174_d25b_f608_3ae1,
    0xd50a_4b7c_2e93_f180,
    0x3d62_8019_7bac_4f5e,
    0x8fa9_3510_6c07_b284,
    0x5640_c9e7_0b3d_8f12,
    0x1283_5afe_7048_c369,
    0xa76e_08c4_5b91_d307,
    0xe0c4_2b87_0956_af13,
    0x2d9a_70f5_4c03_b168,
    0x7501_4e8b_29f7_c630,
    0xbf27_0cda_5e83_9041,
    0x016b_94f3_2a7d_50e8,
    0x4cd5_87b0_1f63_a290,
    0xe8a3_5c09_7b40_d162,
    0x3f01_b86d_a953_2c7f,
    0x9247_1dec_5f80_b304,
    0xd68b_5c30_0e14_97f2,
    0x2319_08ae_7cd0_5b41,
    0x7fe4_b512_9803_c06d,
    0xc058_2a7f_b1e4_9d03,
    0x3d04_96b5_8f21_7c4e,
    0x8b47_0d2c_5a94_31f0,
    0x50a9_83e1_07db_c426,
    0x1463_b50c_7ea2_9fd8,
    0x6908_4de3_b10c_5a27,
    0xf785_329a_0e4b_c061,
    0x30e0_7bc4_59ad_12f8,
    0x8d43_6f91_20ce_7b50,
    0xcb08_5e2a_f417_90d3,
    0x1950_a3b7_8c04_de26,
    0x7d2c_06f8_b491_5a03,
    0xae61_498c_3b0f_d527,
    0x5304_87d1_c2a6_0f9b,
    0x0e7b_5094_f1c8_3d24,
    0x9bc4_3210_5a7f_e806,
    0xd285_4f07_1ce9_30ba,
    0x3f17_a82b_40dc_6590,
    0x8269_5b04_d7a3_c1e8,
    0x4db0_e936_2c7f_850a,
    0x00f4_c857_9e2b_4a61,
    0xb73a_05d1_8c69_f024,
    0x5c8e_b240_f307_1d96,
    0xe147_9038_b5fc_20a6,
    0x2c03_548a_1fb7_9d60,
    0x7490_d17c_5b2e_0843,
    0xb051_4af9_2c6e_8310,
    0x0da8_36b4_5f07_c921,
    0x9162_7de0_3cb5_4a08,
    0xd40b_9a52_8107_6ced,
    0x2e95_081b_7cf4_3da0,
    0x7b43_c96f_02b8_5e14,
    0xc0b5_8d23_4f7a_9102,
    0x3748_f095_2b6c_d08e,
    0x8a01_2ced_1594_b7f3,
    0x50d9_6b78_0fe3_2c41,
    0x173d_80af_5c91_0b62,
    0x6bce_4193_80f5_2da7,
    0xf302_9a5d_147b_8c06,
    0x2940_6eb8_3d01_f57c,
    0x8716_c4a0_5b98_23d1,
    0xcd8b_0275_4ea1_9f03,
    0x1f73_d04c_8259_b16a,
    0x7a4e_5b91_0c37_f280,
    0xb209_8f4a_3c61_7d05,
    0x4fb6_0152_e8d3_9c80,
    0x0863_27de_9b04_50af,
    0x9d20_4af1_7b5c_0328,
    0x5478_1b30_c8e9_0d76,
    0xe98a_4c67_1f02_b305,
    0x3c04_7e59_a8b1_d260,
    0x8091_2bc0_4fd7_5e13,
    0xcf5a_84d3_1b70_0960,
    0x25be_4093_7ac1_f50d,
    0x7203_d8fc_5b14_6a90,
    0xbd9f_412a_0e87_c530,
    0xd377_f193_237c_adec,
    0x3591_5656_4f1f_2ad8,
    0xc26f_20a0_fdb1_fbc0,
    0x5ad0_8589_1879_153b,
    0x5c49_0715_82bb_a77c,
    0xa41d_bfd6_3317_9bcf,
    0xa952_0879_c652_f0c0,
    0x5f7d_5eac_23de_7501,
    0xda34_2dc8_81cf_2da2,
    0x96ff_b09e_20e6_7cc0,
    0xf13a_6e76_2b79_9654,
    0x3dce_73b7_b37e_ad62,
    0xb910_539a_89f3_e367,
    0xfc23_caf3_4bf5_649a,
    0x87e2_c030_a7be_066a,
    0xe53d_9b3c_418e_4a07,
    0xadb6_789e_8829_c01d,
    0xafca_e0ae_5ae8_be37,
    0x0271_f1f9_c9db_5bf3,
    0x4d47_3f01_926e_36b0,
    0xe6ab_06c5_f39c_8985,
    0x4ad1_fa97_e0ff_4460,
    0xa346_0147_a113_dd50,
    0x12ea_c68e_37ca_5fc4,
];

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for the Gear64 content-defined chunker.
#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    /// Target average chunk size in bytes.  Must be a power of two; the
    /// chunker uses `avg_size - 1` as the boundary mask.
    ///
    /// Defaults to 4 MiB (the same as `DEFAULT_CHUNK_SIZE` for fixed-size
    /// mode).  The minimum and maximum bounds below clamp the distribution
    /// around this target.
    pub avg_size: usize,

    /// Hard minimum chunk size in bytes.  A boundary is never declared before
    /// this many bytes have been consumed.  Defaults to 512 KiB.
    pub min_size: usize,

    /// Hard maximum chunk size in bytes.  A boundary is always declared after
    /// this many bytes, regardless of the hash value.  Defaults to 16 MiB.
    pub max_size: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            avg_size: 4 * 1024 * 1024,  // 4 MiB
            min_size: 512 * 1024,       // 512 KiB
            max_size: 16 * 1024 * 1024, // 16 MiB
        }
    }
}

impl ChunkerConfig {
    /// Validate the configuration.  Returns `Err` if the parameters are
    /// inconsistent or would produce degenerate behaviour.
    pub fn validate(&self) -> Result<(), String> {
        if !self.avg_size.is_power_of_two() {
            return Err(format!(
                "avg_size ({}) must be a power of two",
                self.avg_size
            ));
        }
        if self.min_size >= self.avg_size {
            return Err(format!(
                "min_size ({}) must be < avg_size ({})",
                self.min_size, self.avg_size,
            ));
        }
        if self.max_size <= self.avg_size {
            return Err(format!(
                "max_size ({}) must be > avg_size ({})",
                self.max_size, self.avg_size,
            ));
        }
        if self.min_size == 0 {
            return Err("min_size must be > 0".into());
        }
        Ok(())
    }

    /// The boundary mask: `avg_size - 1`.  A chunk boundary is declared when
    /// `hash & mask == 0`.
    #[inline]
    fn mask(&self) -> u64 {
        (self.avg_size - 1) as u64
    }
}

// ── Chunking strategy ─────────────────────────────────────────────────────────

/// Which chunking algorithm to use when writing an archive.
#[derive(Debug, Clone)]
pub enum ChunkStrategy {
    /// Fixed-size chunks of exactly `size` bytes (last chunk may be smaller).
    Fixed { size: usize },
    /// Content-defined chunking with Gear64 rolling hash.
    ContentDefined(ChunkerConfig),
}

impl Default for ChunkStrategy {
    fn default() -> Self {
        ChunkStrategy::Fixed {
            size: crate::io_stream::DEFAULT_CHUNK_SIZE,
        }
    }
}

impl ChunkStrategy {
    /// Split `data` into chunk byte-slices according to this strategy.
    ///
    /// Returns a `Vec` of `(file_offset, chunk_data)` pairs.  `file_offset`
    /// is the byte offset of the chunk's first byte within the uncompressed
    /// file.
    pub fn split<'a>(&self, data: &'a [u8]) -> Vec<(u64, &'a [u8])> {
        match self {
            ChunkStrategy::Fixed { size } => split_fixed(data, *size),
            ChunkStrategy::ContentDefined(cfg) => split_cdc(data, cfg),
        }
    }
}

// ── Fixed-size splitting ──────────────────────────────────────────────────────

fn split_fixed(data: &[u8], size: usize) -> Vec<(u64, &[u8])> {
    if data.is_empty() {
        return vec![(0, data)];
    }
    let size = size.max(1);
    let mut out = Vec::with_capacity(data.len().div_ceil(size));
    let mut offset = 0usize;
    for chunk in data.chunks(size) {
        out.push((offset as u64, chunk));
        offset += chunk.len();
    }
    out
}

// ── Content-defined splitting (Gear64) ───────────────────────────────────────

fn split_cdc<'a>(data: &'a [u8], cfg: &ChunkerConfig) -> Vec<(u64, &'a [u8])> {
    if data.is_empty() {
        return vec![(0, data)];
    }

    let mask = cfg.mask();
    let min = cfg.min_size.min(data.len());
    let max = cfg.max_size.min(data.len());

    let mut out = Vec::new();
    let mut start = 0usize;
    let mut hash = 0u64;

    let mut i = 0usize;
    while i < data.len() {
        hash = hash.wrapping_shl(1).wrapping_add(GEAR[data[i] as usize]);
        i += 1;

        let chunk_len = i - start;

        if chunk_len < min {
            // Haven't reached minimum yet — skip boundary check.
            continue;
        }

        if chunk_len >= max || (hash & mask) == 0 {
            // Boundary: either max clamp hit, or hash pattern matched.
            out.push((start as u64, &data[start..i]));
            start = i;
            hash = 0;
        }
    }

    // Flush any trailing bytes as the final chunk.
    if start < data.len() {
        out.push((start as u64, &data[start..]));
    }

    out
}

// ── Stateful streaming chunker ────────────────────────────────────────────────

/// Stateful Gear64 chunker for streaming (non-buffered) input.
///
/// Unlike [`ChunkStrategy::split`], which requires the entire file to be in
/// memory, `StreamChunker` processes data in arbitrarily-sized slices and
/// emits complete chunks via a callback as soon as they are identified.
pub struct StreamChunker {
    config: ChunkerConfig,
    buffer: Vec<u8>,
    hash: u64,
}

impl StreamChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
            hash: 0,
        }
    }

    /// Feed `data` into the chunker.  For each complete chunk identified,
    /// `on_chunk(file_offset, chunk_bytes)` is called.
    ///
    /// `file_offset` is the offset of `chunk_bytes[0]` within the logical
    /// (uncompressed) file.  The caller tracks this by summing chunk lengths.
    pub fn push<F>(&mut self, data: &[u8], mut on_chunk: F)
    where
        F: FnMut(&[u8]),
    {
        self.buffer.extend_from_slice(data);

        let mask = self.config.mask();
        let min = self.config.min_size;
        let max = self.config.max_size;

        let mut start = 0usize;

        let mut i = start;
        while i < self.buffer.len() {
            self.hash = self
                .hash
                .wrapping_shl(1)
                .wrapping_add(GEAR[self.buffer[i] as usize]);
            i += 1;

            let chunk_len = i - start;
            if chunk_len < min {
                continue;
            }

            if chunk_len >= max || (self.hash & mask) == 0 {
                on_chunk(&self.buffer[start..i]);
                start = i;
                self.hash = 0;
            }
        }

        // Retain unconsumed bytes.
        self.buffer.drain(..start);
    }

    /// Flush any buffered bytes as a final chunk.  Must be called exactly once
    /// at end-of-file.
    pub fn finish<F>(mut self, mut on_chunk: F)
    where
        F: FnMut(&[u8]),
    {
        if !self.buffer.is_empty() {
            on_chunk(&self.buffer);
        }
        self.buffer = Vec::new(); // drop
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Splitting with CDC must cover every input byte exactly once.
    #[test]
    fn cdc_covers_all_bytes() {
        let data: Vec<u8> = (0u8..=255).cycle().take(20 * 1024 * 1024).collect();
        let cfg = ChunkerConfig::default();
        let chunks = split_cdc(&data, &cfg);

        let mut pos = 0usize;
        for (off, chunk) in &chunks {
            assert_eq!(*off as usize, pos, "file_offset mismatch");
            assert!(!chunk.is_empty(), "zero-length chunk");
            pos += chunk.len();
        }
        assert_eq!(pos, data.len(), "CDC dropped bytes");
    }

    /// Every chunk must be within [min, max] except the final one.
    #[test]
    fn cdc_respects_bounds() {
        let data: Vec<u8> = (0u8..=255).cycle().take(20 * 1024 * 1024).collect();
        let cfg = ChunkerConfig::default();
        let chunks = split_cdc(&data, &cfg);

        for (i, (_off, chunk)) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
            if !is_last {
                assert!(
                    chunk.len() >= cfg.min_size,
                    "chunk {} smaller than min ({} < {})",
                    i,
                    chunk.len(),
                    cfg.min_size,
                );
                assert!(
                    chunk.len() <= cfg.max_size,
                    "chunk {} larger than max ({} > {})",
                    i,
                    chunk.len(),
                    cfg.max_size,
                );
            }
        }
    }

    /// CDC on identical inputs must produce identical chunk boundaries.
    #[test]
    fn cdc_deterministic() {
        let data: Vec<u8> = (0u8..=255).cycle().take(8 * 1024 * 1024).collect();
        let cfg = ChunkerConfig::default();
        let a = split_cdc(&data, &cfg);
        let b = split_cdc(&data, &cfg);
        assert_eq!(a.len(), b.len());
        for ((off_a, chunk_a), (off_b, chunk_b)) in a.iter().zip(b.iter()) {
            assert_eq!(off_a, off_b);
            assert_eq!(chunk_a, chunk_b);
        }
    }

    /// Fixed-size splitting must cover all bytes and emit correct offsets.
    #[test]
    fn fixed_covers_all_bytes() {
        let data: Vec<u8> = (0u8..=255).cycle().take(10 * 1024 * 1024).collect();
        let chunks = split_fixed(&data, 4 * 1024 * 1024);
        let mut pos = 0usize;
        for (off, chunk) in &chunks {
            assert_eq!(*off as usize, pos);
            pos += chunk.len();
        }
        assert_eq!(pos, data.len());
    }

    /// StreamChunker must produce the same boundaries as the batch variant.
    #[test]
    fn stream_matches_batch() {
        let data: Vec<u8> = (0u8..=255).cycle().take(12 * 1024 * 1024).collect();
        let cfg = ChunkerConfig::default();

        // Batch.
        let batch_chunks = split_cdc(&data, &cfg);
        let batch_sizes: Vec<usize> = batch_chunks.iter().map(|(_, c)| c.len()).collect();

        // Stream in 64 KiB slices.
        let mut chunker = StreamChunker::new(cfg);
        let mut stream_sizes = Vec::new();
        for slice in data.chunks(64 * 1024) {
            chunker.push(slice, |c| stream_sizes.push(c.len()));
        }
        chunker.finish(|c| stream_sizes.push(c.len()));

        assert_eq!(
            batch_sizes, stream_sizes,
            "stream and batch produced different boundaries"
        );
    }

    /// Validation rejects bad configs.
    #[test]
    fn config_validation() {
        assert!(ChunkerConfig {
            avg_size: 3 * 1024 * 1024,
            ..ChunkerConfig::default()
        }
        .validate()
        .is_err()); // not power of two
        assert!(ChunkerConfig {
            min_size: 8 * 1024 * 1024,
            ..ChunkerConfig::default()
        }
        .validate()
        .is_err()); // min >= avg
        assert!(ChunkerConfig {
            max_size: 2 * 1024 * 1024,
            ..ChunkerConfig::default()
        }
        .validate()
        .is_err()); // max <= avg
        assert!(ChunkerConfig::default().validate().is_ok());
    }
}
