//! Read one local PDF asynchronously and build an in-memory Arrow batch.
//! Usage: cargo run -p pmc-arrow --example convert -- PMC123 /path/to/article.pdf
use pmc_arrow::{to_record_batch, PdfMetadata, PdfRecord};
use tokio::io::AsyncWriteExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: convert <PMCID> <PDF_PATH>".into());
    }
    let path = tokio::fs::canonicalize(&args[1]).await?;
    let pdf = tokio::fs::read(&path).await?;
    let retrieved_at = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_micros(),
    )?;
    let metadata = PdfMetadata::new(&args[0], path.to_string_lossy(), retrieved_at);
    let record = PdfRecord::try_new(pdf, metadata)?;
    let batch = to_record_batch(std::slice::from_ref(&record))?;
    let summary = format!(
        "{} row, {} columns; {}: {} PDF bytes\n",
        batch.num_rows(),
        batch.num_columns(),
        record.metadata().pmcid,
        record.pdf_size_bytes(),
    );
    let mut stdout = tokio::io::stdout();
    stdout.write_all(summary.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}
