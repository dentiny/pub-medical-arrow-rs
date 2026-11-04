//! Discover metadata, read articles concurrently, and build Arrow batches.
//! Usage: cargo run -p pmc-io --example ingest -- <MIRROR_ROOT> <PREFIX> [PARALLELISM] [MAX_ROWS] [MAX_MEMORY_BYTES]
use futures::{pin_mut, TryStreamExt};
use opendal::{services::Fs, Operator};
use pmc_arrow::{PdfBatchBuilder, PdfBatchConfig};
use pmc_io::{discover_metadata, PmcReader};
use tokio::io::AsyncWriteExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=5).contains(&args.len()) {
        return Err(
            "usage: ingest <MIRROR_ROOT> <PREFIX> [PARALLELISM] [MAX_ROWS] [MAX_MEMORY_BYTES]"
                .into(),
        );
    }
    let parallelism = args
        .get(2)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(8);
    let mut config = PdfBatchConfig::default();
    if let Some(max_rows) = args.get(3) {
        config.max_rows = max_rows.parse()?;
    }
    if let Some(max_memory_bytes) = args.get(4) {
        config.max_memory_bytes = max_memory_bytes.parse()?;
    }
    let mut builder = PdfBatchBuilder::new(config)?;
    let root = tokio::fs::canonicalize(&args[0]).await?;
    let operator =
        Operator::new(Fs::default().root(root.to_str().ok_or("invalid mirror directory")?))?;
    let paths = discover_metadata(&operator, &args[1]).await?;
    let reader = PmcReader::new(operator);
    let records = reader.read_many(paths, parallelism);
    pin_mut!(records);

    let mut stdout = tokio::io::stdout();
    while let Some(record) = records.try_next().await? {
        if let Some(batch) = builder.append(record)? {
            let summary = format!(
                "batch: {} rows, {} columns\n",
                batch.num_rows(),
                batch.num_columns()
            );
            stdout.write_all(summary.as_bytes()).await?;
        }
    }
    if let Some(batch) = builder.flush()? {
        let summary = format!(
            "batch: {} rows, {} columns\n",
            batch.num_rows(),
            batch.num_columns()
        );
        stdout.write_all(summary.as_bytes()).await?;
    }
    stdout.flush().await?;
    Ok(())
}
