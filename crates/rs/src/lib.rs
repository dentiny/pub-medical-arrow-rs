//! Validated PMC PDF snapshots and their fixed Arrow schema.
//!
//! Callers supply PDF bytes and source metadata. The constructor checks the PDF
//! header and metadata, normalizes the PMCID and computes SHA-256. It performs
//! no network access, file I/O or complete PDF parsing.

mod error;
mod model;
mod schema;

pub use error::{Error, Result};
pub use model::{PdfMetadata, PdfRecord};
pub use schema::{schema, SCHEMA_VERSION};
