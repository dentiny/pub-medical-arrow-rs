use futures::io::{AsyncRead, AsyncReadExt};
use opendal::Operator;
use pmc_arrow::{PdfMetadata, PdfRecord};

use crate::ReadError;

/// Read one PDF through a caller-configured async OpenDAL operator.
///
/// `path` is relative to the operator's root. `metadata.source_uri` is preserved
/// as supplied; it is not derived from the path or operator configuration.
/// Caller-configured OpenDAL layers provide retries and timeouts.
///
/// Reads a native OpenDAL buffer and copies it once into the
/// `Vec<u8>` required by `PdfRecord`;
/// no futures `AsyncRead` adapter is used on this path. No stat request is made
/// to infer source metadata. The complete PDF is materialized in memory.
pub async fn read_pdf(
    operator: &Operator,
    path: &str,
    metadata: PdfMetadata,
) -> Result<PdfRecord, ReadError> {
    let bytes = operator
        .read(path)
        .await
        .map_err(|source| ReadError::Storage {
            source_uri: metadata.source_uri.clone(),
            source,
        })?
        .to_vec();
    let source_uri = metadata.source_uri.clone();
    PdfRecord::try_new(bytes, metadata)
        .map_err(|source| ReadError::InvalidRecord { source_uri, source })
}

/// A reader for one PDF supplied as a futures async byte stream.
///
/// Owns the stream (or a borrowed `&mut R`) and metadata.
/// Construction performs no I/O. [`Self::read`]
/// consumes the reader and materializes the complete PDF in memory.
/// OpenDAL objects use [`read_pdf`] directly to retain native buffer reads.
pub struct PdfReader<R> {
    input: R,
    metadata: PdfMetadata,
}

impl<R: AsyncRead + Unpin> PdfReader<R> {
    /// Create a reader without performing I/O.
    pub fn new(input: R, metadata: PdfMetadata) -> Self {
        Self { input, metadata }
    }

    /// Consume this reader and return one validated PDF record.
    ///
    /// Reads the stream to EOF and materializes the complete PDF in memory.
    /// Subsequent Arrow conversion makes another payload copy.
    ///
    /// Dropping the future stops collection without returning a partial record;
    /// a borrowed stream is not rewound. Metadata is passed to the core unchanged,
    /// and its existing normalization and validation rules apply.
    pub async fn read(self) -> Result<PdfRecord, ReadError> {
        let Self {
            mut input,
            metadata,
        } = self;
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .await
            .map_err(|source| ReadError::Read {
                source_uri: metadata.source_uri.clone(),
                source,
            })?;
        let source_uri = metadata.source_uri.clone();
        PdfRecord::try_new(bytes, metadata)
            .map_err(|source| ReadError::InvalidRecord { source_uri, source })
    }
}
