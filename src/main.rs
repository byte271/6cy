use clap::{Parser, Subcommand};
use sixcy::io_stream::{SixCyWriter, SixCyReader};
use sixcy::CodecId;
use std::fs::{self, File};
use std::io::{Read, Write};
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
    Pack {
        #[arg(short, long)]
        output: PathBuf,
        input: PathBuf,
        #[arg(short, long, default_value = "zstd")]
        codec: String,
    },
    Unpack {
        input: PathBuf,
        #[arg(short = 'C', long, default_value = ".")]
        output_dir: PathBuf,
    },
    List {
        input: PathBuf,
    },
    Info {
        input: PathBuf,
    },
    Optimize {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Pack { output, input, codec } => {
            let codec_id = match codec.as_str() {
                "zstd" => CodecId::Zstd,
                "lz4" => CodecId::Lz4,
                _ => CodecId::Zstd,
            };
            let mut data = Vec::new();
            File::open(input)?.read_to_end(&mut data)?;
            let file = File::create(output)?;
            let mut writer = SixCyWriter::new(file)?;
            writer.add_file(input.file_name().unwrap().to_string_lossy().into_owned(), &data, codec_id)?;
            writer.finalize()?;
            println!("Successfully packed {} into archive", input.display());
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
            println!("{:<20} {:<10} {:<10}", "Name", "Size", "Compressed");
            for record in &reader.index.records {
                println!("{:<20} {:<10} {:<10}", record.name, record.original_size, record.compressed_size);
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
            println!("Feature Bitmap:  {:064b}", sb.feature_bitmap);
        }
        Commands::Optimize { input, output } => {
            let file_in = File::open(input)?;
            let mut reader = SixCyReader::new(file_in)?;
            let file_out = File::create(output)?;
            let mut writer = SixCyWriter::new(file_out)?;
            let records = reader.index.records.clone();
            for record in records {
                let data = reader.unpack_file(record.id)?;
                writer.add_file(record.name, &data, CodecId::Zstd)?;
            }
            writer.finalize()?;
            println!("Archive optimized and saved to {}", output.display());
        }
    }
    Ok(())
}
