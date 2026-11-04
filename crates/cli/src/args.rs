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

    /// Maximum rows per Arrow batch, not a byte limit
    #[arg(long, default_value = "32")]
    pub batch_size: NonZeroUsize,

    /// Compute SHA-256 for each entire PDF (does not verify a source checksum)
    #[arg(long)]
    pub compute_pdf_sha256: bool,
}
