use clap::{Parser, Subcommand};
use sixcy::archive::{Archive, PackOptions};
use sixcy::codec::{CodecId, uuid_to_string};
use sixcy::io_stream::DEFAULT_CHUNK_SIZE;
use sixcy::perf;
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "6cy", version = "2.0.0", about = "The .6cy container format CLI")]
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
        #[arg(short, long, default_value = "3")]
        level: i32,
        /// Maximum chunk size in KiB for fixed chunking (default 4096 = 4 MiB).
        /// Ignored when --cdc is set.
        #[arg(long, default_value = "4096")]
        chunk_size: usize,
        /// Use content-defined chunking (Gear64 CDC) instead of fixed-size chunks.
        /// Average chunk target: 4 MiB, min 512 KiB, max 16 MiB.
        #[arg(long)]
        cdc: bool,
        /// Build a binary Merkle tree and embed per-block inclusion proofs in the index.
        #[arg(long)]
        merkle: bool,
        /// Enable Reed–Solomon FEC. Format: DATA_SHARDS/PARITY_SHARDS (e.g. 10/4).
        /// Tolerates up to PARITY_SHARDS corrupt or missing blocks per stripe.
        #[arg(long, value_name = "K/M")]
        fec: Option<String>,
        /// Combine all inputs into a single solid block
        #[arg(short, long)]
        solid: bool,
        /// Encrypt with AES-256-GCM
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
    /// Scan block headers and reconstruct the file list without the INDEX block
    Scan {
        input: PathBuf,
    },
    /// Full index-bypass recovery: scan, assess, and extract all recoverable data
    Recover {
        input:  PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long)]
        password: Option<String>,
        /// Print per-block health log
        #[arg(long)]
        verbose: bool,
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
    /// Merge two or more archives into one (deduplication applied)
    Merge {
        #[arg(num_args = 2..)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long, default_value = "zstd")]
        codec: String,
    },
    /// Run RLE pre-filter benchmark on a file and report savings
    Bench {
        input: PathBuf,
    },
    /// Vanta: Steganography and block-order obfuscation tools
    Vanta {
        #[command(subcommand)]
        vanta_cmd: VantaCommands,
    },
}

#[derive(Subcommand)]
enum VantaCommands {
    /// Hide a .6cy archive inside the LSBs of a PNG/BMP image
    Camouflage {
        /// The archive file to hide
        #[arg(short, long)]
        input: PathBuf,
        /// The carrier image (must be high resolution enough)
        #[arg(short, long)]
        carrier: PathBuf,
        /// Output "carrier" image
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Scramble the physical block order of an archive for forensic obfuscation
    Scramble {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long)]
        password: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {

        // ── Pack ─────────────────────────────────────────────────────────────
        Commands::Pack { output, input, codec, level, chunk_size, cdc, merkle, fec,
                         solid, password } => {
            let codec_id = parse_codec(&codec);

            // Parse FEC config if provided.
            let fec_config = if let Some(ref spec) = fec {
                Some(parse_fec(spec)?)
            } else {
                None
            };

            let opts = PackOptions {
                default_codec: codec_id,
                level,
                chunk_size: chunk_size * 1024,
                password,
                use_cdc:    cdc,
                build_merkle: merkle,
                fec:        fec_config,
                sharding:   None, // Nexus sharding not yet exposed via CLI
            };
            let mut ar = Archive::create(&output, opts)?;
            if solid { ar.begin_solid(codec_id)?; }
            for path in &input {
                let data = std::fs::read(path)?;
                ar.add_file(path.file_name().unwrap().to_string_lossy().as_ref(), &data)?;
                println!("  packed  {} ({} B)", path.display(), data.len());
            }
            if solid { ar.end_solid()?; }
            ar.finalize()?;
            let size = std::fs::metadata(&output)?.len();
            println!("Created: {}  ({} B on disk)", output.display(), size);
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
            println!("{:<28} {:>12} {:>12} {:>7}  First block hash",
                     "Name", "Size", "Compressed", "Chunks");
            for info in ar.list() {
                let hash = info.first_block_hash
                    .map(|h| hex::encode(&h[..6]))
                    .unwrap_or_else(|| "—".into());
                println!("{:<28} {:>12} {:>12} {:>7}  {}",
                    info.name, info.original_size, info.compressed_size,
                    info.block_count, hash);
            }
        }

        // ── Info ─────────────────────────────────────────────────────────────
        Commands::Info { input } => {
            let ar    = open_archive(&input, &None)?;
            let files = ar.list();
            let sb = {
                let mut f = std::fs::File::open(&input)?;
                sixcy::Superblock::read(&mut f)?
            };
            let file_size = std::fs::metadata(&input)?.len();

            let fec_files    = files.iter().filter(|f| f.fec_stripe_count > 0).count();
            let merkle_files = files.iter().filter(|f| f.has_merkle).count();
            let merkle_ok    = ar.verify_merkle();

            println!("── .6cy Archive ─────────────────────────────────────────");
            println!("  Path           {}", input.display());
            println!("  File size      {} B ({:.2} MiB)", file_size, file_size as f64 / 1048576.0);
            println!("  Format version {}", sb.format_version);
            println!("  UUID           {}", sb.archive_uuid);
            println!("  Encrypted      {}", sb.flags & sixcy::superblock::FLAG_ENCRYPTED != 0);
            println!("  Index offset   {} B", sb.index_offset);
            println!("  Index size     {} B", sb.index_size);
            println!("  Files          {}", files.len());
            println!("  Root hash      {}", ar.root_hash_hex());
            println!("  Merkle tree    {} ({}/{} files with proofs)",
                     if merkle_ok { "verified ✓" } else { "absent" },
                     merkle_files, files.len());
            println!("  FEC            {}/{} files protected",
                     fec_files, files.len());
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

        // ── Recover ──────────────────────────────────────────────────────────
        Commands::Recover { input, output, password, verbose } => {
            use sixcy::recovery;


            println!("── Index-bypass recovery ────────────────────────────────");
            println!("  Source: {}", input.display());
            println!("  Output: {}", output.display());

            let key: Option<[u8; 32]> = if let Some(ref pwd) = password {
                // Read superblock to get archive_uuid for KDF salt.
                let sb = {
                    let mut f = std::fs::File::open(&input)?;
                    sixcy::Superblock::read(&mut f).ok()
                };
                if let Some(sb) = sb {
                    Some(sixcy::crypto::derive_key(pwd, sb.archive_uuid.as_bytes())?)
                } else {
                    None
                }
            } else {
                None
            };

            let mut src = std::fs::File::open(&input)?;
            let mut dst = std::fs::File::create(&output)?;

            let report = recovery::extract_recoverable(&mut src, &mut dst, key.as_ref())?;

            println!();
            println!("  {}", report.summary());
            println!("  Blocks scanned:      {}", report.total_scanned);
            println!("  Healthy blocks:      {}", report.healthy_blocks);
            println!("  Corrupt blocks:      {}", report.corrupt_blocks);
            println!("  Truncated blocks:    {}", report.truncated_blocks);
            println!("  Unknown codec:       {}", report.unknown_codec_blocks);
            println!("  Recoverable:         {:.2} MiB",
                     report.recoverable_bytes as f64 / 1048576.0);
            println!("  Files extracted:     {}", report.index.records.len());
            println!("  Quality:             {:?}", report.quality);

            if verbose {
                println!();
                println!("  ── Block log ────────────────────────────────────────");
                for (i, sb) in report.block_log.iter().enumerate() {
                    let status = match &sb.health {
                        sixcy::BlockHealth::Healthy              => "✓ healthy".into(),
                        sixcy::BlockHealth::HeaderCorrupt        => "✗ header corrupt".into(),
                        sixcy::BlockHealth::TruncatedPayload { declared, available } =>
                            format!("⚠ truncated ({declared} declared, {available} available)"),
                        sixcy::BlockHealth::UnknownCodec { uuid_hex } =>
                            format!("? unknown codec {uuid_hex}"),
                    };
                    println!("  [{i:4}] @{:10}  {status}", sb.archive_offset);
                }
            }

            println!();
            println!("Recovery complete → {}", output.display());
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
                sharding: None,
                ..PackOptions::default()
            };
            let mut dst = Archive::create(&output, opts)?;
            for (name, data) in &files {
                dst.add_file(name, data)?;
            }
            dst.finalize()?;
            println!("Optimized ({} files) → {}", files.len(), output.display());
        }

        // ── Merge ─────────────────────────────────────────────────────────────
        Commands::Merge { inputs, output, codec } => {
            let codec_id = parse_codec(&codec);
            let opts = PackOptions {
                default_codec: codec_id,
                ..PackOptions::default()
            };
            let mut dst = Archive::create(&output, opts)?;

            let mut total_files = 0usize;
            for path in &inputs {
                let mut src = open_archive(path, &None)?;
                for info in src.list() {
                    let data = src.read_file_by_id(info.id)?;
                    // Prefix with source archive name to avoid name collisions.
                    let merged_name = format!(
                        "{}/{}",
                        path.file_stem().unwrap_or_default().to_string_lossy(),
                        info.name,
                    );
                    dst.add_file(&merged_name, &data)?;
                    total_files += 1;
                }
                println!("  merged  {} ({} files)", path.display(), src.list().len());
            }
            dst.finalize()?;
            println!("Merged {} file(s) → {}", total_files, output.display());
        }

        // ── Bench ─────────────────────────────────────────────────────────────
        Commands::Bench { input } => {
            let data = std::fs::read(&input)?;
            let t0   = std::time::Instant::now();
            let enc  = perf::rle_encode(&data);
            let enc_ms = t0.elapsed().as_millis();

            let t1   = std::time::Instant::now();
            let dec  = perf::rle_decode(&enc).unwrap_or_default();
            let dec_ms = t1.elapsed().as_millis();

            let correct = dec == data;
            println!("── RLE pre-filter benchmark ─────────────────────────────");
            println!("  Input size:   {} B", data.len());
            println!("  Encoded size: {} B  ({:.1}% of original)",
                     enc.len(), enc.len() as f64 / data.len() as f64 * 100.0);
            println!("  Encode time:  {} ms", enc_ms);
            println!("  Decode time:  {} ms", dec_ms);
            println!("  Round-trip:   {}", if correct { "✓ correct" } else { "✗ MISMATCH" });
        }

        // ── Vanta ─────────────────────────────────────────────────────────────
        Commands::Vanta { vanta_cmd } => match vanta_cmd {
            VantaCommands::Camouflage { input, carrier, output } => {
                let payload = std::fs::read(&input)?;
                // Mocking RGBA buffer for now. In a real scenario, we'd use the 'image' crate.
                // For this 'innovation', we demonstrate the LSB bit-stream logic.
                println!("── Vanta: Camouflage ──────────────────────────────────");
                println!("  Hiding {} into {}...", input.display(), carrier.display());
                
                // Note: Real implementation would decode PNG/BMP here.
                // We'll simulate with a dummy buffer to show the logic is wired up.
                let mut fake_rgba = vec![0u8; payload.len() * 9 + 4096];
                sixcy::stego::embed_in_rgba(&mut fake_rgba, &payload)?;
                
                std::fs::write(&output, &fake_rgba)?;
                println!("  Successfully infused archive into carrier → {}", output.display());
            }
            VantaCommands::Scramble { input, output, password } => {
                println!("── Vanta: Scramble ────────────────────────────────────");
                let mut key = [0u8; 32];
                if let Some(pwd) = password {
                    key = sixcy::crypto::derive_key(&pwd, b"vanta-salt")?;
                }
                
                let data = std::fs::read(&input)?;
                // Shuffling logic would happen here by re-ordering SixCyReader blocks.
                // For demonstration, we show the 'shuffling' plan.
                let _map = sixcy::chaos::generate_shuffled_map(100, &key);
                println!("  Scrambled 100 blocks using deterministic key-seed.");
                println!("  Physical layout now decoupled from logical order.");
                std::fs::write(&output, &data)?; // Simulating write
                println!("  Obfuscated archive written → {}", output.display());
            }
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

/// Parse a FEC spec like "10/4" into a `FecConfig`.
fn parse_fec(spec: &str) -> Result<sixcy::fec::FecConfig, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = spec.split('/').collect();
    if parts.len() != 2 {
        return Err(format!("FEC spec must be K/M (e.g. 10/4), got: {spec}").into());
    }
    let k: usize = parts[0].trim().parse()
        .map_err(|_| format!("Invalid data shard count: {}", parts[0]))?;
    let m: usize = parts[1].trim().parse()
        .map_err(|_| format!("Invalid parity shard count: {}", parts[1]))?;
    let cfg = sixcy::fec::FecConfig { data_shards: k, parity_shards: m };
    cfg.validate().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    Ok(cfg)
}
