# PMC PDF snapshot schema v1

One row represents one supplied PDF file snapshot. Arrow schema metadata contains
`pmc_arrow.schema_version = "1"` and `pmc_arrow.row_kind = "pdf_snapshot"`.
Column order and nullability are fixed, even for an empty batch.

| Column | Arrow type | Nullable | Meaning |
| --- | --- | --- | --- |
| `pmcid` | `Utf8` | No | Canonical PMC article identifier |
| `pmc_version` | `UInt32` | Yes | Positive source version, if known |
| `pmid` | `Utf8` | Yes | Source-supplied PubMed identifier |
| `doi` | `Utf8` | Yes | Source-supplied DOI |
| `title` | `LargeUtf8` | Yes | Source-supplied article title |
| `license` | `Utf8` | Yes | Source-supplied license label |
| `source_uri` | `Utf8` | No | Original PDF location, including a local path if applicable |
| `source_last_modified` | `Timestamp(Microsecond, "UTC")` | Yes | Last-modified time of the PDF source object, if known |
| `retrieved_at` | `Timestamp(Microsecond, "UTC")` | No | Caller-supplied retrieval time |
| `pdf_sha256` | `FixedSizeBinary(32)` | No | SHA-256 over the exact PDF bytes, stored as 32 bytes, not hex |
| `pdf_size_bytes` | `UInt64` | No | Length of the exact PDF payload |
| `pdf_bytes` | `LargeBinary` | No | Complete unmodified PDF payload |
| `raw_metadata_json` | `LargeUtf8` | Yes | Verbatim source metadata JSON object |

## Identity and provenance

PMCID normalization trims surrounding whitespace, accepts an optional
case-insensitive `PMC` prefix, requires positive ASCII decimal digits, removes
leading zeros and adds `PMC`. Thus ` pmc00123 ` and `123` both become `PMC123`.
An embedded version such as `PMC123.2` is rejected: pass version 2 separately.
The numeric component is not constrained to a machine integer.

`pmc_version = 0` is rejected. A missing version stays null. The converter does
not infer a version from filenames or source URIs. Other supplied text fields,
including DOI, PMID, license and URI, are preserved without normalization. The
source location is checked only for nonblank content, not network reachability
or URI syntax.

PMC may update content under the same article version. The digest identifies
the PDF content separately from its article and version. The library imposes no
primary key, performs no deduplication and makes no assumption that a higher
version is preferred. See the
[official PMC versioning documentation](https://pmc.ncbi.nlm.nih.gov/tools/pmcaws/).

`source_last_modified` must describe the PDF object, not the article's publication
date or an unrelated metadata object's timestamp. Both timestamps are signed
UTC microseconds since 1970-01-01; zero and negative values are representable.
The library does not read the system clock or impose ordering between these times.

## Input validation and preservation

`PdfRecord::try_new(Vec<u8>, PdfMetadata)` validates metadata and the payload,
then computes the digest. Private record fields and read-only accessors keep
bytes, digest and length consistent. Inputs remain owned by the caller until
they are passed to this constructor; construction does not copy the PDF vector.

The header must start at byte zero with `%PDF-`, one ASCII digit, `.`, one ASCII
digit and a CR or LF. This is a lightweight ingestion check, not a complete PDF
validator. Header-only or truncated inputs can pass. Leading garbage is rejected,
even if a tolerant PDF reader might accept it. No text decoding or payload
rewriting is performed; binary data is retained exactly.

Raw metadata must be a syntactically valid JSON object. JSON null, arrays, scalar
values and trailing non-whitespace data are rejected. Parsing uses a borrowed
`RawValue`, so large numeric tokens are not converted to floating point, duplicate
keys are not collapsed, and original whitespace is retained. Absent metadata is
null; `{}` is a present empty object. Validation uses serde_json's nesting limit.

Optional typed fields are caller-supplied independently of raw JSON. The core
does not infer mappings or resolve disagreements. A future source adapter in
`pmc-io` should define that mapping explicitly and preserve its source JSON.

## Batches and memory

`to_record_batch(&[PdfRecord])` preserves row order, duplicates, nulls and all
payload bytes. Empty input yields zero rows and all 13 columns. Errors are typed
through `pmc_arrow::Error`; malformed records cannot be constructed through the
public API and are not silently skipped.

PDF bytes are copied into one contiguous Arrow `LargeBinary` buffer. The input
records stay alive until their owner drops them. The converter checks aggregate
string/binary lengths against Arrow offset limits before array construction, but
it does not recover from memory exhaustion. `Utf8` columns use 32-bit offsets;
`LargeUtf8` and `LargeBinary` use 64-bit offsets. Callers must size batches to fit
available memory, accounting for both inputs and outputs.

The schema contains no Lance-specific field metadata. Selecting a Lance blob
storage policy and writing a dataset belong to the future I/O crate. Arrow IPC
round-trip tests verify portable Arrow serialization; they do not establish
Lance integration compatibility.
