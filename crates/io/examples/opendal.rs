//! Read a local PDF through OpenDAL and convert it to an Arrow batch.
//! Usage: cargo run -p pmc-io --example opendal -- PMC123 /path/to/article.pdf
use opendal::{services::Fs, Operator};
use pmc_arrow::{to_record_batch, PdfMetadata};
use pmc_io::read_pdf;
use tokio::io::AsyncWriteExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: opendal <PMCID> <PDF_PATH>".into());
    }
    let path = tokio::fs::canonicalize(&args[1]).await?;
    let root = path
        .parent()
        .and_then(|p| p.to_str())
        .ok_or("invalid PDF directory")?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid PDF filename")?;
    let operator = Operator::new(Fs::default().root(root))?;
    let metadata = PdfMetadata::new(&args[0], path.to_string_lossy());
    let record = read_pdf(&operator, name, metadata).await?;
    let batch = to_record_batch(std::slice::from_ref(&record))?;
    let summary = format!(
        "{} row, {} columns; {}: {} PDF bytes\n",
        batch.num_rows(),
        batch.num_columns(),
        record.metadata().pmcid,
        record.pdf_size_bytes()
    );
    let mut stdout = tokio::io::stdout();
    stdout.write_all(summary.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}
