use std::path::PathBuf;

use clap::{Args, Command, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "fo",
    version,
    about = "Local-first file organizer with dry-run and undo"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a default config file
    Init {
        /// Overwrite an existing config
        #[arg(long)]
        force: bool,
    },
    /// Scan directories and classify files (dry-run unless --apply)
    Scan(ScanArgs),
    /// Scan and apply in one step (same as `scan --apply`)
    Apply(ScanArgs),
    /// Undo the last applied operation
    Undo {
        /// Undo this specific journal file
        #[arg(long, conflicts_with = "last")]
        journal: Option<PathBuf>,
        /// Undo the last N operations
        #[arg(long, default_value_t = 1)]
        last: usize,
        /// Config file (used to locate the journal directory)
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Print the resolved config path and contents
    Config {
        /// Print only the path
        #[arg(long)]
        path: bool,
        /// Config file to use instead of the platform default
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// One or more directories to scan
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    /// Path to config (defaults to the platform config dir)
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Actually move files (default: dry-run)
    #[arg(long)]
    pub apply: bool,
    /// Emit machine-readable JSON
    #[arg(long)]
    pub json: bool,
    /// Skip files smaller than this many bytes
    #[arg(long, value_name = "BYTES")]
    pub min_size: Option<u64>,
}
