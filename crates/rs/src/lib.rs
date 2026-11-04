//! Store PMC PDFs as Arrow rows without changing their bytes.
//!
//! Each row is one PDF snapshot. Callers supply source metadata;
//! this crate performs no network access, file I/O, PDF text extraction or Lance
//! writes. PDF validation is limited to the header, not document integrity.
//!
//! ```
//! use pmc_arrow::{to_record_batch, PdfMetadata, PdfRecord};
//!
//! // Header-only fixture for this example, not a complete PDF document.
//! let metadata = PdfMetadata::new("PMC123", "file:///data/PMC123.pdf");
//! let record = PdfRecord::try_new(b"%PDF-1.7\nexample".to_vec(), metadata)?;
//! let batch = to_record_batch(&[record])?;
//! assert_eq!(batch.num_rows(), 1);
//! assert_eq!(batch.num_columns(), 11);
//! # Ok::<(), pmc_arrow::Error>(())
//! ```

mod batch;
mod convert;
mod error;
mod model;
mod schema;
mod validation;

pub use batch::{PdfBatchBuilder, PdfBatchConfig};
pub use convert::to_record_batch;
pub use error::{Error, Result};
pub use model::{PdfMetadata, PdfRecord};
pub use schema::schema;
