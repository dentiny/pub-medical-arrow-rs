//! Discover metadata, read articles concurrently, and build Arrow batches.
//! Usage: cargo run -p pmc-io --example ingest -- <MIRROR_ROOT> <PREFIX> <PARALLELISM> <BATCH_SIZE>
use futures::{pin_mut, TryStreamExt};
use opendal::{services::Fs, Operator};
use pmc_arrow::PdfBatchBuilder;
use pmc_io::{discover_metadata, PmcReader};
use tokio::io::AsyncWriteExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: ingest <MIRROR_ROOT> <PREFIX> <PARALLELISM> <BATCH_SIZE>".into());
    }
    let parallelism = args[2].parse()?;
    let mut builder = PdfBatchBuilder::new(args[3].parse()?)?;
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
