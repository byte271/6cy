use clap::{Parser, Subcommand};
use sixcy::io_stream::{SixCyWriter, SixCyReader};
use sixcy::CodecId;
use std::fs::{self, File};
use std::io::{Read, Write, Seek};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "6cy")]
#[command(about = "The .6cy container format CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack a file into a .6cy archive
    Pack {
        #[arg(short, long)]
        output: PathBuf,
        input: Vec<PathBuf>,
        #[arg(short, long, default_value = "zstd")]
        codec: String,
        #[arg(short, long)]
        solid: bool,
    },
    /// Unpack a .6cy archive
    Unpack {
        input: PathBuf,
        #[arg(short = 'C', long, default_value = ".")]
        output_dir: PathBuf,
    },
    /// List contents of a .6cy archive
    List {
        input: PathBuf,
    },
    /// Show detailed info about a .6cy archive
    Info {
        input: PathBuf,
    },
    /// Optimize a .6cy archive (re-compress with better settings)
    Optimize {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Pack { output, input, codec, solid } => {
            let codec_id = match codec.as_str() {
                "zstd" => CodecId::Zstd,
                "lz4" => CodecId::Lz4,
                _ => CodecId::Zstd,
            };

            let file = File::create(output)?;
            let mut writer = SixCyWriter::new(file)?;
            
            if *solid {
                writer.start_solid_session(codec_id)?;
            }

            for path in input {
                let mut data = Vec::new();
                File::open(path)?.read_to_end(&mut data)?;
                writer.add_file(path.file_name().unwrap().to_string_lossy().into_owned(), &data, codec_id)?;
                println!("Added: {}", path.display());
            }
            
            writer.finalize()?;
            println!("Successfully created archive: {}", output.display());
        }
        Commands::Unpack { input, output_dir } => {
            let file = File::open(input)?;
            let mut reader = SixCyReader::new(file)?;
            
            if !output_dir.exists() {
                fs::create_dir_all(output_dir)?;
            }

            let records = reader.index.records.clone();
            for record in records {
                let data = reader.unpack_file(record.id)?;
                let out_path = output_dir.join(&record.name);
                File::create(out_path)?.write_all(&data)?;
                println!("Unpacked: {}", record.name);
            }
        }
        Commands::List { input } => {
            let file = File::open(input)?;
            let reader = SixCyReader::new(file)?;
            
            println!("Archive: {}", input.display());
            println!("{:<20} {:<10} {:<10} {:<10}", "Name", "Size", "Compressed", "Blocks");
            for record in &reader.index.records {
                let block_hashes: Vec<String> = record.block_refs.iter().map(|b| hex::encode(&b.hash[..4])).collect();
                println!("{:<20} {:<10} {:<10} {:<10}", record.name, record.original_size, record.compressed_size, block_hashes.join(","));
            }
        }
        Commands::Info { input } => {
            let mut file = File::open(input)?;
            let sb = sixcy::superblock::Superblock::read(&mut file)?;
            println!("--- .6cy Archive Info ---");
            println!("Version: {}", sb.version);
            println!("UUID:    {}", sb.uuid);
            println!("Index Offset:    {}", sb.index_offset);
            println!("Recovery Offset: {}", sb.recovery_map_offset);
            println!("Index Size:      {}", sb.index_size);
            println!("Required Codecs: {:?}", sb.required_codecs);

            // Read index to show root hash
            file.seek(std::io::SeekFrom::Start(sb.index_offset))?;
            let mut index_bytes = vec![0u8; sb.index_size as usize];
            file.read_exact(&mut index_bytes)?;
            let index = sixcy::index::FileIndex::from_bytes(&index_bytes)?;
            println!("Root Hash:       {}", hex::encode(index.root_hash));
        }
        Commands::Optimize { input, output } => {
            let file_in = File::open(input)?;
            let mut reader = SixCyReader::new(file_in)?;
            
            let file_out = File::create(output)?;
            let mut writer = SixCyWriter::new(file_out)?;
            
            let records = reader.index.records.clone();
            for record in records {
                let data = reader.unpack_file(record.id)?;
                // Optimize by using Zstd level 19 for the new archive
                writer.add_file(record.name, &data, CodecId::Zstd)?;
            }
            writer.finalize()?;
            println!("Archive optimized and saved to {}", output.display());
        }
    }

    Ok(())
}
