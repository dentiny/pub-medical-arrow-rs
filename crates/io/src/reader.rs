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
/// The entire accepted PDF is buffered. See [`read_pdf_from`] for payload limit
/// and memory semantics. No stat request is used to infer timestamps or size.
/// Object access can be deferred until the stream is read.
pub async fn read_pdf(
    operator: &Operator,
    path: &str,
    metadata: PdfMetadata,
    max_pdf_bytes: usize,
) -> Result<PdfRecord, ReadError> {
    validate_limit(max_pdf_bytes)?;
    let input = async {
        operator
            .reader(path)
            .await?
            .into_futures_async_read(..)
            .await
    }
    .await
    .map_err(|source| ReadError::Open {
        source_uri: metadata.source_uri.clone(),
        source,
    })?;
    read_pdf_from(input, metadata, max_pdf_bytes).await
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
