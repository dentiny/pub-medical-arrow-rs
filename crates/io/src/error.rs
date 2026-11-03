/// Errors while opening, reading or validating a PDF source.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("{source_uri}: OpenDAL PDF read failed: {source}")]
    Storage {
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
    #[error("{source_uri}: invalid PDF record: {source}")]
    InvalidRecord {
        source_uri: String,
        #[source]
        source: pmc_arrow::Error,
    },
}
