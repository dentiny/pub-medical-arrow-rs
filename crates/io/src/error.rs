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

/// Errors while reading or parsing a PMC metadata JSON source.
#[derive(Debug, thiserror::Error)]
pub enum MetadataReadError {
    #[error("{source_uri}: OpenDAL metadata read failed: {source}")]
    Storage {
        source_uri: String,
        #[source]
        source: opendal::Error,
    },
    #[error("{source_uri}: failed to read metadata bytes: {source}")]
    Read {
        source_uri: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{source_uri}: metadata is not UTF-8: {source}")]
    InvalidEncoding {
        source_uri: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("{source_uri}: invalid PMC metadata JSON: {source}")]
    InvalidJson {
        source_uri: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{source_uri}: PMC metadata must be a JSON object")]
    NotObject { source_uri: String },
}
