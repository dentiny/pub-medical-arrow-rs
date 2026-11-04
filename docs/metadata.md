# PMC metadata JSON

The reader supports the per-article-version JSON described in the
[official PMC bucket README](https://pmc-oa-opendata.s3.amazonaws.com/README.txt),
checked on 2026-11-03. Real objects were inspected to confirm numeric `pmid`
and `version` fields, and that `pdf_url` can be absent. Files in `metadata/`
and the corresponding article-version prefix use this format.

## API

- `read_metadata(&Operator, path).await` uses native OpenDAL buffers. `path` is
  relative to the operator root and is included in errors.
- `MetadataReader::new(input, metadata_uri).read().await` accepts a futures
  `AsyncRead + Unpin`, including `&mut` streams. The URI is diagnostic context.
  Construction performs no I/O; `read(self)` consumes the reader.
- Both return `PmcMetadata` and retain the entire UTF-8 JSON, including original
  whitespace, number spelling and unknown fields. Reads continue to EOF with
  no configured size limit. An I/O failure returns no partial metadata.
  `PmcMetadata` stores only the original JSON and its diagnostic source location;
  there is no separate copy of the article fields.
- `PmcMetadata::into_pdf_metadata(source_last_modified)` is a
  synchronous extraction into the existing `PdfMetadata` type. It returns
  `Result<Option<PdfMetadata>, MetadataReadError>`: missing or null `pdf_url`
  returns `Ok(None)`, while incorrectly typed mapped fields return `Err`.
  Reading only checks UTF-8, JSON syntax and the object root; article fields
  are checked at conversion, when a PDF URL is present.

## Mapping into PDF snapshots

| JSON field | `PdfMetadata` field | Mapping |
| --- | --- | --- |
| `pmcid` | `pmcid` | Required string; core normalizes it at record construction |
| `version` | `pmc_version` | Optional unsigned 32-bit integer |
| `pmid` | `pmid` | Optional unsigned 64-bit integer or string; integer becomes decimal text |
| `doi` | `doi` | Optional string, unchanged |
| `title` | `title` | Optional string, unchanged |
| `license_code` | `license` | Optional string, unchanged |
| `pdf_url` | `source_uri` | Optional string, preserved including query parameters |
| Entire input | `raw_metadata_json` | Original JSON, not reserialized |

Missing or null optional fields remain `None`; empty strings remain present.
Unknown properties, including XML/text/media links, are retained in raw JSON
but are not fetched. Conversion borrows raw JSON values and decodes only mapped
fields. For duplicate keys, the last value is used for conversion; all original
occurrences remain in the raw JSON.
The core continues to validate PMCID, positive versions and nonblank PDF source
URIs when constructing `PdfRecord`; a parsed `PmcMetadata` is not a validated
PDF record.

`source_last_modified` describes the PDF object in signed microseconds since
the Unix epoch in UTC. The metadata JSON object's
modification time or inventory timestamp must not be substituted for it;
use `None` if the PDF modification time is unknown.

A `pdf_url` is provenance, not an OpenDAL-relative key: it can include a bucket,
URL escaping and an MD5 query parameter. This reader neither resolves it to an
operator/path nor verifies that MD5. Use `PmcReader` for combined metadata/PDF
reads from the PMC bucket or a matching mirror, or supply a PDF path explicitly
to the lower-level reader. `PmcReader` resolves the URL but does not verify MD5.

## Errors

`MetadataReadError` retains the metadata source location and distinguishes
OpenDAL failures, stream failures, invalid UTF-8, invalid JSON/field types and
non-object JSON roots. Wrapped errors remain available as error sources.
JSON parsing uses serde_json's nesting limit.
