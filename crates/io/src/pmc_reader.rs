use opendal::Operator;
use pmc_arrow::{PdfRecord, PdfRecordOptions};

use crate::validation::validate_pdf_path;
use crate::{read_metadata, read_pdf_with_options, PmcReadError};

/// Read an article's metadata and its referenced PDF through one operator.
///
/// The operator root must represent the root of `pmc-oa-opendata`, or a mirror
/// with the same object paths. Backend configuration remains caller-controlled.
pub struct PmcReader {
    operator: Operator,
    options: PdfRecordOptions,
}

impl PmcReader {
    /// Store the operator without performing I/O.
    pub fn new(operator: Operator) -> Self {
        Self {
            operator,
            options: PdfRecordOptions::default(),
        }
    }

    /// Configure all records read by [`Self::read`] or [`Self::read_many`].
    /// SHA-256 computation is off by default; enabling it hashes entire PDFs
    /// synchronously, without verifying a source checksum.
    pub fn with_options(mut self, options: PdfRecordOptions) -> Self {
        self.options = options;
        self
    }

    /// Read one metadata object and the PDF named by its `pdf_url`.
    ///
    /// `metadata_path` is relative to the operator root.
    /// PDF modification time remains unknown; no stat request is made.
    /// Missing or null `pdf_url` returns `Ok(None)` without a PDF read.
    /// The full PDF is materialized in memory, with no configured size limit.
    pub async fn read(&self, metadata_path: &str) -> Result<Option<PdfRecord>, PmcReadError> {
        let article = read_metadata(&self.operator, metadata_path).await?;
        let Some(metadata) = article.into_pdf_metadata(None)? else {
            return Ok(None);
        };
        let path = validate_pdf_path(&metadata.source_uri)?;
        Ok(Some(
            read_pdf_with_options(&self.operator, &path, metadata, self.options).await?,
        ))
    }
}
