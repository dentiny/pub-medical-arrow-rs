//! Read an article from a local mirror of the PMC bucket.
//! Usage: cargo run -p pmc-io --example pmc -- <MIRROR_ROOT> <METADATA_PATH>
use opendal::{services::Fs, Operator};
use pmc_arrow::to_record_batch;
use pmc_io::PmcReader;
use tokio::io::AsyncWriteExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: pmc <MIRROR_ROOT> <METADATA_PATH>".into());
    }
    let root = tokio::fs::canonicalize(&args[0]).await?;
    let operator =
        Operator::new(Fs::default().root(root.to_str().ok_or("invalid mirror directory")?))?;
    let reader = PmcReader::new(operator);
    let summary = match reader.read(&args[1]).await? {
        Some(record) => {
            let batch = to_record_batch(std::slice::from_ref(&record))?;
            format!(
                "{} row, {} columns; {}: {} PDF bytes\n",
                batch.num_rows(),
                batch.num_columns(),
                record.metadata().pmcid,
                record.pdf_size_bytes()
            )
        }
        None => format!("{}: no PDF referenced\n", args[1]),
    };
    let mut stdout = tokio::io::stdout();
    stdout.write_all(summary.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}
