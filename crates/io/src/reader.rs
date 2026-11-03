use futures::io::{AsyncRead, AsyncReadExt};
use futures::TryStreamExt;
use opendal::{Buffer, Operator};
use pmc_arrow::{PdfMetadata, PdfRecord};

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

/// Read one PDF from an async byte stream and validate it with `pmc-arrow`.
///
/// Reads at most `max_pdf_bytes + 1` bytes from the supplied stream. The extra
/// byte distinguishes an exact-size PDF from an oversized PDF, which is rejected
/// rather than truncated. Zero rejects all nonempty input. The limit must be
/// smaller than `isize::MAX` so the probe byte fits in an addressable vector.
///
/// This bounds collected payload length, not total memory or backend prefetch.
/// Vector spare capacity, backend buffers and the subsequent Arrow copy consume
/// additional memory. Dropping the future cancels this read without yielding a
/// partial record; it does not rewind a caller-owned stream borrowed as `&mut R`.
///
/// Metadata and PDF bytes are passed unchanged to `PdfRecord::try_new`; its
/// normalization and validation rules apply. No timestamps are synthesized.
pub async fn read_pdf_from<R: AsyncRead + Unpin>(
    input: R,
    metadata: PdfMetadata,
    max_pdf_bytes: usize,
) -> Result<PdfRecord, ReadError> {
    let probe_limit = validate_limit(max_pdf_bytes)?;
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

fn validate_limit(limit: usize) -> Result<u64, ReadError> {
    if limit >= isize::MAX as usize {
        return Err(ReadError::InvalidSizeLimit);
    }
    Ok(limit as u64 + 1)
}
