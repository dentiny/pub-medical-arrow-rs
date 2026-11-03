//! Asynchronously read a PDF into a validated `pmc_arrow::PdfRecord`.
//!
//! Use [`read_pdf`] with a caller-configured OpenDAL operator, or
//! [`read_pdf_from`] with an existing futures `AsyncRead` stream. Supply a
//! per-file payload limit and source metadata explicitly. No discovery, clock
//! reads, metadata inference or dataset writes are performed here.
//!
//! ```no_run
//! use pmc_arrow::{to_record_batch, PdfMetadata};
//! use pmc_io::read_pdf;
//!
//! async fn ingest(operator: &opendal::Operator) -> Result<(), Box<dyn std::error::Error>> {
//!     let metadata = PdfMetadata::new(
//!         "PMC123", "s3://example/PMC123.pdf", 1_700_000_000_000_000,
//!     );
//!     let record = read_pdf(operator, "PMC123.pdf", metadata, 64 * 1024 * 1024).await?;
//!     let batch = to_record_batch(&[record])?;
//!     assert_eq!(batch.num_rows(), 1);
//!     Ok(())
//! }
//! ```

mod error;
mod reader;

pub use error::ReadError;
pub use reader::{read_pdf, read_pdf_from};
