/// Errors from validating a PDF snapshot or constructing its Arrow batch.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid PMCID {0:?}: expected a positive ASCII decimal ID with optional PMC prefix")]
    InvalidPmcid(String),
    #[error("PMC version must be positive when supplied")]
    InvalidVersion,
    #[error("source_uri must not be blank")]
    MissingSourceUri,
    #[error("PDF payload is empty")]
    EmptyPdf,
    #[error("PDF payload must start with %PDF-<digit>.<digit> followed by CR or LF")]
    InvalidPdfHeader,
    #[error("invalid raw_metadata_json: {0}")]
    InvalidMetadataJson(#[from] serde_json::Error),
    #[error("raw_metadata_json must contain a JSON object")]
    MetadataNotObject,
    #[error("batch exceeds the Arrow offset or allocation limit for {0}")]
    BatchTooLarge(&'static str),
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
}

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, Error>;
