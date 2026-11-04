//! Asynchronously read PMC metadata JSON and PDF payloads.
//!
//! Use [`PmcReader`] to read a metadata object and its referenced PDF.
//! For separate reads, use [`read_pdf`] with a caller-configured operator, or
//! [`PdfReader`] with an existing futures `AsyncRead` stream. Read source JSON
//! using [`read_metadata`] or [`MetadataReader`], then map it with
//! [`PmcMetadata::into_pdf_metadata`]. Use [`discover_metadata`] to stream JSON
//! paths under a metadata prefix. No clock reads or dataset writes are performed.
//!
//! ```no_run
//! use pmc_arrow::{to_record_batch, PdfMetadata};
//! use pmc_io::{read_pdf, PdfReader};
//!
//! async fn ingest(operator: &opendal::Operator) -> Result<(), Box<dyn std::error::Error>> {
//!     let metadata = PdfMetadata::new(
//!         "PMC123", "s3://example/PMC123.pdf",
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
//! Read an article from a PMC bucket or matching mirror:
//! ```no_run
//! use pmc_io::PmcReader;
//!
//! async fn ingest_article(
//!     operator: opendal::Operator,
//!     metadata_path: &str,
//! ) -> Result<Option<pmc_arrow::PdfRecord>, pmc_io::PmcReadError> {
//!     PmcReader::new(operator).read(metadata_path).await
//! }
//! ```

mod discovery;
mod error;
mod metadata;
mod metadata_reader;
mod pdf_reader;
mod pmc_reader;
mod validation;

pub use discovery::discover_metadata;
pub use error::{MetadataReadError, PmcReadError, ReadError};
pub use metadata::PmcMetadata;
pub use metadata_reader::{read_metadata, MetadataReader};
pub use pdf_reader::{read_pdf, PdfReader};
pub use pmc_reader::PmcReader;
