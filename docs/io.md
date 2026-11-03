# Async PDF ingestion

`pmc-io` reads a single PDF into a validated `pmc_arrow::PdfRecord`:

- `read_pdf(&Operator, path, PdfMetadata).await` consumes native
  OpenDAL `Buffer` chunks, without an `AsyncRead` adapter.
- `PdfReader::new(stream, PdfMetadata).read().await` accepts a
  futures async byte stream, including a borrowed `&mut` stream. The synchronous
  constructor stores the input and metadata without I/O; `read(self)` performs asynchronous
  I/O and consumes the reader to produce a single record.

The application configures the operator, credentials, backend features and any
retry/timeout layers. The object path is relative to the operator root. The
source URI in metadata is retained independently of that path; no backend
configuration or credentials are serialized into metadata.

All file and network reads are asynchronous. CPU-only validation, SHA-256 and
Arrow construction use the synchronous core library. The CLI remains a scaffold.

## Reading and buffering

Both paths read the complete PDF without a configured size limit.
The OpenDAL path collects native buffer chunks, retaining their storage until
copied once into the `Vec<u8>` required by the core record. No
`FuturesAsyncReader` or `read_to_end` adapter is used on this path, and there is
no preliminary stat request to determine the object length.

`PdfReader::read` reads an existing `AsyncRead` stream to EOF into a `Vec<u8>`.
Both paths materialize the complete PDF in memory; subsequent Arrow conversion
also copies the payload.

## Metadata and errors

Metadata is supplied explicitly, including PMCID, source URI and UTC microsecond
timestamps. Reads do not infer versions, licenses, article metadata or object
modification times, and do not read the clock. The core's existing normalization
and validation apply after the complete PDF is collected. PDF header validation
is not a complete PDF integrity check.

`ReadError` separates OpenDAL failures, stream-read failures and invalid core
records. Source-related errors
include the caller-provided source URI; wrapped I/O, OpenDAL and core errors are
retained in the error chain. The OpenDAL path returns `ReadError::Storage`;
the generic async stream path returns `ReadError::Read` for I/O failures.
No partially read record is returned on failure.

`PdfReader` owns the input and metadata. Reading still materializes the
complete PDF; the reader type does not change the buffering strategy.

Dropping the read future stops collection without a partial record. A borrowed
stream is not rewound; retrying it requires the caller to reopen or reset it.

Source inventory discovery, multi-file batch scheduling, retries implemented by
this crate and Lance writes are not included in this initial reader API.
