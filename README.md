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

Add these dependencies to your application, replacing the checkout paths:

```toml
[dependencies]
pmc-arrow = { path = "/path/to/pub-medical-arrow-rs/crates/rs" }
pmc-io = { path = "/path/to/pub-medical-arrow-rs/crates/io" }
opendal = { version = "0.59.3", default-features = false, features = ["services-fs", "executors-tokio", "http-transport-reqwest"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

For in-memory conversion alone, only `pmc-arrow` is needed. Enable the OpenDAL
backend you use (`services-fs` here). The workspace uses Arrow 58 and OpenDAL 0.59.

## Convert PDF bytes

Given PDF bytes in a `Vec<u8>`, inside a function returning `Result`:

```rust
use pmc_arrow::{to_record_batch, PdfMetadata, PdfRecord};

let metadata = PdfMetadata::new("PMC123", "file:///data/PMC123.pdf");
let record = PdfRecord::try_new(pdf_bytes, metadata)?;
let batch = to_record_batch(&[record])?;
```

This validates the PDF header and metadata, computes SHA-256, and creates a
one-row Arrow batch.

## Read a PMC article

Point an OpenDAL operator at a local mirror of the PMC bucket:

```rust
use opendal::{services::Fs, Operator};
use pmc_arrow::to_record_batch;
use pmc_io::PmcReader;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let operator = Operator::new(Fs::default().root("/path/to/pmc-mirror"))?;
    let reader = PmcReader::new(operator);

    if let Some(record) = reader.read("metadata/PMC123.2.json").await? {
        let batch = to_record_batch(&[record])?;
        assert_eq!(batch.num_rows(), 1);
    }
    Ok(())
}
```

The reader loads metadata and its referenced PDF. An article without a PDF URL
returns `None`. The mirror must preserve bucket paths: for example,
`metadata/PMC123.2.json` and `PMC123.2/article.pdf`, where the JSON's `pdf_url`
is `s3://pmc-oa-opendata/PMC123.2/article.pdf`.

See the runnable [single-article example](crates/io/examples/pmc.rs).

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

Use `discover_metadata` → `PmcReader::read_many` → `PdfBatchBuilder` to process
multiple articles. The [ingest example](crates/io/examples/ingest.rs) shows the
complete flow, including flushing the final partial batch:

```sh
cargo run --locked -p pmc-io --example ingest -- /path/to/pmc-mirror metadata/ 8 32
```

Here, `8` is the number of concurrent article reads and `32` is the batch row limit.

- Discovery streams `.json` paths under `metadata/` without loading all paths.
- `read_many` uses `buffer_unordered`: records arrive as reads complete, with no
  input-order guarantee. Articles without PDFs are skipped; the first observed
  error ends the stream and drops pending reads.
- `PdfBatchBuilder` retains arrival order and duplicates. Call `flush()` after
  the last record to emit any remaining rows.
- Each PDF is fully loaded into memory and copied into Arrow buffers. Concurrency
  and batch size limit counts, not bytes; choose them to fit available memory.

I/O is async; validation, hashing and Arrow conversion are synchronous.

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
