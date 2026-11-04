//! Asynchronously read PMC metadata JSON and PDF payloads.
//!
//! Use [`read_pdf`] with a caller-configured OpenDAL operator, or
//! [`PdfReader`] with an existing futures `AsyncRead` stream. Read source JSON
//! using [`read_metadata`] or [`MetadataReader`], then map it with
//! [`PmcMetadata::into_pdf_metadata`]. No discovery, clock reads or dataset
//! writes are performed here.
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
//!
//! Read metadata and a caller-selected PDF object using the same operator.
//!
//! The PDF path is explicit: a source URL may include checksum query parameters
//! and is not an operator-relative path.
//! ```no_run
//! use pmc_io::{read_metadata, read_pdf};
//!
//! async fn ingest_article(
//!     operator: &opendal::Operator,
//!     metadata_path: &str,
//!     pdf_path: &str,
//!     retrieved_at: i64,
//! ) -> Result<Option<pmc_arrow::PdfRecord>, Box<dyn std::error::Error>> {
//!     let article = read_metadata(operator, metadata_path).await?;
//!     let Some(metadata) = article.into_pdf_metadata(retrieved_at, None)? else {
//!         return Ok(None);
//!     };
//!     Ok(Some(read_pdf(operator, pdf_path, metadata).await?))
//! }
//! ```

mod error;
mod metadata;
mod metadata_reader;
mod pdf_reader;

pub use error::{MetadataReadError, ReadError};
pub use metadata::PmcMetadata;
pub use metadata_reader::{read_metadata, MetadataReader};
pub use pdf_reader::{read_pdf, PdfReader};
