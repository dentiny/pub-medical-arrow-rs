use std::{num::NonZeroUsize, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "pmc-arrow",
    version,
    about = "Convert PMC metadata and PDFs into an Arrow IPC file"
)]
pub struct Args {
    /// Local mirror directory or s3://pmc-oa-opendata
    pub source: String,

    /// Local Arrow IPC output file (must not already exist)
    pub output: PathBuf,

    /// Recursively discover metadata JSON under this source-relative prefix
    #[arg(long, default_value = "metadata/")]
    pub prefix: String,

    /// Maximum number of concurrent article reads (output order is unspecified)
    #[arg(long, default_value = "8")]
    pub parallelism: NonZeroUsize,

    /// Maximum rows per Arrow batch
    #[arg(long, visible_alias = "batch-size", default_value = "32")]
    pub max_rows: NonZeroUsize,

    /// Maximum estimated Arrow buffer bytes per batch (not process memory)
    #[arg(long, default_value = "67108864")]
    pub max_memory_bytes: NonZeroUsize,
}
