# PMC Arrow

Convert metadata and PDF files from the
[PubMed Central (PMC) Article Datasets](https://registry.opendata.aws/ncbi-pmc/)
into Apache Arrow `RecordBatch` values. Each row contains one PDF snapshot:
article metadata, original PDF bytes, their SHA-256 digest, and source provenance.

PMC is an archive of full-text biomedical and life sciences articles. See the
[dataset documentation](https://pmc-oa-opendata.s3.amazonaws.com/README.txt)
for the source data layout, metadata fields and access instructions.

The library produces in-memory batches for your own storage pipeline. It does
not write Arrow files, Parquet or Lance datasets, or extract text from PDFs.

| Crate | Use it to |
| --- | --- |
| `pmc-arrow` (`crates/rs`) | Validate PDF records and convert them into Arrow batches |
| `pmc-io` (`crates/io`) | Discover PMC metadata and asynchronously read metadata and PDFs through OpenDAL |
| `pmc-cli` (`crates/cli`) | Reserved for a future CLI; currently a placeholder |

## Quick start

From a checkout, run the conversion example with the included synthetic PDF:

```sh
cargo run --locked -p pmc-arrow --example convert -- PMC123 crates/rs/tests/fixtures/tiny.pdf
```

This builds one row with 12 columns and prints a summary. No download is needed.

For the library examples below, add these dependencies to your application's
`Cargo.toml`, replacing the checkout paths:

```toml
[dependencies]
pmc-arrow = { path = "/path/to/pub-medical-arrow-rs/crates/rs" }
pmc-io = { path = "/path/to/pub-medical-arrow-rs/crates/io" }
arrow-array = "58"
futures = { version = "0.3", default-features = false, features = ["std", "async-await"] }
opendal = { version = "0.59.3", default-features = false, features = ["services-fs", "executors-tokio", "http-transport-reqwest"] }
tokio = { version = "1", features = ["macros", "rt", "fs", "io-std", "io-util"] }
```

If you already have PDF bytes, only `pmc-arrow` and `arrow-array` are needed for
the first example. Backend features, credentials, retries and timeouts are
configured by your application through OpenDAL. The workspace uses Arrow 58
and OpenDAL 0.59; its lockfile pins them to 58.4.0 and 0.59.4 respectively.

## Convert PDF bytes

```rust
use arrow_array::RecordBatch;
use pmc_arrow::{to_record_batch, PdfMetadata, PdfRecord};

fn convert(pdf: Vec<u8>) -> pmc_arrow::Result<RecordBatch> {
    let mut metadata = PdfMetadata::new("PMC123", "file:///data/PMC123.pdf");
    metadata.title = Some("Example article".into());
    metadata.pmc_version = Some(2);

    let record = PdfRecord::try_new(pdf, metadata)?;
    to_record_batch(&[record])
}
```

`PdfRecord::try_new` takes ownership of the bytes, validates the metadata and
PDF header, and computes SHA-256. Its fields are immutable; use `metadata()`,
`pdf_bytes()`, `pdf_sha256()` and `pdf_size_bytes()` to inspect them.
`to_record_batch` preserves input order and duplicates. Empty input produces
an empty batch with the same schema.

## Read a PMC article

`PmcReader` accepts an OpenDAL `Operator` rooted at the `pmc-oa-opendata` bucket,
or at a local mirror preserving its object paths. Metadata paths are relative
to that root. The source format is described in the
[official PMC dataset README](https://pmc-oa-opendata.s3.amazonaws.com/README.txt).

```rust
use arrow_array::RecordBatch;
use opendal::Operator;
use pmc_arrow::to_record_batch;
use pmc_io::PmcReader;

async fn read_article(
    operator: Operator,
    metadata_path: &str,
) -> Result<Option<RecordBatch>, Box<dyn std::error::Error>> {
    let reader = PmcReader::new(operator);
    match reader.read(metadata_path).await? {
        Some(record) => Ok(Some(to_record_batch(&[record])?)),
        None => Ok(None), // The article has no PDF URL.
    }
}
```

The reader fetches the JSON, resolves its `pdf_url`, then reads that PDF.
Missing or null `pdf_url` returns `None`; a referenced PDF that cannot be read
returns an error. PDF URLs must start with `s3://pmc-oa-opendata/`. Query and
fragment components are removed and percent escapes decoded for the object
path; the original URL is preserved in `source_uri`. The source MD5 query
parameter is not verified. `source_last_modified` stays null because the reader
does not fetch the PDF's modification time.

For example, a local mirror could contain:

```text
pmc-mirror/
├── metadata/PMC123.2.json
└── PMC123.2/article.pdf
```

The JSON's `pdf_url` would be
`s3://pmc-oa-opendata/PMC123.2/article.pdf`. Point the operator at `pmc-mirror/`,
not its `metadata/` directory. To try the included single-article example:

```sh
cargo run --locked -p pmc-io --example pmc -- /path/to/pmc-mirror metadata/PMC123.2.json
```

For the [public S3 bucket](https://registry.opendata.aws/ncbi-pmc/), add
`"services-s3"` to the OpenDAL features above and construct the operator inside
your async application as follows. Anonymous access does not require credentials:

```rust
opendal::install_default();
let operator = opendal::Operator::new(
    opendal::services::S3::default()
        .bucket("pmc-oa-opendata")
        .region("us-east-1")
        .skip_signature(),
)?;
```

## Discover, read concurrently, and batch

The following complete program reads a local mirror. Save it as `src/main.rs`
and run `cargo run -- /path/to/pmc-mirror metadata/` in your application:

```rust
use arrow_array::RecordBatch;
use futures::{pin_mut, TryStreamExt};
use opendal::{services::Fs, Operator};
use pmc_arrow::PdfBatchBuilder;
use pmc_io::{discover_metadata, PmcReader};
use tokio::io::AsyncWriteExt;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("expected mirror root")?;
    let prefix = args.next().ok_or("expected metadata prefix")?;
    let root = tokio::fs::canonicalize(root).await?;
    let operator =
        Operator::new(Fs::default().root(root.to_str().ok_or("invalid mirror directory")?))?;

    let paths = discover_metadata(&operator, &prefix).await?;
    let reader = PmcReader::new(operator);
    let records = reader.read_many(paths, 8); // Up to 8 article reads in flight.
    pin_mut!(records);
    let mut builder = PdfBatchBuilder::new(32)?; // Up to 32 rows per batch.

    while let Some(record) = records.try_next().await? {
        if let Some(batch) = builder.append(record)? {
            consume(batch).await?;
        }
    }
    // Emit the final partial batch, if any.
    if let Some(batch) = builder.flush()? {
        consume(batch).await?;
    }
    Ok(())
}

async fn consume(batch: RecordBatch) -> std::io::Result<()> {
    // Replace this summary with your application's async dataset writer.
    let summary = format!(
        "batch: {} rows, {} columns\n",
        batch.num_rows(),
        batch.num_columns()
    );
    let mut stdout = tokio::io::stdout();
    stdout.write_all(summary.as_bytes()).await?;
    stdout.flush().await
}
```

The same flow is available in the repository with configurable concurrency and
batch size:

```sh
cargo run --locked -p pmc-io --example ingest -- /path/to/pmc-mirror metadata/ 8 32
```

- **Discovery** recursively lists `.json` files under the prefix, streaming paths
  without reading their contents. The operator must support listing. Use
  `metadata/` to avoid discovering the duplicate JSON copies in article folders.
  If you already have paths, pass a stream such as
  `futures::stream::iter(paths.into_iter().map(Ok))` to `read_many` instead.
- **Concurrent reads** use `buffer_unordered`: records arrive as reads complete,
  with no input-order guarantee. A parallelism of zero is treated as one.
  Articles without PDFs are skipped. The first observed error ends the stream
  and drops pending reads; previously returned batches remain with the caller.
- **Batching** preserves the order in which records arrive and retains duplicates.
  `PdfBatchBuilder::new` requires a positive row count. `append` emits full batches;
  `flush` emits any remainder, or returns `None` when empty.
- **Memory** depends on PDF sizes as well as concurrency and batch size. Each PDF
  is fully materialized, with no size limit. Arrow conversion copies payloads
  into column buffers while the input records still exist. Consume each batch
  before requesting more; row and concurrency limits are not byte limits.

File, network and stream I/O is async. Validation, hashing and Arrow conversion
are synchronous. The library does not choose article versions, filter licenses
or deduplicate records.

## Arrow schema

`pmc_arrow::schema()` returns the following columns in this order, including for
empty batches. Schema metadata contains `pmc_arrow.row_kind = "pdf_snapshot"`.

| Column | Arrow type | Nullable | Meaning |
| --- | --- | --- | --- |
| `pmcid` | `Utf8` | No | Canonical PMC identifier, e.g. `PMC123` |
| `pmc_version` | `UInt32` | Yes | Positive article version, if known |
| `pmid` | `Utf8` | Yes | PubMed identifier |
| `doi` | `Utf8` | Yes | Source DOI |
| `title` | `LargeUtf8` | Yes | Article title |
| `license` | `Utf8` | Yes | Source license label |
| `source_uri` | `Utf8` | No | Original PDF location, including query parameters |
| `source_last_modified` | `Timestamp(Microsecond, "UTC")` | Yes | PDF modification time: signed microseconds since the Unix epoch (1970-01-01 UTC) |
| `pdf_sha256` | `FixedSizeBinary(32)` | No | SHA-256 of the exact PDF payload, stored as 32 raw bytes, not hex |
| `pdf_size_bytes` | `UInt64` | No | PDF payload length in bytes |
| `pdf_bytes` | `LargeBinary` | No | Complete, unmodified PDF payload |
| `raw_metadata_json` | `LargeUtf8` | Yes | Verbatim source JSON object, including unknown fields and whitespace |

The PMC adapter maps `version` → `pmc_version`, `license_code` → `license`, and
`pdf_url` → `source_uri`; numeric PMIDs become decimal strings. Other article
fields map by name. Missing optional values remain null; supplied empty strings
stay empty. Unknown versions are not replaced with version 1.

The core normalizes PMCID (` pmc00123 ` → `PMC123`); provide a version separately,
not embedded as `PMC123.2`. PDF validation checks only the header, not document
completeness. Supplied raw metadata must be a JSON object. With `pmc-arrow` alone,
typed fields are caller-supplied independently of raw JSON; no mapping is inferred.

## Separate PDF and metadata reads

Use these APIs when you need to control metadata mapping or PDF paths yourself:

| Input | API | Result |
| --- | --- | --- |
| OpenDAL PDF object | `read_pdf(&operator, path, metadata).await` | `PdfRecord` |
| futures `AsyncRead` PDF stream | `PdfReader::new(input, metadata).read().await` | `PdfRecord` |
| OpenDAL metadata object | `read_metadata(&operator, path).await` | `PmcMetadata` |
| futures `AsyncRead` JSON stream | `MetadataReader::new(input, source_uri).read().await` | `PmcMetadata` |

All four APIs are exported by `pmc-io` and return `Result`. OpenDAL PDF reads use
native buffer chunks without an `AsyncRead` adapter. Stream reader constructors
perform no I/O; `read` consumes the reader. `PmcMetadata` retains the original
JSON even for articles without PDFs. Call
`into_pdf_metadata(source_last_modified)` to obtain `Result<Option<PdfMetadata>, _>`;
pass `None` if the PDF modification time is unknown, or UTC microseconds if known.
XML, text and media links remain in raw JSON but their contents are not fetched.

## Development

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The code and synthetic PDF fixture are distributed under the [MIT license](LICENSE).
