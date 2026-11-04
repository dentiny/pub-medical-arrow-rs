# PDF batches

`PdfBatchBuilder` accepts validated `PdfRecord` values and emits Arrow batches:

```rust
let mut builder = pmc_arrow::PdfBatchBuilder::new(128)?;
for record in records {
    if let Some(batch) = builder.append(record)? {
        consume(batch);
    }
}
if let Some(batch) = builder.flush()? {
    consume(batch);
}
```

`append` takes ownership of each record and automatically flushes a full batch.
`flush` emits the remaining rows, returns `None` when empty, and allows reuse of
the same builder. It releases buffered records even if conversion returns an
error. Output preserves input order, duplicate records and the existing schema.

The positive batch size limits rows, not PDF bytes. Records are buffered in full
until conversion through `to_record_batch`, which copies payloads into Arrow's
column buffers. All builder operations are synchronous; callers can feed records
from an async reader or stream and consume each returned batch before continuing.
