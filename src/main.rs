use clap::{Parser, Subcommand};
use sixcy::archive::{Archive, PackOptions};
use sixcy::codec::{CodecId, uuid_to_string};
use sixcy::io_stream::DEFAULT_CHUNK_SIZE;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "6cy", about = "The .6cy container format CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack one or more files into a .6cy archive
    Pack {
        #[arg(short, long)]
        output: PathBuf,
        /// Codec: zstd (default), lz4, brotli, lzma, none
        #[arg(short, long, default_value = "zstd")]
        codec: String,
        /// Compression level (zstd 1-19; brotli 0-11; ignored for lz4/lzma)
        #[arg(short, long, default_value = "3")]
        level: i32,
        /// Maximum chunk size in KiB (default 4096 = 4 MiB)
        #[arg(long, default_value = "4096")]
        chunk_size: usize,
        /// Combine all inputs into a single solid block
        #[arg(short, long)]
        solid: bool,
        /// Encrypt with AES-256-GCM (Argon2id key derivation)
        #[arg(short, long)]
        password: Option<String>,
        #[arg(short, long, required = true, num_args = 1..)]
        input: Vec<PathBuf>,
    },
    /// Unpack a .6cy archive
    Unpack {
        input: PathBuf,
        #[arg(short = 'C', long, default_value = ".")]
        output_dir: PathBuf,
        #[arg(short, long)]
        password: Option<String>,
    },
    /// List archive contents
    List {
        input: PathBuf,
    },
    /// Show archive metadata
    Info {
        input: PathBuf,
    },
    /// Scan blocks and reconstruct the file list without the INDEX block
    Scan {
        input: PathBuf,
    },
    /// Re-compress at maximum Zstd ratio
    Optimize {
        input:  PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(short, long, default_value = "19")]
        level: i32,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {

        // ── Pack ─────────────────────────────────────────────────────────────
        Commands::Pack { output, input, codec, level, chunk_size, solid, password } => {
            let codec_id = parse_codec(&codec);
            let opts = PackOptions {
                default_codec: codec_id,
                level,
                chunk_size: chunk_size * 1024,
                password,
            };
            let mut ar = Archive::create(&output, opts)?;
            if solid { ar.begin_solid(codec_id)?; }
            for path in &input {
                let data = std::fs::read(path)?;
                ar.add_file(path.file_name().unwrap().to_string_lossy().as_ref(), &data)?;
                println!("  packed  {}", path.display());
            }
            if solid { ar.end_solid()?; }
            ar.finalize()?;
            println!("Created: {}", output.display());
        }

        // ── Unpack ───────────────────────────────────────────────────────────
        Commands::Unpack { input, output_dir, password } => {
            let mut ar = open_archive(&input, &password)?;
            ar.extract_all(&output_dir)?;
            println!("Unpacked to: {}", output_dir.display());
        }

        // ── List ─────────────────────────────────────────────────────────────
        Commands::List { input } => {
            let ar = open_archive(&input, &None)?;
            println!("Archive: {}", input.display());
            println!("{:<26} {:>12} {:>12} {:>7}  First block hash",
                     "Name", "Size", "Compressed", "Chunks");
            for info in ar.list() {
                let hash = info.first_block_hash
                    .map(|h| hex::encode(&h[..6]))
                    .unwrap_or_else(|| "—".into());
                println!("{:<26} {:>12} {:>12} {:>7}  {}", 
                    info.name, info.original_size, info.compressed_size,
                    info.block_count, hash);
            }
        }

        // ── Info ─────────────────────────────────────────────────────────────
        Commands::Info { input } => {
            let ar    = open_archive(&input, &None)?;
            let files = ar.list();
            // Read superblock directly for low-level fields.
            let sb = {
                use std::io::Read;
                let mut f = std::fs::File::open(&input)?;
                sixcy::Superblock::read(&mut f)?
            };

            println!("── .6cy Archive ─────────────────────────────────────────");
            println!("  Path           {}", input.display());
            println!("  Format version {}", sb.format_version);
            println!("  UUID           {}", sb.archive_uuid);
            println!("  Encrypted      {}", sb.flags & sixcy::superblock::SB_FLAG_ENCRYPTED != 0);
            println!("  Index offset   {} B", sb.index_offset);
            println!("  Index size     {} B", sb.index_size);
            println!("  Files          {}", files.len());
            println!("  Root hash      {}", ar.root_hash_hex());
            println!("  Required codecs ({}):", sb.required_codec_uuids.len());
            for uuid_bytes in &sb.required_codec_uuids {
                let name = CodecId::from_uuid(uuid_bytes)
                    .map(|c| c.name())
                    .unwrap_or("UNKNOWN");
                println!("    {} ({})", uuid_to_string(uuid_bytes), name);
            }
        }

        // ── Scan ─────────────────────────────────────────────────────────────
        Commands::Scan { input } => {
            use sixcy::io_stream::SixCyReader;
            let mut reader = SixCyReader::new(std::fs::File::open(&input)?)?;
            let idx = reader.scan_blocks()?;
            println!("Scan recovered {} file(s) from block headers:", idx.records.len());
            for r in &idx.records {
                println!("  id={:08x}  chunks={}  size={}  name={}",
                    r.id, r.block_refs.len(), r.original_size, r.name);
            }
        }

        // ── Optimize ─────────────────────────────────────────────────────────
        Commands::Optimize { input, output, password, level } => {
            let mut src = open_archive(&input, &password)?;
            let files: Vec<(String, Vec<u8>)> = src.list()
                .into_iter()
                .map(|info| (info.name.clone(), src.read_file_by_id(info.id).unwrap_or_default()))
                .collect();

            let opts = PackOptions {
                default_codec: CodecId::Zstd,
                level,
                chunk_size: DEFAULT_CHUNK_SIZE,
                password: None,
            };
            let mut dst = Archive::create(&output, opts)?;
            for (name, data) in files {
                dst.add_file(&name, &data)?;
            }
            dst.finalize()?;
            println!("Optimized → {}", output.display());
        }
    }

    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn open_archive(path: &PathBuf, password: &Option<String>) -> Result<Archive, Box<dyn std::error::Error>> {
    Ok(match password {
        Some(pwd) => Archive::open_encrypted(path, pwd)?,
        None      => Archive::open(path)?,
    })
}

fn parse_codec(s: &str) -> CodecId {
    CodecId::from_name(s).unwrap_or_else(|| {
        eprintln!("Unknown codec '{}', defaulting to zstd", s);
        CodecId::Zstd
    })
}
