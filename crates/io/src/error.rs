/// Errors while opening, reading or validating a PDF source.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("max_pdf_bytes must be smaller than isize::MAX")]
    InvalidSizeLimit,
    #[error("{source_uri}: failed to open PDF source: {source}")]
    Open {
        source_uri: String,
        #[source]
        source: opendal::Error,
    },
    #[error("{source_uri}: failed to read PDF bytes: {source}")]
    Read {
        source_uri: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{source_uri}: PDF payload exceeds the {limit} byte limit")]
    TooLarge { source_uri: String, limit: usize },
    #[error("{source_uri}: invalid PDF record: {source}")]
    InvalidRecord {
        source_uri: String,
        #[source]
        source: pmc_arrow::Error,
    },
}
