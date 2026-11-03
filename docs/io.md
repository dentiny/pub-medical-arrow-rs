# Async PDF ingestion

`pmc-io` reads a single PDF into a validated `pmc_arrow::PdfRecord`:

- `read_pdf(&Operator, path, PdfMetadata, max_pdf_bytes).await` uses OpenDAL.
- `read_pdf_from(impl AsyncRead + Unpin, PdfMetadata, max_pdf_bytes).await`
  accepts a futures async byte stream, including a borrowed `&mut` reader.

The application configures the operator, credentials, backend features and any
retry/timeout layers. The object path is relative to the operator root. The
source URI in metadata is retained independently of that path; no backend
configuration or credentials are serialized into metadata.

All file and network reads are asynchronous. CPU-only validation, SHA-256 and
Arrow construction use the synchronous core library. The CLI remains a scaffold.

## Payload limits

Callers must provide a maximum PDF payload size in bytes. It must be smaller
than `isize::MAX`; zero rejects every nonempty payload. The library collects at
most the limit plus one probe byte. A file exactly at the limit is accepted if
valid; a larger file returns `ReadError::TooLarge`. It never converts a truncated
prefix into a record. The limit is enforced on bytes received, not a source
content-length header.

This is a payload length limit, not a total memory budget. The PDF vector may
have spare capacity and the backend may buffer or prefetch data. Subsequent
Arrow conversion also copies the payload. Callers should bound concurrent reads
and batch sizes separately.

## Metadata and errors

Metadata is supplied explicitly, including PMCID, source URI and UTC microsecond
timestamps. Reads do not infer versions, licenses, article metadata or object
modification times, and do not read the clock. The core's existing normalization
and validation apply after the complete PDF is collected. PDF header validation
is not a complete PDF integrity check.

`ReadError` separates invalid size limits, source-opening failures, stream-read
failures, oversized payloads and invalid core records. Source-related errors
include the caller-provided source URI; wrapped I/O, OpenDAL and core errors are
retained in the error chain. OpenDAL may defer object access until stream reading,
so failures such as a missing file can be reported while opening or reading.
No partially read record is returned on failure.

Dropping the read future stops collection without a partial record. A borrowed
stream is not rewound; retrying it requires the caller to reopen or reset it.

Source inventory discovery, multi-file batch scheduling, retries implemented by
this crate and Lance writes are not included in this initial reader API.
