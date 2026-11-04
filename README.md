# PMC Arrow

Convert metadata and PDF files from the
[PubMed Central (PMC) Article Datasets](https://registry.opendata.aws/ncbi-pmc/)
into Apache Arrow `RecordBatch` values. Each row contains one PDF snapshot:
article metadata, original PDF bytes, and source provenance.

PMC is an archive of full-text biomedical and life sciences articles. See the
[dataset documentation](https://pmc-oa-opendata.s3.amazonaws.com/README.txt)
for the source data layout, metadata fields and access instructions.

Use the Rust libraries in your own storage pipeline, or the CLI to write
Arrow IPC files.

| Crate | Use it to |
| --- | --- |
| [pmc-arrow 0.1.0](https://crates.io/crates/pmc-arrow/0.1.0) (`crates/rs`) | Validate PDF records and convert them into Arrow batches |
| [pmc-io 0.1.0](https://crates.io/crates/pmc-io/0.1.0) (`crates/io`) | Discover PMC metadata and asynchronously read metadata and PDFs through OpenDAL |
| `pmc-cli` (`crates/cli`) | Convert a PMC source into an Arrow IPC file with the `pmc-arrow` command |

## CLI

Convert a local mirror, or one article's folder in the public PMC bucket:

```sh
pmc-arrow /path/to/pmc-mirror output.arrow
pmc-arrow s3://pmc-oa-opendata article.arrow --prefix PMC10009416.1/
```

`--prefix` defaults to `metadata/`. Use `--parallelism 8`, `--max-rows 32`, and
`--max-memory-bytes 67108864` to control concurrent reads, rows per batch, and
the Arrow array memory flush threshold (these are the defaults; 67108864 bytes
is 64 MiB). `--batch-size` remains an alias for `--max-rows`. All limits must be
positive. Memory is checked after each append, so a batch may cross its threshold
with the final record. A PDF larger than the threshold immediately triggers a flush.
Records arrive in completion order; articles without PDFs are skipped.
The output is an Arrow IPC **file**, including a final partial batch. No PDFs
produces a valid empty file with the PMC schema. Existing output files are
never overwritten; on a conversion error, the incomplete output is removed.
Use `pmc-arrow --help` for all options.

## Quick start

From a checkout, run the conversion example with the included synthetic PDF:

```sh
cargo run --locked -p pmc-arrow --example convert -- PMC123 crates/rs/tests/fixtures/tiny.pdf
```

This builds one row with 9 columns and prints a summary. No download is needed.

Use [pmc-arrow 0.1.0 from crates.io](https://crates.io/crates/pmc-arrow/0.1.0)
in your application:

```toml
[dependencies]
pmc-arrow = "0.1.0"
```

For the I/O examples, use both crates from crates.io:

```toml
[dependencies]
pmc-arrow = "0.1.0"
pmc-io = "0.1.0"
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

This validates the PDF header and metadata and creates a
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
multiple articles. Batching groups PDF rows into columnar buffers so IPC can
encode and write several rows together. Larger batches reduce per-batch overhead
but retain more data until flushing.

Configure both limits with `PdfBatchConfig`; its defaults are 32 rows and 64 MiB:

```rust
use pmc_arrow::{PdfBatchBuilder, PdfBatchConfig};

let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
    max_rows: 32,
    max_memory_bytes: 64 * 1024 * 1024,
})?;
```

The [ingest example](crates/io/examples/ingest.rs) shows the complete flow,
including flushing the final partial batch:

```sh
cargo run --locked -p pmc-io --example ingest -- /path/to/pmc-mirror metadata/
```

The optional third argument sets concurrent article reads (default: 8). Optional
fourth and fifth arguments override the row and Arrow memory limits, for
example `8 32 67108864`. Omitted limits use `PdfBatchConfig::default()`
(32 rows and 64 MiB).

- Discovery streams `.json` paths under `metadata/` without loading all paths.
- `read_many` uses `buffer_unordered`: records arrive as reads complete, with no
  input-order guarantee. Articles without PDFs are skipped; the first observed
  error ends the stream and drops pending reads.
- `PdfBatchBuilder` retains arrival order and duplicates. After each append it
  returns the batch if either the row or memory threshold is reached. Call
  `flush()` after the last record to emit any remaining rows.
- `max_memory_bytes` is compared with the sum of
  `RecordBatch::get_array_memory_size()` for buffered rows after each append.
  Arrow counts array structures and buffer capacities.
  This is not a process memory or RSS limit:
  PDFs are fully read before this check, and concurrent I/O, concatenation at
  flush, and IPC output buffers require additional memory.

I/O is async; validation and Arrow conversion are synchronous.

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
`into_pdf_metadata()` to obtain `Result<Option<PdfMetadata>, _>`.
XML, text and media links remain in raw JSON but their contents are not fetched.

The code and synthetic PDF fixture are distributed under the [MIT license](LICENSE).
