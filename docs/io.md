# Async PDF ingestion

`pmc-io` reads a single PDF into a validated `pmc_arrow::PdfRecord`:

- `read_pdf(&Operator, path, PdfMetadata, max_pdf_bytes).await` consumes native
  OpenDAL `Buffer` chunks, without an `AsyncRead` adapter.
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
than `isize::MAX`; zero rejects every nonempty payload. A file exactly at the
limit is accepted if valid; a larger file returns `ReadError::TooLarge`.
Neither path converts a truncated prefix into a record.

The OpenDAL path collects native buffer chunks and checks their combined length
before retaining each chunk. A chunk that would exceed the limit is rejected;
it may already have been fetched by the backend. Accepted chunks retain their
native storage until they are copied once into the `Vec<u8>` required by the
core record. No `FuturesAsyncReader` or `read_to_end` adapter is used on this path.
There is no preliminary stat request to determine the object length.

`read_pdf_from` handles existing `AsyncRead` streams. It collects at most the
limit plus one probe byte to distinguish an exact-size file from oversized
input. Both paths check actual returned bytes rather than trusting a source
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

`ReadError` separates invalid size limits, OpenDAL failures, stream-read
failures, oversized payloads and invalid core records. Source-related errors
include the caller-provided source URI; wrapped I/O, OpenDAL and core errors are
retained in the error chain. The OpenDAL path returns `ReadError::Storage`;
the generic async stream path returns `ReadError::Read` for I/O failures.
No partially read record is returned on failure.

Dropping the read future stops collection without a partial record. A borrowed
stream is not rewound; retrying it requires the caller to reopen or reset it.

Source inventory discovery, multi-file batch scheduling, retries implemented by
this crate and Lance writes are not included in this initial reader API.
