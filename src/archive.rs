//! High-level [`Archive`] API — the primary embedding surface.
//!
//! ```no_run
//! use sixcy::archive::{Archive, PackOptions};
//! use sixcy::codec::CodecId;
//!
//! // Write
//! let mut ar = Archive::create("out.6cy", PackOptions::default())?;
//! ar.add_file("readme.txt", b"Hello, world!")?;
//! ar.finalize()?;
//!
//! // Read
//! let mut ar = Archive::open("out.6cy")?;
//! let data = ar.read_file("readme.txt")?;
//! assert_eq!(data, b"Hello, world!");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::codec::CodecId;
use crate::crypto::derive_key;
use crate::index::FileIndexRecord;
use crate::io_stream::{SixCyReader, SixCyWriter, DEFAULT_CHUNK_SIZE, DEFAULT_COMPRESSION_LEVEL};
use crate::superblock::Superblock;

// ── PackOptions ───────────────────────────────────────────────────────────────

/// Configuration for [`Archive::create`].
#[derive(Debug, Clone)]
pub struct PackOptions {
    pub default_codec: CodecId,
    pub level:         i32,
    pub chunk_size:    usize,
    /// When set, every block is AES-256-GCM encrypted.
    /// Key = Argon2id(password, salt=archive_uuid).
    pub password:      Option<String>,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            default_codec: CodecId::Zstd,
            level:         DEFAULT_COMPRESSION_LEVEL,
            chunk_size:    DEFAULT_CHUNK_SIZE,
            password:      None,
        }
    }
}

// ── FileInfo ──────────────────────────────────────────────────────────────────

/// Lightweight descriptor returned by [`Archive::list`].
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub id:               u32,
    pub name:             String,
    pub original_size:    u64,
    pub compressed_size:  u64,
    pub block_count:      usize,
    pub first_block_hash: Option<[u8; 32]>,
}

impl From<&FileIndexRecord> for FileInfo {
    fn from(r: &FileIndexRecord) -> Self {
        FileInfo {
            id:               r.id,
            name:             r.name.clone(),
            original_size:    r.original_size,
            compressed_size:  r.compressed_size,
            block_count:      r.block_refs.len(),
            first_block_hash: r.block_refs.first().map(|b| b.content_hash),
        }
    }
}

// ── ArchiveMode ───────────────────────────────────────────────────────────────

enum ArchiveMode {
    Read(SixCyReader<File>),
    Write(SixCyWriter<File>, CodecId),
}

// ── Archive ───────────────────────────────────────────────────────────────────

pub struct Archive {
    path: PathBuf,
    mode: ArchiveMode,
}

impl Archive {
    // ── Constructors ─────────────────────────────────────────────────────────

    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Self::open_with_password(path, None)
    }

    pub fn open_encrypted<P: AsRef<Path>>(path: P, password: &str) -> io::Result<Self> {
        Self::open_with_password(path, Some(password.to_owned()))
    }

    fn open_with_password<P: AsRef<Path>>(path: P, password: Option<String>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();

        let key = if let Some(ref pwd) = password {
            let mut f = File::open(&path)?;
            let sb = Superblock::read(&mut f)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Some(derive_key(pwd, sb.archive_uuid.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?)
        } else {
            None
        };

        let reader = SixCyReader::with_key(File::open(&path)?, key)?;
        Ok(Self { path, mode: ArchiveMode::Read(reader) })
    }

    pub fn create<P: AsRef<Path>>(path: P, opts: PackOptions) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        let mut writer = SixCyWriter::with_options(
            File::create(&path)?,
            opts.chunk_size,
            opts.level,
            None,
        )?;

        if let Some(ref pwd) = opts.password {
            let key = derive_key(pwd, writer.superblock.archive_uuid.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            writer.encryption_key = Some(key);
        }

        let default_codec = opts.default_codec;
        Ok(Self { path, mode: ArchiveMode::Write(writer, default_codec) })
    }

    // ── Write ─────────────────────────────────────────────────────────────────

    pub fn add_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        let codec = match &self.mode {
            ArchiveMode::Write(_, c) => *c,
            ArchiveMode::Read(_)     => return Err(read_only()),
        };
        self.add_file_with_codec(name, data, codec)
    }

    pub fn add_file_with_codec(&mut self, name: &str, data: &[u8], codec: CodecId) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.add_file(name.to_owned(), data, codec),
            ArchiveMode::Read(_)     => Err(read_only()),
        }
    }

    pub fn begin_solid(&mut self, codec: CodecId) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.start_solid_session(codec),
            ArchiveMode::Read(_)     => Err(read_only()),
        }
    }

    pub fn end_solid(&mut self) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.flush_solid_session(),
            ArchiveMode::Read(_)     => Err(read_only()),
        }
    }

    /// Flush the INDEX block and patch the superblock.  Must be called once.
    pub fn finalize(&mut self) -> io::Result<()> {
        match &mut self.mode {
            ArchiveMode::Write(w, _) => w.finalize(),
            ArchiveMode::Read(_)     => Err(read_only()),
        }
    }

    // ── Read ──────────────────────────────────────────────────────────────────

    pub fn list(&self) -> Vec<FileInfo> {
        match &self.mode {
            ArchiveMode::Read(r)     => r.index.records.iter().map(FileInfo::from).collect(),
            ArchiveMode::Write(w, _) => w.index.records.iter().map(FileInfo::from).collect(),
        }
    }

    pub fn stat(&self, name: &str) -> Option<FileInfo> {
        self.list().into_iter().find(|f| f.name == name)
    }

    pub fn read_file(&mut self, name: &str) -> io::Result<Vec<u8>> {
        let id = self.stat(name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound,
                format!("File not found: {name}")))?
            .id;
        self.read_file_by_id(id)
    }

    pub fn read_file_by_id(&mut self, id: u32) -> io::Result<Vec<u8>> {
        match &mut self.mode {
            ArchiveMode::Read(r) => r.unpack_file(id),
            ArchiveMode::Write(_, _) => Err(write_only()),
        }
    }

    pub fn read_at(&mut self, name: &str, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let id = self.stat(name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound,
                format!("File not found: {name}")))?
            .id;
        match &mut self.mode {
            ArchiveMode::Read(r) => r.read_at(id, offset, buf),
            ArchiveMode::Write(_, _) => Err(write_only()),
        }
    }

    /// Extract all files into `dest`, creating it if necessary.
    pub fn extract_all<P: AsRef<Path>>(&mut self, dest: P) -> io::Result<()> {
        let dest = dest.as_ref();
        if !dest.exists() { std::fs::create_dir_all(dest)?; }
        let ids: Vec<(u32, String)> = self.list().into_iter().map(|f| (f.id, f.name)).collect();
        for (id, name) in ids {
            let data = self.read_file_by_id(id)?;
            File::create(dest.join(&name))?.write_all(&data)?;
        }
        Ok(())
    }

    // ── Metadata ─────────────────────────────────────────────────────────────

    pub fn path(&self) -> &Path { &self.path }

    pub fn uuid(&self) -> uuid::Uuid {
        match &self.mode {
            ArchiveMode::Read(r)     => r.superblock.archive_uuid,
            ArchiveMode::Write(w, _) => w.superblock.archive_uuid,
        }
    }

    pub fn root_hash_hex(&self) -> String {
        match &self.mode {
            ArchiveMode::Read(r)     => hex::encode(r.index.root_hash),
            ArchiveMode::Write(w, _) => hex::encode(w.index.root_hash),
        }
    }
}

fn read_only()  -> io::Error { io::Error::new(io::ErrorKind::PermissionDenied, "archive is read-only") }
fn write_only() -> io::Error { io::Error::new(io::ErrorKind::PermissionDenied, "archive is write-only") }
