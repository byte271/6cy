//! # Zenith Orchestration Layer ([`Archive`])
//!
//! The `Archive` orchestrator is the authoritative high-level interface for the
//! Zenith Data Sovereignty Protocol. It manages the lifecycle of
//! resilient data volumes, coordinating the transition between logical file
//! systems and the hardened physical block framework.
//!
//! ## Strategic Capabilities
//! - **Project Oort**: Hybrid-Quantum key encapsulation infrastructure.
//! - **Nebula Layer**: Threshold-based cryptographic key sharding (SSS).
//! - **Sentinel Layer**: High-velocity Cauchy-Reed-Solomon recovery.
//! - **Vanta-Black**: Forensic-grade obfuscation and whitening.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use crate::cdc::ChunkStrategy;
use crate::codec::CodecId;
use crate::crypto::derive_key;
use crate::fec::FecConfig;
use crate::index::FileIndexRecord;
use crate::io_stream::{
    SixCyReader, SixCyWriter, WriterOptions, DEFAULT_CHUNK_SIZE, DEFAULT_COMPRESSION_LEVEL,
};
use crate::sharding::NebulaShard;
use crate::superblock::Superblock;

// ── Zenith Configuration ──────────────────────────────────────────────────

/// High-precision configuration for Zenith archive generation.
#[derive(Debug, Clone)]
pub struct PackOptions {
    /// The default codec suite for new data blocks.
    pub default_codec: CodecId,
    /// Global compression velocity/ratio trade-off level.
    pub level: i32,
    /// Boundary size (in bytes) for high-velocity fixed chunking.
    pub chunk_size: usize,
    /// Global sovereignty secret; triggers AES-256-GCM and shadow-whitening.
    pub password: Option<String>,
    /// Gear64 Content-Defined Chunking for superior deduplication.
    pub use_cdc: bool,
    /// Binary Merkle tree generation for verifiable inclusion proofs.
    pub build_merkle: bool,
    /// Sentinel Layer: Cauchy-Reed-Solomon FEC redundancy configuration.
    pub fec: Option<FecConfig>,
    /// Nebula Layer: Threshold-based key fragmentation (K of N).
    pub sharding: Option<(u8, u8)>,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            default_codec: CodecId::Zstd,
            level: DEFAULT_COMPRESSION_LEVEL,
            chunk_size: DEFAULT_CHUNK_SIZE,
            password: None,
            use_cdc: false,
            build_merkle: false,
            fec: None,
            sharding: None,
        }
    }
}

// ── FileInfo (Zenith Metadata Descriptor) ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub id: u32,
    pub name: String,
    pub original_size: u64,
    pub compressed_size: u64,
    pub block_count: usize,
    pub first_block_hash: Option<[u8; 32]>,
    pub fec_stripe_count: usize,
    pub has_merkle: bool,
}

impl From<&FileIndexRecord> for FileInfo {
    fn from(r: &FileIndexRecord) -> Self {
        FileInfo {
            id: r.id,
            name: r.name.clone(),
            original_size: r.original_size,
            compressed_size: r.compressed_size,
            block_count: r.block_refs.len(),
            first_block_hash: r.block_refs.first().map(|b| b.content_hash),
            fec_stripe_count: r.fec_stripes.len(),
            has_merkle: !r.merkle_proofs.is_empty(),
        }
    }
}

// ── Archive Orchestrator ─────────────────────────────────────────────────────

enum ArchiveMode {
    Read(SixCyReader<File>),
    Write(SixCyWriter<File>, CodecId),
}

pub struct Archive {
    mode: ArchiveMode,
}

impl Archive {
    // ── Zenith Constructors ──────────────────────────────────────────────────

    /// Open a sovereign volume for read-access.
    ///
    /// If the volume is encrypted with Shadow-Whitening, use [`Archive::open_encrypted`].
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Self::open_with_password(path, None)
    }

    /// Open an encrypted sovereign volume using the provided master password.
    ///
    /// Triggers immediate Argon2id-KDF derivation to authenticate the Zenith Anchor.
    pub fn open_encrypted<P: AsRef<Path>>(path: P, password: &str) -> io::Result<Self> {
        Self::open_with_password(path, Some(password.to_owned()))
    }

    fn open_with_password<P: AsRef<Path>>(path: P, password: Option<String>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        let key = if let Some(ref pwd) = password {
            let mut f = File::open(&path)?;
            let sb =
                Superblock::read(&mut f).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Some(
                derive_key(pwd, sb.archive_uuid.as_bytes())
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
            )
        } else {
            None
        };

        let reader = SixCyReader::with_key(File::open(&path)?, key)?;
        Ok(Self {
            mode: ArchiveMode::Read(reader),
        })
    }

    /// Initialize a new Zenith-grade sovereign volume at the specified path.
    ///
    /// Technical parameters defined in [`PackOptions`] are locked into the
    /// Superblock v4.0 upon successful creation.
    pub fn create<P: AsRef<Path>>(path: P, opts: PackOptions) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        let chunk_strategy = if opts.use_cdc {
            ChunkStrategy::ContentDefined(crate::cdc::ChunkerConfig::default())
        } else {
            ChunkStrategy::Fixed {
                size: opts.chunk_size.max(1),
            }
        };

        let mut writer = SixCyWriter::with_writer_options(
            File::create(&path)?,
            WriterOptions {
                chunk_strategy,
                compression_level: opts.level,
                encryption_key: None,
                fec: opts.fec,
                build_merkle: opts.build_merkle,
            },
        )?;

        if let Some(ref pwd) = opts.password {
            let key = derive_key(pwd, writer.superblock.archive_uuid.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            writer.encryption_key = Some(key);
            writer.options.encryption_key = Some(key);
        }

        let default_codec = opts.default_codec;
        Ok(Self {
            mode: ArchiveMode::Write(writer, default_codec),
        })
    }

    // ── Nebula Sharding API ──────────────────────────────────────────────────

    /// Fragment the archive's master key across N physical shards via
    /// Shamirs Secret Sharing.
    ///
    /// Requires a threshold of K shards for total cryptographic reconstruction.
    pub fn sharding_zenith_key(
        &self,
        k: u8,
        n: u8,
        _password: &str,
    ) -> io::Result<Vec<NebulaShard>> {
        let key = match &self.mode {
            ArchiveMode::Read(r) => r
                .decryption_key
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Archive is not encrypted"))?,
            ArchiveMode::Write(w, _) => w
                .encryption_key
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Archive is not encrypted"))?,
        };
        let splitter = crate::sharding::NebulaSplitter::new(k, n)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(splitter.split(&key))
    }

    // ── Precision Logistics ──────────────────────────────────────────────────

    pub fn begin_solid(&mut self, codec: CodecId) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.start_solid_session(codec),
            ArchiveMode::Read(_) => Err(io_err("Zenith Read-Only")),
        }
    }

    pub fn end_solid(&mut self) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.flush_solid_session(),
            ArchiveMode::Read(_) => Err(io_err("Zenith Read-Only")),
        }
    }

    pub fn add_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        let codec = match &self.mode {
            ArchiveMode::Write(_, c) => *c,
            ArchiveMode::Read(_) => return Err(io_err("Zenith Read-Only")),
        };
        self.add_file_with_codec(name, data, codec)
    }

    pub fn add_file_with_codec(
        &mut self,
        name: &str,
        data: &[u8],
        codec: CodecId,
    ) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.add_file(name.to_owned(), data, codec),
            ArchiveMode::Read(_) => Err(io_err("Zenith Read-Only")),
        }
    }

    pub fn finalize(&mut self) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.finalize(),
            ArchiveMode::Read(_) => Err(io_err("Zenith Read-Only")),
        }
    }

    pub fn list(&self) -> Vec<FileInfo> {
        match &self.mode {
            ArchiveMode::Read(r) => r.index.records.iter().map(FileInfo::from).collect(),
            ArchiveMode::Write(w, _) => w.index.records.iter().map(FileInfo::from).collect(),
        }
    }

    pub fn read_file(&mut self, name: &str) -> io::Result<Vec<u8>> {
        let id = self
            .stat(name)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Entity not in index: {name}"),
                )
            })?
            .id;
        self.read_file_by_id(id)
    }

    pub fn read_file_by_id(&mut self, id: u32) -> io::Result<Vec<u8>> {
        match &mut self.mode {
            ArchiveMode::Read(r) => r.unpack_file(id),
            ArchiveMode::Write(_, _) => Err(io_err("Zenith Write-Only")),
        }
    }

    pub fn extract_all<P: AsRef<Path>>(&mut self, dest: P) -> io::Result<()> {
        let dest = dest.as_ref();
        if !dest.exists() {
            std::fs::create_dir_all(dest)?;
        }
        let ids: Vec<(u32, String)> = self.list().into_iter().map(|f| (f.id, f.name)).collect();
        for (id, name) in ids {
            let data = self.read_file_by_id(id)?;
            File::create(dest.join(&name))?.write_all(&data)?;
        }
        Ok(())
    }

    pub fn stat(&self, name: &str) -> Option<FileInfo> {
        self.list().into_iter().find(|f| f.name == name)
    }

    pub fn uuid(&self) -> uuid::Uuid {
        match &self.mode {
            ArchiveMode::Read(r) => r.superblock.archive_uuid,
            ArchiveMode::Write(w, _) => w.superblock.archive_uuid,
        }
    }

    pub fn root_hash_hex(&self) -> String {
        match &self.mode {
            ArchiveMode::Read(r) => hex::encode(r.index.root_hash),
            ArchiveMode::Write(w, _) => hex::encode(w.index.root_hash),
        }
    }

    pub fn verify_merkle(&self) -> bool {
        match &self.mode {
            ArchiveMode::Read(r) => r.index.verify_merkle(),
            ArchiveMode::Write(w, _) => w.index.verify_merkle(),
        }
    }
}

fn io_err(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, s)
}
