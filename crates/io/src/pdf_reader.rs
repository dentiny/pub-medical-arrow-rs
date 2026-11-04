use futures::io::{AsyncRead, AsyncReadExt};
use futures::TryStreamExt;
use opendal::{Buffer, Operator};
use pmc_arrow::{PdfMetadata, PdfRecord, PdfRecordOptions};

use crate::ReadError;

/// Read one PDF through a caller-configured async OpenDAL operator.
///
/// `path` is relative to the operator's root. `metadata.source_uri` is preserved
/// as supplied; it is not derived from the path or operator configuration.
/// Caller-configured OpenDAL layers provide retries and timeouts.
/// SHA-256 computation is disabled; use [`read_pdf_with_options`] to enable it.
///
/// Collects native OpenDAL buffer chunks and copies them once into the
/// `Vec<u8>` required by `PdfRecord`;
/// no futures `AsyncRead` adapter is used on this path. No stat request is made
/// to infer source metadata. The complete PDF is materialized in memory.
pub async fn read_pdf(
    operator: &Operator,
    path: &str,
    metadata: PdfMetadata,
) -> Result<PdfRecord, ReadError> {
    read_pdf_with_options(operator, path, metadata, PdfRecordOptions::default()).await
}

/// Read one PDF with optional whole-file SHA-256 computation.
///
/// Performs the same reads and validation as [`read_pdf`]. If enabled, hashing
/// runs synchronously after the complete PDF has been read; no expected checksum
/// is verified.
pub async fn read_pdf_with_options(
    operator: &Operator,
    path: &str,
    metadata: PdfMetadata,
    options: PdfRecordOptions,
) -> Result<PdfRecord, ReadError> {
    let storage_error = |source| ReadError::Storage {
        source_uri: metadata.source_uri.clone(),
        source,
    };
    let reader = operator.reader(path).await.map_err(&storage_error)?;
    let mut stream = reader.into_stream(..).await.map_err(&storage_error)?;
    let mut chunks = Vec::new();
    while let Some(buffer) = stream.try_next().await.map_err(&storage_error)? {
        chunks.extend(buffer);
    }
    let bytes = chunks.into_iter().collect::<Buffer>().to_vec();
    let source_uri = metadata.source_uri.clone();
    PdfRecord::try_new_with_options(bytes, metadata, options)
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
    options: PdfRecordOptions,
}

impl<R: AsyncRead + Unpin> PdfReader<R> {
    /// Create a reader without performing I/O or enabling SHA-256 computation.
    pub fn new(input: R, metadata: PdfMetadata) -> Self {
        Self {
            input,
            metadata,
            options: PdfRecordOptions::default(),
        }
    }

    /// Configure record construction. SHA-256 computation is off by default.
    pub fn with_options(mut self, options: PdfRecordOptions) -> Self {
        self.options = options;
        self
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
            options,
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
        PdfRecord::try_new_with_options(bytes, metadata, options)
            .map_err(|source| ReadError::InvalidRecord { source_uri, source })
    }
}
