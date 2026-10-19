//! `mart` — the M-ART snapshot command line.
//!
//! ```text
//! mart pack     <dir> -o snap.mart [--base old.mart]
//! mart inspect  snap.mart [--manifest-only]
//! mart verify   snap.mart
//! mart restore  snap.mart <dir>
//! mart diff     old.mart new.mart
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use map_snapshot::{pack, MartArchive, PackOptions, DEFAULT_CHUNK_SIZE};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mart", version, about = "M-ART .mart snapshot tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pack a directory tree into a new .mart archive.
    Pack {
        /// Directory to snapshot.
        root: PathBuf,
        /// Output .mart path.
        #[arg(short, long)]
        out: PathBuf,
        /// Reuse chunk payloads from this existing archive.
        #[arg(long)]
        base: Option<PathBuf>,
        /// Free-form origin label stored in the manifest.
        #[arg(long, default_value = "")]
        source: String,
        /// Chunk size in bytes (default 1 MiB).
        #[arg(long, default_value_t = DEFAULT_CHUNK_SIZE)]
        chunk_size: usize,
    },
    /// Print archive metadata and the MAP snapshot manifest.
    Inspect {
        path: PathBuf,
        /// Print only the MAP snapshot manifest.
        #[arg(long)]
        manifest_only: bool,
    },
    /// Recompute every chunk digest.
    Verify { path: PathBuf },
    /// Restore an archive into a directory.
    Restore { path: PathBuf, dest: PathBuf },
    /// Structural diff between two archives.
    Diff { from: PathBuf, to: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Pack {
            root,
            out,
            base,
            source,
            chunk_size,
        } => {
            let options = PackOptions {
                chunk_size,
                base,
                source,
            };
            let stats = pack(&root, &out, &options)
                .with_context(|| format!("packing {}", root.display()))?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Command::Inspect {
            path,
            manifest_only,
        } => {
            let archive =
                MartArchive::open(&path).with_context(|| format!("opening {}", path.display()))?;
            let value = if manifest_only {
                archive.to_snapshot_json()
            } else {
                archive.inspect_json()
            };
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Command::Verify { path } => {
            let archive =
                MartArchive::open(&path).with_context(|| format!("opening {}", path.display()))?;
            let report = archive.verify()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Restore { path, dest } => {
            let archive =
                MartArchive::open(&path).with_context(|| format!("opening {}", path.display()))?;
            let stats = archive
                .restore(&dest)
                .with_context(|| format!("restoring into {}", dest.display()))?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Command::Diff { from, to } => {
            let old =
                MartArchive::open(&from).with_context(|| format!("opening {}", from.display()))?;
            let new =
                MartArchive::open(&to).with_context(|| format!("opening {}", to.display()))?;
            println!("{}", serde_json::to_string_pretty(&old.diff(&new))?);
        }
    }
    Ok(())
}
