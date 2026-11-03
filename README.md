# PMC Arrow

Convert PubMed Central PDF bytes and source metadata into Apache Arrow
`RecordBatch` values. Each row is one PDF file snapshot, retaining the complete
original PDF bytes, a SHA-256 checksum and source provenance for downstream
dataset storage.

## Workspace

| Directory | Crate | Status |
| --- | --- | --- |
| `crates/rs` | `pmc-arrow` | Implemented: model, validation, schema and in-memory Arrow conversion |
| `crates/io` | `pmc-io` | Implemented: async PDF and PMC metadata reads via OpenDAL or byte streams |
| `crates/cli` | `pmc-cli` | Placeholder; exits with an explicit not-implemented error |

Dependency declarations match the local `atif-arrow` workspace: Arrow `58` and
OpenDAL `0.59.3` with default features disabled and `executors-tokio` /
`http-transport-reqwest` enabled. The lockfile aligns the Arrow family at `58.4.0`
and OpenDAL (including core and the HTTP transport) at `0.59.4`. Use `--locked`
to retain this alignment; coordinate dependency updates with `atif-arrow`.

## Use the Rust library

From another local project:

```toml
[dependencies]
pmc-arrow = { path = "/path/to/pub-medical-arrow-rs/crates/rs" }
```

```rust
use pmc_arrow::{to_record_batch, PdfMetadata, PdfRecord};

fn convert(pdf: Vec<u8>) -> pmc_arrow::Result<arrow_array::RecordBatch> {
    // Timestamps are UTC microseconds since the Unix epoch, supplied by the caller.
    let mut metadata = PdfMetadata::new(
        "PMC123",
        "file:///data/PMC123.pdf",
        1_700_000_000_000_000,
    );
    metadata.title = Some("Example article".into());
    // Leave version, PMID, DOI and license absent unless the source provides them.
    let record = PdfRecord::try_new(pdf, metadata)?;
    to_record_batch(&[record])
}
```

The function signature above uses `arrow_array::RecordBatch`; add
`arrow-array = "58"` if you name Arrow types directly in your application.
The record constructor takes ownership of the PDF vector. Its fields are
immutable after validation; read them through `metadata()`, `pdf_bytes()`,
`pdf_sha256()` and `pdf_size_bytes()`.

Run the included example on a local PDF:

```sh
cargo run --locked -p pmc-arrow --example convert -- PMC123 /path/to/article.pdf
```

Or use the synthetic one-page fixture:

```sh
cargo run --locked -p pmc-arrow --example convert -- PMC123 crates/rs/tests/fixtures/tiny.pdf
```

The example uses Tokio for asynchronous file reads, path canonicalization and
stdout writes, then builds an in-memory batch. It does not write a dataset.
Tokio is a development dependency of `pmc-arrow` for the example.

## Read through OpenDAL

`pmc_io::read_pdf(&operator, path, metadata).await` reads one PDF
through a caller-configured OpenDAL operator using native `Buffer` chunks
and returns a validated `PdfRecord`, without an `AsyncRead` adapter.
Use `PdfReader::new(stream, metadata).read().await` for an
existing futures `AsyncRead` stream. `new` performs no I/O; async `read` consumes
the reader and returns one PDF record. Pass accepted records to `pmc_arrow::to_record_batch`.

```sh
cargo run --locked -p pmc-io --example opendal -- PMC123 crates/rs/tests/fixtures/tiny.pdf
```

The example uses the filesystem backend. Backend features
are enabled by the application; filesystem support is a development dependency.
See [I/O semantics and error handling](docs/io.md).

## Read PMC metadata

`pmc_io::read_metadata(&operator, metadata_path).await` reads one PMC JSON object.
For an existing futures byte stream, use
`MetadataReader::new(input, metadata_uri).read().await`. Both return `PmcMetadata`,
including articles that have no PDF.

```rust
let article = pmc_io::read_metadata(operator, "metadata/PMC123.2.json").await?;
if let Some(metadata) = article.into_pdf_metadata(retrieved_at, None) {
    // The object path is supplied separately from the source PDF URL.
    let record = pmc_io::read_pdf(operator, "PMC123.2/article.pdf", metadata).await?;
    let batch = pmc_arrow::to_record_batch(&[record])?;
}
```

Source JSON is preserved verbatim. The adapter maps `version`, `license_code`
and `pdf_url` into the PDF snapshot fields; numeric PMIDs become decimal strings.
Retrieval and PDF modification times remain explicit caller inputs in UTC
microseconds. See [metadata mapping and errors](docs/metadata.md).

## Conversion contract

- [Schema and validation details](docs/schema.md).
- PDF data is `LargeBinary`, the digest is `FixedSizeBinary(32)`, and long text
  fields are `LargeUtf8`. Timestamps carry a UTC timezone in Arrow.
- Missing optional values stay null; supplied empty strings stay empty. Unknown
  versions are never replaced with version 1.
- Raw JSON must be an object and is retained verbatim, including unknown fields,
  large numbers, duplicate keys and whitespace. The core accepts explicit
  fields; the I/O crate maps PMC source JSON as described above.
- PDF validation checks only the header. It is not a PDF parser and cannot
  certify completeness, renderability or safety.
- Batch conversion preserves order and duplicates. It does not select a preferred
  article version, infer licensing, deduplicate or read source URIs.
- PDF bytes are copied into the Arrow column once. Caller-owned records remain
  allocated until dropped. Limit both rows and bytes before calling the converter;
  the core library does not impose a batch budget or catch allocation exhaustion.
- Source discovery, batch scheduling, built-in retries, Lance blob policies, XML
  parsing, OCR and text extraction are outside the current implementation.
  Configure retries/timeouts through OpenDAL layers when needed.

## Development

Functions that perform filesystem, network or stream I/O use async APIs. Pure
in-memory conversion and validation remain synchronous.

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Tests cover validation, byte preservation, independently
computed checksums, UTC microseconds, missing values, ordering, revisions, empty
batches and Arrow IPC round trips. The PDF fixture is synthetic and distributed
under this repository's MIT license.
