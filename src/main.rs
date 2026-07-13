use clap::{Parser, Subcommand};
use afptool_rs::{unpack_file, pack_rkfw, pack_rkaf};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "afptool-rs")]
#[command(about = "A Rust tool for packing and unpacking RockChip firmware images")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Unpack {
        #[arg(help = "Path to the firmware file (RKFW or RKAF format)")]
        input: String,

        #[arg(help = "Directory where extracted files will be saved")]
        output: String,
    },

    PackRkfw {
        #[arg(help = "Directory containing BOOT and embedded-update.img files")]
        input: String,

        #[arg(help = "Output RKFW firmware image file path")]
        output: String,

        #[arg(short, long, help = "Chip name (e.g., RK3588, RK3566, RK3562, RK3399, PX30, RK32XX); optional when rkfw-header.bin from unpack is present")]
        chip: Option<String>,

        #[arg(short, long, help = "Version in format: major.minor.build (e.g., 8.1.0); optional when rkfw-header.bin from unpack is present")]
        version: Option<String>,

        #[arg(short, long, help = "Unix timestamp for build date (e.g., 1731031994); optional when rkfw-header.bin from unpack is present")]
        timestamp: Option<i64>,

        #[arg(long, help = "Code field as hex string (e.g., 0x02000000); optional when rkfw-header.bin from unpack is present")]
        code: Option<String>,
    },

    PackRkaf {
        #[arg(help = "Directory containing package-file and files to pack")]
        input: String,

        #[arg(help = "Output RKAF update image file path")]
        output: String,

        #[arg(short, long, help = "Model name")]
        model: String,

        #[arg(short = 'M', long, help = "Manufacturer name")]
        manufacturer: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Unpack { input, output } => {
            unpack_file(&input, &output)?;
        }
        Commands::PackRkfw{ input, output, chip, version, timestamp, code } => {
            pack_rkfw(&input, &output, chip.as_deref(), version.as_deref(), timestamp, code.as_deref())?;
        }
        Commands::PackRkaf { input, output, model, manufacturer } => {
            pack_rkaf(&input, &output, &model, &manufacturer)?;
        }
    }

    Ok(())
}