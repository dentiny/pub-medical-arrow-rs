//! Asynchronously read a PDF into a validated `pmc_arrow::PdfRecord`.
//!
//! Use [`read_pdf`] with a caller-configured OpenDAL operator, or
//! [`PdfReader`] with an existing futures `AsyncRead` stream. Supply a
//! source metadata explicitly. No discovery, clock
//! reads, metadata inference or dataset writes are performed here.
//!
//! ```no_run
//! use pmc_arrow::{to_record_batch, PdfMetadata};
//! use pmc_io::{read_pdf, PdfReader};
//!
//! async fn ingest(operator: &opendal::Operator) -> Result<(), Box<dyn std::error::Error>> {
//!     let metadata = PdfMetadata::new(
//!         "PMC123", "s3://example/PMC123.pdf", 1_700_000_000_000_000,
//!     );
//!     let record = read_pdf(operator, "PMC123.pdf", metadata).await?;
//!     let batch = to_record_batch(&[record])?;
//!     assert_eq!(batch.num_rows(), 1);
//!     Ok(())
//! }
//!
//! async fn ingest_stream(
//!     input: impl futures::io::AsyncRead + Unpin,
//!     metadata: PdfMetadata,
//! ) -> Result<pmc_arrow::PdfRecord, pmc_io::ReadError> {
//!     PdfReader::new(input, metadata).read().await
//! }
//! ```

mod error;
mod pdf_reader;

pub use error::ReadError;
pub use pdf_reader::{read_pdf, PdfReader};
