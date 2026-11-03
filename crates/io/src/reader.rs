use futures::io::{AsyncRead, AsyncReadExt};
use futures::TryStreamExt;
use opendal::{Buffer, Operator};
use pmc_arrow::{PdfMetadata, PdfRecord};

use crate::validation::validate_limit;
use crate::ReadError;

/// Read one PDF through a caller-configured async OpenDAL operator.
///
/// `path` is relative to the operator's root. `metadata.source_uri` is preserved
/// as supplied; it is not derived from the path or operator configuration.
/// Caller-configured OpenDAL layers provide retries and timeouts.
///
/// Collects native OpenDAL buffer chunks while checking their combined size.
/// A chunk exceeding the remaining limit is rejected before copying the PDF.
/// Accepted chunks are copied once into the `Vec<u8>` required by `PdfRecord`;
/// no futures `AsyncRead` adapter is used on this path. No stat request is made
/// to infer source metadata. The final backend chunk may exceed the limit;
/// this bounds accepted payload size, not network traffic or backend buffering.
pub async fn read_pdf(
    operator: &Operator,
    path: &str,
    metadata: PdfMetadata,
    max_pdf_bytes: usize,
) -> Result<PdfRecord, ReadError> {
    validate_limit(max_pdf_bytes)?;
    let storage_error = |source| ReadError::Storage {
        source_uri: metadata.source_uri.clone(),
        source,
    };
    let reader = operator.reader(path).await.map_err(&storage_error)?;
    let mut stream = reader.into_stream(..).await.map_err(&storage_error)?;
    let mut chunks = Vec::new();
    let mut size = 0;
    while let Some(buffer) = stream.try_next().await.map_err(&storage_error)? {
        if buffer.len() > max_pdf_bytes - size {
            return Err(ReadError::TooLarge {
                source_uri: metadata.source_uri,
                limit: max_pdf_bytes,
            });
        }
        size += buffer.len();
        chunks.extend(buffer);
    }
    let bytes = chunks.into_iter().collect::<Buffer>().to_vec();
    let source_uri = metadata.source_uri.clone();
    PdfRecord::try_new(bytes, metadata)
        .map_err(|source| ReadError::InvalidRecord { source_uri, source })
}

/// A reader for one PDF supplied as a futures async byte stream.
///
/// Owns the stream (or a borrowed `&mut R`), metadata and per-file size limit.
/// Construction validates configuration without performing I/O. [`Self::read`]
/// consumes the reader and materializes the complete PDF in memory.
/// OpenDAL objects use [`read_pdf`] directly to retain native buffer reads.
pub struct PdfReader<R> {
    input: R,
    metadata: PdfMetadata,
    max_pdf_bytes: usize,
    probe_limit: u64,
}

impl<R: AsyncRead + Unpin> PdfReader<R> {
    /// Create a reader with an explicit payload-size limit, without reading bytes.
    ///
    /// The limit must be smaller than `isize::MAX`; zero rejects nonempty input.
    pub fn new(input: R, metadata: PdfMetadata, max_pdf_bytes: usize) -> Result<Self, ReadError> {
        let probe_limit = validate_limit(max_pdf_bytes)?;
        Ok(Self {
            input,
            metadata,
            max_pdf_bytes,
            probe_limit,
        })
    }

    /// Consume this reader and return one validated PDF record.
    ///
    /// Reads at most `max_pdf_bytes + 1` bytes. The probe byte distinguishes an
    /// exact-size PDF from oversized input, which is rejected rather than truncated.
    /// This bounds collected payload length, not allocation capacity or backend
    /// buffering. Subsequent Arrow conversion makes another payload copy.
    ///
    /// Dropping the future stops collection without returning a partial record;
    /// a borrowed stream is not rewound. Metadata is passed to the core unchanged,
    /// and its existing normalization and validation rules apply.
    pub async fn read(self) -> Result<PdfRecord, ReadError> {
        let Self {
            input,
            metadata,
            max_pdf_bytes,
            probe_limit,
        } = self;
        let mut bytes = Vec::new();
        input
            .take(probe_limit)
            .read_to_end(&mut bytes)
            .await
            .map_err(|source| ReadError::Read {
                source_uri: metadata.source_uri.clone(),
                source,
            })?;
        if bytes.len() > max_pdf_bytes {
            return Err(ReadError::TooLarge {
                source_uri: metadata.source_uri,
                limit: max_pdf_bytes,
            });
        }
        let source_uri = metadata.source_uri.clone();
        PdfRecord::try_new(bytes, metadata)
            .map_err(|source| ReadError::InvalidRecord { source_uri, source })
    }
}
