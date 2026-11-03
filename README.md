# PMC Arrow

Convert PubMed Central PDF bytes and source metadata into Apache Arrow
`RecordBatch` values. Each row is one PDF file snapshot, retaining the complete
original PDF bytes, a SHA-256 checksum and source provenance for downstream
dataset storage.

## Workspace

| Directory | Crate | Status |
| --- | --- | --- |
| `crates/rs` | `pmc-arrow` | Implemented: model, validation, schema and in-memory Arrow conversion |
| `crates/io` | `pmc-io` | Placeholder for discovery, downloads, batching and Lance output |
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

The example builds an in-memory batch; it does not write a dataset.

## Conversion contract

- [Schema and validation details](docs/schema.md).
- PDF data is `LargeBinary`, the digest is `FixedSizeBinary(32)`, and long text
  fields are `LargeUtf8`. Timestamps carry a UTC timezone in Arrow.
- Missing optional values stay null; supplied empty strings stay empty. Unknown
  versions are never replaced with version 1.
- Raw JSON must be an object and is retained verbatim, including unknown fields,
  large numbers, duplicate keys and whitespace. The library does not map a
  provider's JSON into typed fields automatically.
- PDF validation checks only the header. It is not a PDF parser and cannot
  certify completeness, renderability or safety.
- Batch conversion preserves order and duplicates. It does not select a preferred
  article version, infer licensing, deduplicate or read source URIs.
- PDF bytes are copied into the Arrow column once. Caller-owned records remain
  allocated until dropped. Limit both rows and bytes before calling the converter;
  the core library does not impose a batch budget or catch allocation exhaustion.
- Downloads, retries, Lance blob policies, XML parsing, OCR and text extraction
  are outside the current implementation.

## Development

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Tests cover the schema contract, validation, byte preservation, independently
computed checksums, UTC microseconds, missing values, ordering, revisions, empty
batches and Arrow IPC round trips. The PDF fixture is synthetic and distributed
under this repository's MIT license.
