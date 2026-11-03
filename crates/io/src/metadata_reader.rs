use futures::io::{AsyncRead, AsyncReadExt};
use opendal::Operator;

use crate::metadata::parse_metadata;
use crate::{MetadataReadError, PmcMetadata};

/// Read a PMC article-version JSON object through a caller-configured operator.
///
/// `path` is relative to the operator root and identifies the metadata source in
/// errors. Uses native OpenDAL buffers without a futures `AsyncRead` adapter.
/// Reads only the JSON object; it does not fetch the PDF or other linked files.
pub async fn read_metadata(
    operator: &Operator,
    path: &str,
) -> Result<PmcMetadata, MetadataReadError> {
    let buffer = operator
        .read(path)
        .await
        .map_err(|source| MetadataReadError::Storage {
            source_uri: path.into(),
            source,
        })?;
    parse_metadata(buffer.to_vec(), path)
}

/// Reader for a single PMC article-version JSON object from a futures byte stream.
pub struct MetadataReader<R> {
    input: R,
    source_uri: String,
}

impl<R: AsyncRead + Unpin> MetadataReader<R> {
    /// Store the input and its location for diagnostics, without performing I/O.
    pub fn new(input: R, source_uri: impl Into<String>) -> Self {
        Self {
            input,
            source_uri: source_uri.into(),
        }
    }

    /// Consume the reader, read the complete JSON to EOF, and parse its metadata.
    ///
    /// The original JSON is retained verbatim. No size limit or timestamp is
    /// inferred. Dropping the future does not rewind a borrowed stream.
    pub async fn read(self) -> Result<PmcMetadata, MetadataReadError> {
        let Self {
            mut input,
            source_uri,
        } = self;
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .await
            .map_err(|source| MetadataReadError::Read {
                source_uri: source_uri.clone(),
                source,
            })?;
        parse_metadata(bytes, &source_uri)
    }
}
